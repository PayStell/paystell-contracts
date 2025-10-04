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
#[derive(Clone, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum PaymentStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

#[contracttype]
#[derive(Clone)]
pub struct PaymentRecord {
    pub payment_id: String,
    pub payer: Address,
    pub merchant: Address,
    pub amount: i128,
    pub token: Address,
    pub nonce: u64,
    pub order_id: String,
    pub status: PaymentStatus,
    pub created_at: u64,
    pub updated_at: u64,
    pub completed_at: Option<u64>,
    pub error_message: Option<String>,
}

#[contracttype]
#[derive(Clone)]
pub enum QueryFilter {
    ByMerchant(Address),
    ByPayer(Address),
}

#[contracttype]
#[derive(Clone)]
pub struct PaymentRecordQuery {
    pub filter: QueryFilter,
    pub from_timestamp: Option<u64>,
    pub to_timestamp: Option<u64>,
}