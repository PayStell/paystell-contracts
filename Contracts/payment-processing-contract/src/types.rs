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
pub struct PaymentLink {
    pub merchant_id: Address,
    pub amount: i128,
    pub token: Address,
    pub description: String,
    pub active: bool,
} 