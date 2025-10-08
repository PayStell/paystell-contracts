use soroban_sdk::{
    contracttype,
    Address, String, Vec, Map, Symbol,
};

/// Merchant category enumeration
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

impl Merchant {
    pub fn add_token(&mut self, token: Address) -> bool {
        // Check if token already exists
        for existing_token in self.supported_tokens.iter() {
            if existing_token == token {
                return false; // Token already exists
            }
        }
        self.supported_tokens.push_back(token);
        true
    }

    pub fn remove_token(&mut self, token: Address) -> bool {
        let mut found = false;
        let mut new_tokens = Vec::new(&self.supported_tokens.env());
        
        for existing_token in self.supported_tokens.iter() {
            if existing_token != token {
                new_tokens.push_back(existing_token);
            } else {
                found = true;
            }
        }
        
        if found {
            self.supported_tokens = new_tokens;
        }
        found
    }

    pub fn supports_token(&self, token: &Address) -> bool {
        for existing_token in self.supported_tokens.iter() {
            if existing_token == *token {
                return true;
            }
        }
        false
    }
}

/// Optimized payment order with efficient data types
#[contracttype]
#[derive(Clone)]
pub struct PaymentOrder {
    pub merchant_address: Address,
    /// Use i64 instead of i128 for most payment amounts (sufficient for most use cases)
    /// Can be upgraded to i128 if needed for very large amounts
    pub amount: i64,
    pub token: Address,
    /// Use u32 for nonce (4 billion possible values, sufficient for most use cases)
    pub nonce: u32,
    /// Use u32 for expiration timestamp (valid until year 2106)
    pub expiration: u32,
    /// Use compact string representation
    pub order_id: String,
    pub fee_amount: i128,
}

/// Batch operation structures for gas optimization
#[contracttype]
#[derive(Clone)]
pub struct BatchMerchantRegistration {
    pub merchants: Vec<Address>,
}

#[contracttype]
#[derive(Clone)]
pub struct BatchTokenAddition {
    pub merchant: Address,
    pub tokens: Vec<Address>,
}

#[contracttype]
#[derive(Clone)]
pub struct BatchPayment {
    pub payer: Address,
    pub orders: Vec<PaymentOrder>,
    pub signatures: Vec<soroban_sdk::BytesN<64>>,
    pub merchant_public_key: soroban_sdk::BytesN<32>,
}

/// Gas estimation structures
#[contracttype]
#[derive(Clone)]
pub struct GasEstimate {
    pub base_gas: u64,
    pub per_item_gas: u64,
    pub total_estimated: u64,
}

/// Compact nonce tracking using bitmaps for better storage efficiency
#[contracttype]
#[derive(Clone)]
pub struct NonceTracker {
    /// Bitmap for tracking used nonces (each bit represents 8 consecutive nonces)
    pub nonce_bitmap: Map<u32, u32>,
    /// Highest nonce used for this merchant
    pub highest_nonce: u32,
}

impl NonceTracker {
    pub fn new(env: &soroban_sdk::Env) -> Self {
        Self {
            nonce_bitmap: Map::new(env),
            highest_nonce: 0,
        }
    }

    pub fn is_nonce_used(&self, nonce: u32) -> bool {
        let bitmap_index = nonce / 32;
        let bit_position = nonce % 32;
        let bitmap_word = self.nonce_bitmap.get(bitmap_index).unwrap_or(0);
        (bitmap_word & (1 << bit_position)) != 0
    }

    pub fn mark_nonce_used(&mut self, nonce: u32) {
        let bitmap_index = nonce / 32;
        let bit_position = nonce % 32;
        let current_word = self.nonce_bitmap.get(bitmap_index).unwrap_or(0);
        self.nonce_bitmap.set(bitmap_index, current_word | (1 << bit_position));
        
        if nonce > self.highest_nonce {
            self.highest_nonce = nonce;
        }
    }
}

#[contracttype]
#[derive(Clone)]
pub struct Fee {
    pub fee_rate: u64,
    pub fee_collector: Address,
    pub fee_token: Address,
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
    pub order_id: String,
    pub merchant_address: Address,
    pub payer_address: Address,
    pub token: Address,
    pub amount: i128,
    pub paid_at: u64,
    pub refunded_amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct MultiSigPaymentRecord {
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

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
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

// ===== NEW: Payment History and Analytics Types =====

/// Payment History Query Parameters
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct PaymentQueryParams {
    pub start_time: Option<u64>,
    pub end_time: Option<u64>,
    pub min_amount: Option<i128>,
    pub max_amount: Option<i128>,
    pub token: Option<Address>,
    pub limit: u32,
    pub offset: u32,
}

/// Payment History Statistics
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct PaymentStats {
    pub total_payments: u32,
    pub total_volume: i128,
    pub unique_payers: u32,
    pub average_payment: i128,
    pub first_payment_time: Option<u64>,
    pub last_payment_time: Option<u64>,
}

/// Merchant Payment Summary
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct MerchantPaymentSummary {
    pub merchant_address: Address,
    pub total_received: i128,
    pub payment_count: u32,
    pub refund_count: u32,
    pub total_refunded: i128,
    pub net_received: i128,
    pub active_since: u64,
    pub last_payment: Option<u64>,
}

/// Payer Payment Summary
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct PayerPaymentSummary {
    pub payer_address: Address,
    pub total_spent: i128,
    pub payment_count: u32,
    pub unique_merchants: u32,
    pub first_payment: Option<u64>,
    pub last_payment: Option<u64>,
}

/// Payment Index Entry (for efficient querying)
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct PaymentIndexEntry {
    pub order_id: String,
    pub timestamp: u64,
    pub amount: i128,
}

/// Time-based Payment Bucket (for temporal queries)
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct PaymentBucket {
    pub bucket_timestamp: u64, // Start of time bucket (e.g., start of day)
    pub payment_count: u32,
    pub total_volume: i128,
}

/// Compressed Payment Record (for archival)
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct CompressedPaymentRecord {
    pub order_id: String,
    pub merchant_address: Address,
    pub payer_address: Address,
    pub token: Address,
    pub amount: i128,
    pub paid_at: u64,
    pub status: u32, // 0: paid, 1: partially refunded, 2: fully refunded
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