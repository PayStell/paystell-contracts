use crate::{
    types::{Merchant, NonceTracker, Fee},
    error::PaymentError,
};
use soroban_sdk::{contracttype, log, Address, Env, Map, Symbol};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Initialized,
    Merchants,
    NonceTrackers,
    // Cache keys for frequently accessed data
    MerchantCache,
    TokenCache,
    // Pause management keys
    Paused,
    PausedAdmin,
    PausedUntil,
    Admin,
    Fee,
}

impl DataKey {
    fn as_symbol(self, env: &Env) -> Symbol {
        match self {
            DataKey::Initialized => Symbol::new(env, "initialized"),
            DataKey::Merchants => Symbol::new(env, "merchants"),
            DataKey::NonceTrackers => Symbol::new(env, "nonce_trackers"),
            DataKey::MerchantCache => Symbol::new(env, "merchant_cache"),
            DataKey::TokenCache => Symbol::new(env, "token_cache"),
            DataKey::Paused => Symbol::new(env, "paused"),
            DataKey::PausedAdmin => Symbol::new(env, "paused_admin"),
            DataKey::PausedUntil => Symbol::new(env, "paused_until"),
            DataKey::Admin => Symbol::new(env, "admin"),
            DataKey::Fee => Symbol::new(env, "fee"),
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
        self.env
            .storage()
            .instance()
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

    pub fn set_admin(&self, admin: &Address) {
        self.env
            .storage()
            .instance()
            .set(&DataKey::Admin.as_symbol(self.env), &admin);
    }

    fn require_admin(&self, admin: &Address) -> Result<(), PaymentError> {
        let stored_admin: Address = self
            .env
            .storage()
            .instance()
            .get(&DataKey::Admin.as_symbol(self.env))
            .ok_or(PaymentError::AdminNotFound)?;

        if stored_admin != *admin {
            return Err(PaymentError::NotAuthorized);
        }

        Ok(())
    }

    pub fn set_fee_info(&self, fee: &Fee, admin: &Address) -> Result<(), PaymentError> {
        self.require_admin(admin)?;
        if fee.fee_rate > 10 {
            return Err(PaymentError::InvalidFeeRate);
        }
        self.env
            .storage()
            .instance()
            .set(&DataKey::Fee.as_symbol(self.env), &fee.clone());
        self.env.events().publish(
            ("fee_info_set",),
            (
                fee.fee_rate,
                fee.fee_collector.clone(),
                fee.fee_token.clone(),
            ),
        );
        Ok(())
    }

    pub fn get_fee_rate(&self) -> u64 {
        let fee_info: Option<Fee> = self
            .env
            .storage()
            .instance()
            .get(&DataKey::Fee.as_symbol(self.env));
        match fee_info {
            Some(fee) => fee.fee_rate,
            None => 0,
        }
    }

    pub fn get_fee_collector(&self) -> Option<Address> {
        let fee_info: Option<Fee> = self
            .env
            .storage()
            .instance()
            .get(&DataKey::Fee.as_symbol(self.env));
        fee_info.map(|fee| fee.fee_collector)
    }

    pub fn get_fee_token(&self) -> Option<Address> {
        let fee_info: Option<Fee> = self
            .env
            .storage()
            .instance()
            .get(&DataKey::Fee.as_symbol(self.env));
        fee_info.map(|fee| fee.fee_token)
    }

    pub fn calculate_fee(&self, amount: i128) -> i128 {
        let rate = i128::from(self.get_fee_rate());
        let quotient = amount / 100;
        let remainder = amount % 100;
        quotient * rate + (remainder * rate) / 100
    }

    pub fn get_admin(&self) -> Option<Address> {
        self.env
            .storage()
            .instance()
            .get(&DataKey::Admin.as_symbol(self.env))
    }

    // Pause Management Methods
    pub fn set_pause_admin_internal(&self, admin: Address, new_admin: Address) -> Result<(), PaymentError> {
        self.require_admin(&admin)?;

        self.env.storage().instance().set(
            &DataKey::PausedAdmin.as_symbol(self.env),
            &new_admin,
        );
        Ok(())
    }

    pub fn set_pause_until(&self, timestamp: u64) {
        self.env.storage().instance().set(
            &DataKey::PausedUntil.as_symbol(self.env),
            &timestamp,
        );
    }

    pub fn get_pause_admin(&self) -> Result<Address, PaymentError> {
        self.env.storage().instance().get(
            &DataKey::PausedAdmin.as_symbol(self.env),
        ).ok_or(PaymentError::AdminNotFound)
    }

    pub fn set_pause(&self) {
        self.env.storage().instance().set(
            &DataKey::Paused.as_symbol(self.env),
            &true,
        );
    }

    pub fn set_unpause(&self) {
        self.env.storage().instance().set(
            &DataKey::Paused.as_symbol(self.env),
            &false,
        );
    }

    pub fn is_paused(&self) -> bool {
        let pause = self.env.storage().instance()
            .get(&DataKey::Paused.as_symbol(self.env),)
            .unwrap_or(false);
        log!(&self.env, "pause: {}", pause);
        

        let pause_until: u64 = self.env.storage().instance()
            .get(&DataKey::PausedUntil.as_symbol(self.env),)
            .unwrap_or(0);
        log!(&self.env, "pause_until: {}", pause_until);

        let current_time = self.env.ledger().timestamp();

        if !pause && (pause_until == 0 || current_time > pause_until) {
            return false
        } 
        true
    }
}
