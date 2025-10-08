// types.rs
use soroban_sdk::{contracttype, Address, Bytes, Vec, Symbol};

#[contracttype]
#[derive(Clone)]
pub struct UpgradeProposal {
    pub id: u64,
    pub new_impl: Address,
    pub metadata: Bytes,
    pub proposer: Address,
    pub approvals: Vec<Address>,
    pub created_at: u64,
    pub executable_at: u64,
    pub executed: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct ImplementationRecord {
    pub version: u64,
    pub implementation: Address,
    pub prev: Address,
}

#[contracttype]
#[derive(Clone)]
pub struct UpgradeImpact {
    pub critical_funcs_tested: Vec<Symbol>,
    pub failed_sims: Vec<Symbol>,
    pub estimated_gas_increase: u64, // Rough estimate from sim calls
}

#[contracttype]
#[derive(Clone)]
pub struct MigrationProgress {
    pub phase: u32,
    pub total_phases: u32,
    pub completed: bool,
}

// helper constructors intentionally omitted to avoid test-only generation patterns