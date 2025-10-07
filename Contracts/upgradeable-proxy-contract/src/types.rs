use soroban_sdk::{contracttype, Address, Bytes, Vec};

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

// helper constructors intentionally omitted to avoid test-only generation patterns
