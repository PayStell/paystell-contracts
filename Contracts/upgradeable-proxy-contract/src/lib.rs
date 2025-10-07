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
    contract, contractimpl, contracttype, Env, Address, Bytes, Symbol, Vec, Val, TryFromVal,
};
use crate::storage::Storage;
use crate::types::{UpgradeProposal, ImplementationRecord};
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
        let version_val: Val = env.invoke_contract(&proposal.new_impl, &schema_sym, Vec::new(&env));
        let schema_u32: u32 = u32::try_from_val(&env, &version_val).map_err(|_| ProxyError::ValidationFailed)?;
        if schema_u32 == 0 { return Err(ProxyError::ValidationFailed); }

        let prev_impl = store.current_impl().unwrap_or(proposal.new_impl.clone());
        let prev_version = store.version();
        store.set_implementation(&proposal.new_impl)?;
        store.record_history(ImplementationRecord { version: prev_version + 1, implementation: proposal.new_impl.clone(), prev: prev_impl });

        // --- Optional Migration Hook ---
        // If metadata first byte == 1 attempt to call `migrate()` (no args, no return required)
        if proposal.metadata.len() > 0 {
            let flag: u8 = proposal.metadata.get_unchecked(0);
            if flag == 1u8 {
                let migrate_sym = Symbol::new(&env, "migrate");
                let _migration_result: Val = env.invoke_contract(&proposal.new_impl, &migrate_sym, Vec::new(&env));
            }
        }

        proposal.executed = true;
        store.save_proposal(&proposal)?;
        Ok(())
    }

    fn rollback(env: Env) -> Result<(), ProxyError> {
        let store = Storage::new(&env);
        store.require_initialized()?;
        store.require_admin_auth()?;
        let prev = store.last_history_prev().ok_or(ProxyError::NoRollbackAvailable)?;
        store.set_implementation(&prev)?;
        Ok(())
    }

    fn get_current_implementation(env: Env) -> Result<Address, ProxyError> { Storage::new(&env).current_impl().ok_or(ProxyError::ImplementationNotSet) }
    fn get_version(env: Env) -> u64 { Storage::new(&env).version() }
    fn get_proposal(env: Env, proposal_id: u64) -> Result<UpgradeProposal, ProxyError> { Storage::new(&env).get_proposal(proposal_id) }

    fn forward(env: Env, func: Symbol, args: Vec<Val>) -> Result<Val, ProxyError> {
        let store = Storage::new(&env);
        store.require_initialized()?;
    let target = store.current_impl().ok_or(ProxyError::ImplementationNotSet)?;
        let res = env.invoke_contract(&target, &func, args);
        Ok(res)
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
}

#[cfg(test)]
mod test;