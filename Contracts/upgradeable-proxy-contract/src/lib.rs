// lib.rs
#![no_std]
//! Upgradeable Proxy Contract
//! 
//! This contract provides a governance & multisig guarded upgradeable proxy pattern.
//! It stores implementation contract IDs and delegates external calls to the active implementation
//! while keeping state in the proxy's instance storage. Upgrade proposals require multisig
//! approvals and a time delay before execution. A rollback mechanism allows reverting to a
//! previous implementation version.

mod storage;
mod types;
mod error;

use soroban_sdk::{
    contract, contractimpl, contracttype, Env, Address, Bytes, Symbol, Vec, Val, TryFromVal, Map,
    IntoVal,
};
use crate::storage::Storage;
use crate::types::{UpgradeProposal, ImplementationRecord, UpgradeImpact, MigrationProgress};
use crate::error::ProxyError;

#[contract]
pub struct UpgradeableProxyContract;

/// Public interface for the proxy.
pub trait ProxyTrait {
    /// Initialize the proxy with a list of admin addresses, a multisig threshold, and an execution delay (seconds).
    /// Can only be called once. Threshold must be >0 and <= admins length.
    fn init(env: Env, admins: Vec<Address>, threshold: u32, delay_seconds: u64) -> Result<(), ProxyError>;
    /// Create an upgrade proposal pointing to a new implementation contract Address plus arbitrary metadata.
    fn propose_upgrade(env: Env, new_impl: Address, metadata: Bytes) -> Result<u64, ProxyError>;
    /// Approve an existing proposal; each admin counted once.
    fn approve_upgrade(env: Env, proposal_id: u64, admin: Address) -> Result<(), ProxyError>;
    /// Execute an approved proposal after delay; records history and bumps version.
    fn execute_upgrade(env: Env, proposal_id: u64) -> Result<(), ProxyError>;
    /// Roll back to previous implementation (latest history entry prev field). Only if history exists.
    fn rollback(env: Env) -> Result<(), ProxyError>;
    /// Returns current implementation address (error if unset).
    fn get_current_implementation(env: Env) -> Result<Address, ProxyError>;
    /// Returns current implementation version (monotonic u64) starting at 0 before first upgrade.
    fn get_version(env: Env) -> u64;
    /// Fetch proposal by id.
    fn get_proposal(env: Env, proposal_id: u64) -> Result<UpgradeProposal, ProxyError>;
    /// Forward an arbitrary call (symbol+args) to the active implementation (delegate pattern).
    fn forward(env: Env, func: Symbol, args: Vec<Val>) -> Result<Val, ProxyError>;
    /// Get upgrade statistics.
    fn get_upgrade_stats(env: Env) -> Map<Symbol, Val>;
    /// Get upgrade checklist for a proposal.
    fn get_upgrade_checklist(env: Env, proposal_id: u64) -> Vec<Symbol>;
}

#[contractimpl]
impl ProxyTrait for UpgradeableProxyContract {
    fn init(env: Env, admins: Vec<Address>, threshold: u32, delay_seconds: u64) -> Result<(), ProxyError> {
        if admins.len() == 0 { return Err(ProxyError::InvalidAdmins); }
        if threshold == 0 || (threshold as u64) > (admins.len() as u64) { return Err(ProxyError::InvalidThreshold); }
        let store = Storage::new(&env);
        if store.is_initialized() { return Err(ProxyError::AlreadyInitialized); }
        store.init(admins, threshold, delay_seconds);
        Ok(())
    }

    fn propose_upgrade(env: Env, new_impl: Address, metadata: Bytes) -> Result<u64, ProxyError> {
        let store = Storage::new(&env);
        store.require_initialized()?;
        store.require_admin_auth()?; // any admin can auth (mock_all_auths in tests)
        
        // Input validation
        if let Some(current) = store.current_impl() {
            if current == new_impl {
                return Err(ProxyError::SameImplementation);
            }
        }
        
        // Validate metadata size (reasonable limit of 1KB)
        if metadata.len() > 1024 {
            return Err(ProxyError::MetadataTooLarge);
        }
        
        // Get the current invoker and store it
        let current_invoker = env.current_contract_address();
        store.set_last_invoker(&current_invoker);
        
        // create proposal id incrementally
        let id = store.next_proposal_id();
        let ledger_ts = env.ledger().timestamp();
        let proposal = UpgradeProposal {
            id,
            new_impl: new_impl.clone(),
            metadata,
            proposer: store.last_invoker()?,
            approvals: Vec::new(&env),
            created_at: ledger_ts,
            executable_at: ledger_ts + store.delay_seconds(),
            executed: false,
        };
        store.save_proposal(&proposal)?;
        // Emit event for monitoring
        let topic1: Val = env.current_contract_address().into_val(&env);
        let topic2: Val = Symbol::new(&env, "proposal_created").into_val(&env);
        let topics: Vec<Val> = Vec::from_array(&env, [topic1, topic2]);
        let data: Vec<Val> = Vec::from_array(&env, [
            id.into_val(&env),
            (proposal.approvals.len() as u32).into_val(&env),
            (proposal.executable_at - ledger_ts).into_val(&env),
        ]);
        env.events().publish(topics, data);
        Ok(id)
    }

