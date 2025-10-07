use soroban_sdk::{Env, Address, Vec, Map};
use crate::{DataKey, error::ProxyError, types::{UpgradeProposal, ImplementationRecord}};

pub struct Storage<'a> { env: &'a Env }

impl<'a> Storage<'a> {
    pub fn new(env: &'a Env) -> Self { Self { env } }

    pub fn is_initialized(&self) -> bool { self.env.storage().instance().has(&DataKey::Admins) }
    pub fn require_initialized(&self) -> Result<(), ProxyError> { if self.is_initialized() { Ok(()) } else { Err(ProxyError::NotInitialized) } }

    pub fn init(&self, admins: Vec<Address>, threshold: u32, delay: u64) {
        self.env.storage().instance().set(&DataKey::Admins, &admins);
        self.env.storage().instance().set(&DataKey::Threshold, &threshold);
        self.env.storage().instance().set(&DataKey::Delay, &delay);
        // implementation not set initially
        let version: u64 = 0; self.env.storage().instance().set(&DataKey::Version, &version);
        let seq: u64 = 0; self.env.storage().instance().set(&DataKey::ProposalSeq, &seq);
        let history: Vec<ImplementationRecord> = Vec::new(self.env); self.env.storage().instance().set(&DataKey::History, &history);
        let proposals: Map<u64, UpgradeProposal> = Map::new(self.env); self.env.storage().instance().set(&DataKey::Proposals, &proposals);
        // Initialize last invoker to first admin as fallback
        if admins.len() > 0 {
            self.env.storage().instance().set(&DataKey::LastInvoker, &admins.get_unchecked(0));
        }
    }

    pub fn require_admin_auth(&self) -> Result<(), ProxyError> {
        // Accept that any listed admin is authorized; we require_auth on provided address externally
        if !self.is_initialized() { return Err(ProxyError::NotInitialized); }
        Ok(())
    }

    pub fn is_admin(&self, addr: &Address) -> bool {
        let admins: Vec<Address> = self.env.storage().instance().get(&DataKey::Admins).unwrap();
        admins.contains(addr)
    }

    pub fn threshold(&self) -> u32 { self.env.storage().instance().get(&DataKey::Threshold).unwrap() }
    pub fn delay_seconds(&self) -> u64 { self.env.storage().instance().get(&DataKey::Delay).unwrap() }
    pub fn version(&self) -> u64 { self.env.storage().instance().get(&DataKey::Version).unwrap_or(0) }
    pub fn current_impl(&self) -> Option<Address> { self.env.storage().instance().get(&DataKey::Impl) }

    pub fn next_proposal_id(&self) -> u64 {
        let mut seq: u64 = self.env.storage().instance().get(&DataKey::ProposalSeq).unwrap();
        seq += 1; self.env.storage().instance().set(&DataKey::ProposalSeq, &seq); seq
    }

    pub fn save_proposal(&self, proposal: &UpgradeProposal) -> Result<(), ProxyError> {
        let mut map: Map<u64, UpgradeProposal> = self.env.storage().instance().get(&DataKey::Proposals).unwrap();
        map.set(proposal.id, proposal.clone());
        self.env.storage().instance().set(&DataKey::Proposals, &map);
        Ok(())
    }

    pub fn get_proposal(&self, id: u64) -> Result<UpgradeProposal, ProxyError> {
        let map: Map<u64, UpgradeProposal> = self.env.storage().instance().get(&DataKey::Proposals).unwrap();
        map.get(id).ok_or(ProxyError::ProposalNotFound)
    }

    pub fn set_implementation(&self, new_impl: &Address) -> Result<(), ProxyError> {
        self.env.storage().instance().set(&DataKey::Impl, new_impl);
        // Note: Version increment is handled in record_history to avoid double increment
        Ok(())
    }

    pub fn record_history(&self, rec: ImplementationRecord) {
        let mut history: Vec<ImplementationRecord> = self.env.storage().instance().get(&DataKey::History).unwrap();
        history.push_back(rec);
        self.env.storage().instance().set(&DataKey::History, &history);
        // Update version to match the recorded history entry
        self.env.storage().instance().set(&DataKey::Version, &rec.version);
    }

    pub fn last_history_prev(&self) -> Option<Address> {
        let history: Vec<ImplementationRecord> = self.env.storage().instance().get(&DataKey::History).unwrap();
        if history.len() == 0 { return None; }
        let last = history.get_unchecked(history.len() - 1);
        Some(last.prev.clone())
    }

    pub fn set_last_invoker(&self, invoker: &Address) {
        self.env.storage().instance().set(&DataKey::LastInvoker, invoker);
    }

    pub fn last_invoker(&self) -> Result<Address, ProxyError> {
        self.env.storage().instance().get(&DataKey::LastInvoker)
            .ok_or(ProxyError::StorageError)
    }
}
