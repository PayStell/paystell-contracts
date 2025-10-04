use soroban_sdk::{
    contracttype,
    Address, String, Vec, Map,
};

#[contracttype]
#[derive(Clone)]
pub struct Merchant {
    pub wallet_address: Address,
    pub active: bool,
    pub supported_tokens: Vec<Address>,
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

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum PaymentStatus {
    Pending,
    Executed,
    Cancelled,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct MultiSigPayment {
    pub payment_id: u128,
    pub amount: i128,
    pub token: Address,
    pub recipient: Address,
    pub signers: Vec<Address>,
    pub threshold: u32,
    pub signatures: Map<Address, bool>,
    pub status: PaymentStatus,
    pub expiry: u64,
    pub created_at: u64,
    pub reason: Option<String>, // For cancellation reason
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct PaymentRecord {
    pub payment_id: u128,
    pub amount: i128,
    pub token: Address,
    pub recipient: Address,
    pub signers: Vec<Address>,
    pub threshold: u32,
    pub status: PaymentStatus,
    pub executed_at: u64,
    pub executor: Option<Address>, // Who executed the payment
    pub reason: Option<String>, // Cancellation reason if applicable
}