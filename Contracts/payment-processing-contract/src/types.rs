use soroban_sdk::{
    contracttype,
    Address, String, Vec,
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
#[derive(Clone)]
pub struct PaymentRecord {
    pub order_id: String,
    pub merchant_address: Address,
    pub payer_address: Address,
    pub token: Address,
    pub amount: i128,
    pub paid_at: u64,
    pub refunded_amount: i128,
}

#[contracttype]
#[derive(Clone)]
pub enum RefundStatus {
    Pending,
    Approved,
    Rejected,
    Completed,
}

#[contracttype]
#[derive(Clone)]
pub struct RefundRequest {
    pub refund_id: String,
    pub order_id: String,
    pub merchant_address: Address,
    pub payer_address: Address,
    pub token: Address,
    pub amount: i128,
    pub reason: String,
    pub requested_at: u64,
    pub status: RefundStatus,
    pub approved_by: Option<Address>,
}