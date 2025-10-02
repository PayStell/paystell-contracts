use soroban_sdk::{
    contracttype, log, Address, Env, Map, Symbol, Vec
};
use crate::{
    types::Merchant,
    error::PaymentError,
};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Initialized,
    Merchants,
    UsedNonces,
    Paused,
    PausedAdmin,
    PausedUntil,
}

impl DataKey {
    fn as_symbol(self, env: &Env) -> Symbol {
        match self {
            DataKey::Initialized => Symbol::new(env, "initialized"),
            DataKey::Merchants => Symbol::new(env, "merchants"),
            DataKey::UsedNonces => Symbol::new(env, "used_nonces"),
            DataKey::Paused => Symbol::new(env, "paused"),
            DataKey::PausedAdmin => Symbol::new(env, "paused_admin"),
            DataKey::PausedUntil => Symbol::new(env, "paused_until"),
        }
    }
}

pub struct Storage<'a> {
    env: &'a Env,
}

impl<'a> Storage<'a> {
    pub fn new(env: &'a Env) -> Self {
        Self { env }
    }

    pub fn set_initialized(&self) {
        self.env.storage().instance().set(
            &DataKey::Initialized.as_symbol(self.env), 
            &true)
    }

    pub fn is_initialized(&self) -> bool {
       self.env.storage().instance().has(&DataKey::Initialized.as_symbol(self.env)) 
    }

    pub fn set_pause_admin(&self, address: &Address) {
        self.env.storage().instance().set(
            &DataKey::PausedAdmin.as_symbol(self.env),
            &address,
        );
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

    pub fn is_paused(&self) -> bool  {
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

    pub fn save_merchant(&self, address: &Address, merchant: &Merchant) {
        let mut merchants = self.get_merchants_map();
        merchants.set(address.clone(), merchant.clone());
        self.env.storage().instance().set(
            &DataKey::Merchants.as_symbol(self.env),
            &merchants,
        );
    }

    pub fn get_merchant(&self, address: &Address) -> Result<Merchant, PaymentError> {
        let merchants = self.get_merchants_map();
        merchants.get(address.clone())
            .ok_or(PaymentError::MerchantNotFound)
    }

    pub fn is_nonce_used(&self, merchant: &Address, nonce: u64) -> bool {
        let nonces = self.get_merchant_nonces(merchant);
        nonces.contains(&nonce)
    }

    pub fn mark_nonce_used(&self, merchant: &Address, nonce: u64) {
        let mut nonces = self.get_merchant_nonces(merchant);
        nonces.push_back(nonce);
        let mut used_nonces = self.get_used_nonces_map();
        used_nonces.set(merchant.clone(), nonces);
        self.env.storage().instance().set(
            &DataKey::UsedNonces.as_symbol(self.env),
            &used_nonces,
        );
    }

    fn get_merchants_map(&self) -> Map<Address, Merchant> {
        self.env.storage().instance()
            .get(&DataKey::Merchants.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env))
    }

    fn get_used_nonces_map(&self) -> Map<Address, Vec<u64>> {
        self.env.storage().instance()
            .get(&DataKey::UsedNonces.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env))
    }

    fn get_merchant_nonces(&self, merchant: &Address) -> Vec<u64> {
        let used_nonces = self.get_used_nonces_map();
        used_nonces.get(merchant.clone())
            .unwrap_or_else(|| Vec::new(self.env))
    }
} 