use soroban_sdk::{
    contracttype,
    Env, Symbol, Map, Vec, Address,
};
use crate::{
    types::Merchant,
    error::PaymentError,
};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Merchants,
    UsedNonces,
}

impl DataKey {
    fn as_symbol(self, env: &Env) -> Symbol {
        match self {
            DataKey::Merchants => Symbol::new(env, "merchants"),
            DataKey::UsedNonces => Symbol::new(env, "used_nonces"),
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