use soroban_sdk::{contracttype, Address, String, Vec};

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
pub struct Fee {
    pub fee_rate: u64,
    pub fee_collector: Address,
    pub fee_token: Address,
}