    fn approve_upgrade(env: Env, proposal_id: u64, admin: Address) -> Result<(), ProxyError> {
        let store = Storage::new(&env);
        store.require_initialized()?;
        admin.require_auth();
        if !store.is_admin(&admin) { return Err(ProxyError::NotAdmin); }
        let mut proposal = store.get_proposal(proposal_id)?;
        if proposal.executed { return Err(ProxyError::AlreadyExecuted); }
        if !proposal.approvals.contains(&admin) {
            proposal.approvals.push_back(admin);
        }
        store.save_proposal(&proposal)?;
        // Emit event for monitoring
        let remaining = store.threshold() as usize - proposal.approvals.len() as usize;
        let topic1: Val = env.current_contract_address().into_val(&env);
        let topic2: Val = Symbol::new(&env, "approval_added").into_val(&env);
        let topics: Vec<Val> = Vec::from_array(&env, [topic1, topic2]);
        let data: Vec<Val> = Vec::from_array(&env, [
            proposal_id.into_val(&env),
            (remaining as u32).into_val(&env),
        ]);
        env.events().publish(topics, data);
        Ok(())
    }

    fn execute_upgrade(env: Env, proposal_id: u64) -> Result<(), ProxyError> {
        let store = Storage::new(&env);
        store.require_initialized()?;
        let mut proposal = store.get_proposal(proposal_id)?;
        if proposal.executed { return Err(ProxyError::AlreadyExecuted); }
        let threshold = store.threshold();
        if (proposal.approvals.len() as u32) < threshold { return Err(ProxyError::ThresholdNotMet); }
        let now = env.ledger().timestamp();
        if now < proposal.executable_at { return Err(ProxyError::DelayNotPassed); }
        // --- Validation Hook ---
        // Require new implementation to expose `schema_version() -> u32` returning >0
        // If this invocation fails or returns 0 treat as validation failure.
        let schema_sym = Symbol::new(&env, "schema_version");
        let schema_invoke = env.try_invoke_contract::<Val, Val>(&proposal.new_impl, &schema_sym, Vec::new(&env));
        let version_val = if let Ok(Ok(v)) = schema_invoke { v } else { return Err(ProxyError::ValidationFailed); };
        let schema_u32: u32 = u32::try_from_val(&env, &version_val).map_err(|_| ProxyError::ValidationFailed)?;
        if schema_u32 == 0 { return Err(ProxyError::ValidationFailed); }

        // Compatibility check
        let current_impl = store.current_impl().unwrap_or(proposal.new_impl.clone());
        let compat_sym = Symbol::new(&env, "proxy_compatible");
        let current_val = current_impl.into_val(&env);
        let compat_args = Vec::from_array(&env, [current_val]);
        let compat_invoke = env.try_invoke_contract::<Val, Val>(&proposal.new_impl, &compat_sym, compat_args);
        let compat_val = if let Ok(Ok(v)) = compat_invoke { v } else { return Err(ProxyError::CompatibilityCheckFailed); };
        let is_compat: bool = bool::try_from_val(&env, &compat_val).map_err(|_| ProxyError::CompatibilityCheckFailed)?;
        if !is_compat { return Err(ProxyError::CompatibilityCheckFailed); }

        // Impact analysis sim
        let mut impact = UpgradeImpact {
            critical_funcs_tested: Vec::from_array(&env, [Symbol::new(&env, "balance"), Symbol::new(&env, "transfer")]),
            failed_sims: Vec::new(&env),
            estimated_gas_increase: 0,
        };
        for func in impact.critical_funcs_tested.iter() {
            let sim_args = Vec::new(&env); // Or parse from metadata
            let sim_invoke = env.try_invoke_contract::<Val, Val>(&proposal.new_impl, &func, sim_args);
            if !matches!(sim_invoke, Ok(Ok(_))) {
                impact.failed_sims.push_back(func.clone());
            }
        }
        // Emit impact event
        let topic1: Val = env.current_contract_address().into_val(&env);
        let topic2: Val = Symbol::new(&env, "upgrade_impact").into_val(&env);
        let topics: Vec<Val> = Vec::from_array(&env, [topic1, topic2]);
        let data: Vec<Val> = Vec::from_array(&env, [
            proposal.id.into_val(&env),
            (impact.critical_funcs_tested.len() as u32).into_val(&env),
            (impact.failed_sims.len() as u32).into_val(&env),
        ]);
        env.events().publish(topics, data);

        let prev_impl = store.current_impl().unwrap_or(proposal.new_impl.clone());
        let prev_version = store.version();
        store.set_implementation(&proposal.new_impl)?;
        store.record_history(ImplementationRecord { version: prev_version + 1, implementation: proposal.new_impl.clone(), prev: prev_impl.clone() });

        // --- Optional Migration Hook ---
        // If metadata first byte == 1 attempt to call `migrate()` (no args, no return required)
        if proposal.metadata.len() > 0 {
            let flag: u8 = proposal.metadata.get_unchecked(0);
            if flag == 1u8 {
                // Pre-validation
                let state_hash = Bytes::new(&env); // Placeholder; no direct hash
                let state_hash_val = state_hash.clone().into_val(&env);
                let val_sym = Symbol::new(&env, "validate_migration");
                let val_args = Vec::from_array(&env, [state_hash_val]);
                let val_invoke = env.try_invoke_contract::<Val, Val>(&proposal.new_impl, &val_sym, val_args);
                let valid_val = if let Ok(Ok(v)) = val_invoke { v } else { return Err(ProxyError::MigrationValidationFailed); };
                let is_valid: bool = bool::try_from_val(&env, &valid_val).map_err(|_| ProxyError::MigrationValidationFailed)?;
                if !is_valid { return Err(ProxyError::MigrationValidationFailed); }

                // Track progress via events/temp storage
                let progress_key = DataKey::MigrationProgress(proposal.id);
                let progress = MigrationProgress { phase: 0, total_phases: 1, completed: false };
                env.storage().instance().set(&progress_key, &progress);

                let migrate_sym = Symbol::new(&env, "migrate");
                let migrate_args = Vec::new(&env); // Simplified; parse metadata later if needed
                let migrate_invoke = env.try_invoke_contract::<Val, Val>(&proposal.new_impl, &migrate_sym, migrate_args);
                if matches!(migrate_invoke, Ok(Ok(_))) {
                    // Emit completion
                    let topic1: Val = env.current_contract_address().into_val(&env);
                    let topic2: Val = Symbol::new(&env, "migration_complete").into_val(&env);
                    let topics: Vec<Val> = Vec::from_array(&env, [topic1, topic2]);
                    let data: Vec<Val> = Vec::from_array(&env, [proposal.id.into_val(&env)]);
                    env.events().publish(topics, data);
                    env.storage().instance().remove(&progress_key);
                } else {
                    // Auto-rollback: Revert impl, call rollback_migration on prev
                    store.set_implementation(&prev_impl)?;
                    let rb_sym = Symbol::new(&env, "rollback_migration");
                    let _rb_invoke = env.try_invoke_contract::<Val, Val>(&prev_impl, &rb_sym, Vec::new(&env)); // Best-effort
                    // Clean up progress
                    env.storage().instance().remove(&progress_key);
                    return Err(ProxyError::MigrationFailed);
                }
            }
        }

        proposal.executed = true;
        store.save_proposal(&proposal)?;
        // Emit execution event
        let topic1: Val = env.current_contract_address().into_val(&env);
        let topic2: Val = Symbol::new(&env, "upgrade_executed").into_val(&env);
        let topics: Vec<Val> = Vec::from_array(&env, [topic1, topic2]);
        let data: Vec<Val> = Vec::from_array(&env, [
            proposal.id.into_val(&env),
            now.into_val(&env),
        ]);
        env.events().publish(topics, data);
        Ok(())
    }

