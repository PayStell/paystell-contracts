use soroban_sdk::{
    contracttype,
    Address, String, Vec, Symbol,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MerchantCategory {
    Retail,
    ECommerce,
    Hospitality,
    Professional,
    Entertainment,
    Other,
}

#[contracttype]
#[derive(Clone)]
pub struct Merchant {
    pub wallet_address: Address,
    pub active: bool,
    pub supported_tokens: Vec<Address>,
    pub name: String,
    pub description: String,
    pub contact_info: String,
    pub registration_timestamp: u64,
    pub last_activity_timestamp: u64,
    pub category: MerchantCategory,
    pub max_transaction_limit: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct ProfileUpdateData {
    pub update_name: bool,
    pub name: String,
    pub update_description: bool,
    pub description: String,
    pub update_contact_info: bool,
    pub contact_info: String,
    pub update_category: bool,
    pub category: MerchantCategory,
}

#[contracttype]
#[derive(Clone)]
pub struct PaymentOrder {
    pub merchant_address: Address,
    pub amount: i128,
    pub token: Address,
    pub nonce: u64,
    pub expiration: u64,
    pub order_id: String,
}

// Events
#[contracttype]
#[derive(Clone)]
pub struct MerchantRegisteredEvent {
    pub merchant: Address,
    pub name: String,
    pub category: MerchantCategory,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct ProfileUpdatedEvent {
    pub merchant: Address,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct MerchantDeactivatedEvent {
    pub merchant: Address,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct LimitsUpdatedEvent {
    pub merchant: Address,
    pub max_transaction_limit: i128,
    pub timestamp: u64,
}

// Event topics
pub fn merchant_registered_topic(env: &soroban_sdk::Env) -> Symbol {
    Symbol::new(env, "merchant_reg")
}

pub fn profile_updated_topic(env: &soroban_sdk::Env) -> Symbol {
    Symbol::new(env, "profile_upd")
}

pub fn merchant_deactivated_topic(env: &soroban_sdk::Env) -> Symbol {
    Symbol::new(env, "merchant_deact")
}

pub fn limits_updated_topic(env: &soroban_sdk::Env) -> Symbol {
    Symbol::new(env, "limits_upd")
} 