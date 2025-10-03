use soroban_sdk::{
    contracttype,
    Env, Symbol, Map, Vec, Address,
};
use crate::{
    types::{Merchant, PaymentRecord, RefundRequest},
    error::PaymentError,
};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Merchants,
    UsedNonces,
    Payments,
    Refunds,
    Admin,
}

impl DataKey {
    fn as_symbol(self, env: &Env) -> Symbol {
        match self {
            DataKey::Merchants => Symbol::new(env, "merchants"),
            DataKey::UsedNonces => Symbol::new(env, "used_nonces"),
            DataKey::Payments => Symbol::new(env, "payments"),
            DataKey::Refunds => Symbol::new(env, "refunds"),
            DataKey::Admin => Symbol::new(env, "admin"),
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

    // Admin management
    pub fn set_admin(&self, admin: &Address) {
        self.env.storage().instance().set(
            &DataKey::Admin.as_symbol(self.env),
            admin,
        );
    }

    pub fn get_admin(&self) -> Option<Address> {
        self.env.storage().instance()
            .get(&DataKey::Admin.as_symbol(self.env))
    }

    // Payment records
    pub fn save_payment(&self, record: &PaymentRecord) {
        let mut payments = self.get_payments_map();
        payments.set(record.order_id.clone(), record.clone());
        self.env.storage().instance().set(
            &DataKey::Payments.as_symbol(self.env),
            &payments,
        );
    }

    pub fn get_payment(&self, order_id: &soroban_sdk::String) -> Result<PaymentRecord, PaymentError> {
        let payments = self.get_payments_map();
        payments.get(order_id.clone()).ok_or(PaymentError::PaymentNotFound)
    }

    pub fn update_payment(&self, record: &PaymentRecord) {
        let mut payments = self.get_payments_map();
        payments.set(record.order_id.clone(), record.clone());
        self.env.storage().instance().set(
            &DataKey::Payments.as_symbol(self.env),
            &payments,
        );
    }

    // Refund requests
    pub fn save_refund(&self, request: &RefundRequest) {
        let mut refunds = self.get_refunds_map();
        refunds.set(request.refund_id.clone(), request.clone());
        self.env.storage().instance().set(
            &DataKey::Refunds.as_symbol(self.env),
            &refunds,
        );
    }

    pub fn get_refund(&self, refund_id: &soroban_sdk::String) -> Result<RefundRequest, PaymentError> {
        let refunds = self.get_refunds_map();
        refunds.get(refund_id.clone()).ok_or(PaymentError::RefundNotFound)
    }

    pub fn update_refund(&self, request: &RefundRequest) {
        let mut refunds = self.get_refunds_map();
        refunds.set(request.refund_id.clone(), request.clone());
        self.env.storage().instance().set(
            &DataKey::Refunds.as_symbol(self.env),
            &refunds,
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

    fn get_payments_map(&self) -> Map<soroban_sdk::String, PaymentRecord> {
        self.env.storage().instance()
            .get(&DataKey::Payments.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env))
    }

    fn get_refunds_map(&self) -> Map<soroban_sdk::String, RefundRequest> {
        self.env.storage().instance()
            .get(&DataKey::Refunds.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env))
    }
} 