    fn rollback(env: Env) -> Result<(), ProxyError> {
        let store = Storage::new(&env);
        store.require_initialized()?;
        store.require_admin_auth()?;
        
        let history_record = store.get_last_history_record().ok_or(ProxyError::NoRollbackAvailable)?;
        let prev_impl = history_record.prev;
        let prev_version = history_record.version - 1; // Rollback to previous version

        // Validation
        let current_impl = store.current_impl().ok_or(ProxyError::ImplementationNotSet)?;
        let compat_sym = Symbol::new(&env, "rollback_compatible");
        let current_val = current_impl.into_val(&env);
        let compat_args = Vec::from_array(&env, [current_val]);
        let compat_invoke = env.try_invoke_contract::<Val, Val>(&prev_impl, &compat_sym, compat_args);
        let compat_val = if let Ok(Ok(v)) = compat_invoke { v } else { return Err(ProxyError::ValidationFailed); };
        let is_compat: bool = bool::try_from_val(&env, &compat_val).map_err(|_| ProxyError::ValidationFailed)?;
        if !is_compat { return Err(ProxyError::ValidationFailed); }

        // Set implementation and update version
        store.set_implementation(&prev_impl)?;
        store.update_version(prev_version);
        
        // Remove the last history entry (rollback operation)
        store.remove_last_history_entry();

        // Post-rollback impact sim (similar to upgrade)
        let mut impact = UpgradeImpact {
            critical_funcs_tested: Vec::from_array(&env, [Symbol::new(&env, "balance"), Symbol::new(&env, "transfer")]),
            failed_sims: Vec::new(&env),
            estimated_gas_increase: 0,
        };
        for func in impact.critical_funcs_tested.iter() {
            let sim_args = Vec::new(&env);
            let sim_invoke = env.try_invoke_contract::<Val, Val>(&prev_impl, &func, sim_args);
            if !matches!(sim_invoke, Ok(Ok(_))) {
                impact.failed_sims.push_back(func.clone());
            }
        }
        let topic1: Val = env.current_contract_address().into_val(&env);
        let topic2: Val = Symbol::new(&env, "rollback_complete").into_val(&env);
        let topics: Vec<Val> = Vec::from_array(&env, [topic1, topic2]);
        let data: Vec<Val> = Vec::from_array(&env, [
            prev_version.into_val(&env),
            (impact.failed_sims.len() as u32).into_val(&env),
        ]);
        env.events().publish(topics, data);
        
        Ok(())
    }

