use soroban_sdk::{
    contracttype,
    Address, String, Vec, Map,
};

/// Optimized merchant data structure with packed fields
/// Uses bit flags for boolean states and efficient storage layout
#[contracttype]
#[derive(Clone)]
pub struct Merchant {
    pub wallet_address: Address,
    /// Packed flags: bit 0 = active, bits 1-31 reserved for future use
    pub flags: u32,
    /// Compact token storage using Map for O(1) lookups instead of Vec
    pub supported_tokens: Map<Address, bool>,
    /// Cached token count to avoid expensive Map iteration
    pub token_count: u32,
}

impl Merchant {
    pub fn new(env: &soroban_sdk::Env, wallet_address: Address) -> Self {
        Self {
            wallet_address,
            flags: 0x01, // Set active flag
            supported_tokens: Map::new(env),
            token_count: 0,
        }
    }

    pub fn is_active(&self) -> bool {
        (self.flags & 0x01) != 0
    }

    pub fn set_active(&mut self, active: bool) {
        if active {
            self.flags |= 0x01;
        } else {
            self.flags &= !0x01;
        }
    }

    pub fn add_token(&mut self, token: Address) -> bool {
        if !self.supported_tokens.get(token.clone()).unwrap_or(false) {
            self.supported_tokens.set(token, true);
            self.token_count += 1;
            true
        } else {
            false
        }
    }

    pub fn remove_token(&mut self, token: Address) -> bool {
        if self.supported_tokens.get(token.clone()).unwrap_or(false) {
            self.supported_tokens.set(token, false);
            self.token_count -= 1;
            true
        } else {
            false
        }
    }

    pub fn supports_token(&self, token: &Address) -> bool {
        self.supported_tokens.get(token.clone()).unwrap_or(false)
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
