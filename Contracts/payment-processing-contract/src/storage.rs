use soroban_sdk::{
    contracttype,
    Env, Symbol, Map, Vec, Address,
};
use crate::{
    types::{Merchant, NonceTracker},
    error::PaymentError,
};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Merchants,
    NonceTrackers,
    // Cache keys for frequently accessed data
    MerchantCache,
    TokenCache,
}

impl DataKey {
    fn as_symbol(self, env: &Env) -> Symbol {
        match self {
            DataKey::Merchants => Symbol::new(env, "merchants"),
            DataKey::NonceTrackers => Symbol::new(env, "nonce_trackers"),
            DataKey::MerchantCache => Symbol::new(env, "merchant_cache"),
            DataKey::TokenCache => Symbol::new(env, "token_cache"),
        }
    }
}

/// Optimized storage with efficient operations
pub struct Storage<'a> {
    env: &'a Env,
}

impl<'a> Storage<'a> {
    pub fn new(env: &'a Env) -> Self {
        Self { env }
    }

    /// Get merchants map
    fn get_merchants_map(&self) -> Map<Address, Merchant> {
        self.env.storage().instance()
            .get(&DataKey::Merchants.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env))
    }

    /// Get nonce trackers map
    fn get_nonce_trackers_map(&self) -> Map<Address, NonceTracker> {
        self.env.storage().instance()
            .get(&DataKey::NonceTrackers.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env))
    }

    /// Save merchant with optimized storage
    pub fn save_merchant(&self, address: &Address, merchant: &Merchant) {
        let mut merchants = self.get_merchants_map();
        merchants.set(address.clone(), merchant.clone());
        
        // Persist to storage
        self.env.storage().instance().set(
            &DataKey::Merchants.as_symbol(self.env),
            &merchants,
        );
    }

    /// Get merchant
    pub fn get_merchant(&self, address: &Address) -> Result<Merchant, PaymentError> {
        let merchants = self.get_merchants_map();
        merchants.get(address.clone())
            .ok_or(PaymentError::MerchantNotFound)
    }

    /// Check if nonce is used with bitmap optimization
    pub fn is_nonce_used(&self, merchant: &Address, nonce: u32) -> bool {
        let trackers = self.get_nonce_trackers_map();
        if let Some(tracker) = trackers.get(merchant.clone()) {
            tracker.is_nonce_used(nonce)
        } else {
            false
        }
    }

    /// Mark nonce as used with bitmap optimization
    pub fn mark_nonce_used(&self, merchant: &Address, nonce: u32) {
        let mut trackers = self.get_nonce_trackers_map();
        let mut tracker = trackers.get(merchant.clone())
            .unwrap_or_else(|| NonceTracker::new(self.env));
        
        tracker.mark_nonce_used(nonce);
        trackers.set(merchant.clone(), tracker);
        
        // Persist to storage
        self.env.storage().instance().set(
            &DataKey::NonceTrackers.as_symbol(self.env),
            &trackers,
        );
    }

    /// Batch save multiple merchants (gas optimization)
    pub fn batch_save_merchants(&self, merchants_data: &[(Address, Merchant)]) {
        let mut merchants = self.get_merchants_map();
        
        for (address, merchant) in merchants_data {
            merchants.set(address.clone(), merchant.clone());
        }
        
        // Single storage write for all merchants
        self.env.storage().instance().set(
            &DataKey::Merchants.as_symbol(self.env),
            &merchants,
        );
    }

    /// Batch mark multiple nonces as used (gas optimization)
    pub fn batch_mark_nonces_used(&self, merchant: &Address, nonces: &[u32]) {
        let mut trackers = self.get_nonce_trackers_map();
        let mut tracker = trackers.get(merchant.clone())
            .unwrap_or_else(|| NonceTracker::new(self.env));
        
        for &nonce in nonces {
            tracker.mark_nonce_used(nonce);
        }
        
        trackers.set(merchant.clone(), tracker);
        
        // Single storage write
        self.env.storage().instance().set(
            &DataKey::NonceTrackers.as_symbol(self.env),
            &trackers,
        );
    }

    /// Get merchant count (for gas estimation)
    pub fn get_merchant_count(&self) -> u32 {
        let merchants = self.get_merchants_map();
        merchants.len() as u32
    }

    /// Get nonce tracker for a merchant
    pub fn get_nonce_tracker(&self, merchant: &Address) -> Option<NonceTracker> {
        let trackers = self.get_nonce_trackers_map();
        trackers.get(merchant.clone())
    }
} 