    fn get_current_implementation(env: Env) -> Result<Address, ProxyError> { Storage::new(&env).current_impl().ok_or(ProxyError::ImplementationNotSet) }
    fn get_version(env: Env) -> u64 { Storage::new(&env).version() }
    fn get_proposal(env: Env, proposal_id: u64) -> Result<UpgradeProposal, ProxyError> { Storage::new(&env).get_proposal(proposal_id) }

    fn forward(env: Env, func: Symbol, args: Vec<Val>) -> Result<Val, ProxyError> {
        let store = Storage::new(&env);
        store.require_initialized()?;
        let target = store.current_impl().ok_or(ProxyError::ImplementationNotSet)?;
        let invoke_res = env.try_invoke_contract::<Val, Val>(&target, &func, args);
        if let Ok(Ok(res)) = invoke_res {
            Ok(res)
        } else {
            Err(ProxyError::InvocationFailed)
        }
    }

    fn get_upgrade_stats(env: Env) -> Map<Symbol, Val> {
        let store = Storage::new(&env);
        let history_len = store.history_len();
        let mut stats = Map::new(&env);
        stats.set(Symbol::new(&env, "total_upgrades"), history_len.into_val(&env));
        // Add more stats as needed, e.g., avg_approval_time
        stats
    }

    fn get_upgrade_checklist(env: Env, proposal_id: u64) -> Vec<Symbol> {
        let store = Storage::new(&env);
        if let Ok(prop) = store.get_proposal(proposal_id) {
            let mut checklist = Vec::new(&env);
            if (prop.approvals.len() as u32) >= store.threshold() {
                checklist.push_back(Symbol::new(&env, "threshold_met"));
            }
            if prop.executed {
                checklist.push_back(Symbol::new(&env, "executed"));
            }
            if env.ledger().timestamp() >= prop.executable_at {
                checklist.push_back(Symbol::new(&env, "delay_passed"));
            }
            // Add more checklist items
            checklist
        } else {
            Vec::new(&env)
        }
    }
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admins,
    Threshold,
    Delay,
    Impl,
    Version,
    Proposals,
    ProposalSeq,
    History,
    LastInvoker,
    MigrationProgress(u64),
}

#[cfg(test)]
mod test;