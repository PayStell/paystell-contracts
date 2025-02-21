use soroban_sdk::{
    contracttype,
    Env, Symbol, Map, Vec, Address, String,
};
use crate::{
    types::{Merchant, PaymentLink},
    error::PaymentError,
};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Merchants,
    PaymentLinks,
    ProcessedPayments,
}

impl DataKey {
    fn as_symbol(self, env: &Env) -> Symbol {
        match self {
            DataKey::Merchants => Symbol::new(env, "merchants"),
            DataKey::PaymentLinks => Symbol::new(env, "payment_links"),
            DataKey::ProcessedPayments => Symbol::new(env, "processed_payments"),
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

    pub fn save_payment_link(&self, id: &String, link: &PaymentLink) {
        let mut links = self.get_payment_links_map();
        links.set(id.clone(), link.clone());
        self.env.storage().instance().set(
            &DataKey::PaymentLinks.as_symbol(self.env),
            &links,
        );
    }

    pub fn get_payment_link(&self, id: &String) -> Result<PaymentLink, PaymentError> {
        let links = self.get_payment_links_map();
        links.get(id.clone())
            .ok_or(PaymentError::InvalidPaymentLink)
    }

    pub fn is_payment_processed(&self, payment_id: &String) -> bool {
        let processed = self.get_processed_payments();
        processed.contains(payment_id)
    }

    pub fn mark_payment_processed(&self, payment_id: &String) {
        let mut processed = self.get_processed_payments();
        processed.push_back(payment_id.clone());
        self.env.storage().instance().set(
            &DataKey::ProcessedPayments.as_symbol(self.env),
            &processed,
        );
    }

    fn get_merchants_map(&self) -> Map<Address, Merchant> {
        self.env.storage().instance()
            .get(&DataKey::Merchants.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env))
    }

    fn get_payment_links_map(&self) -> Map<String, PaymentLink> {
        self.env.storage().instance()
            .get(&DataKey::PaymentLinks.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env))
    }

    fn get_processed_payments(&self) -> Vec<String> {
        self.env.storage().instance()
            .get(&DataKey::ProcessedPayments.as_symbol(self.env))
            .unwrap_or_else(|| Vec::new(self.env))
    }
} 