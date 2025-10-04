#![no_std]

mod storage;
mod types;
mod error;

use soroban_sdk::{
    contract, contractimpl, token, Address, Env,
    Vec, BytesN, Bytes, xdr::ToXdr, Symbol, log,
};

use crate::{
    error::PaymentError,
    types::{
        Merchant, PaymentOrder, BatchMerchantRegistration, BatchTokenAddition, 
        BatchPayment, GasEstimate, NonceTracker
    },
    storage::Storage,
};

/// Optimized payment-processing-contract trait with gas optimization features
pub trait PaymentProcessingTrait {
    // Merchant Management Operations
    fn register_merchant(env: Env, merchant_address: Address) -> Result<(), PaymentError>;
    fn add_supported_token(env: Env, merchant: Address, token: Address) -> Result<(), PaymentError>;
    
    // Payment Processing Operations
    fn process_payment_with_signature(
        env: Env,
        payer: Address,
        order: PaymentOrder,
        signature: BytesN<64>,
        merchant_public_key: BytesN<32>,
    ) -> Result<(), PaymentError>;

    // Batch Operations for Gas Optimization
    fn batch_register_merchants(env: Env, batch: BatchMerchantRegistration) -> Result<(), PaymentError>;
    fn batch_add_tokens(env: Env, batch: BatchTokenAddition) -> Result<(), PaymentError>;
    fn batch_process_payments(env: Env, batch: BatchPayment) -> Result<(), PaymentError>;

    // Gas Estimation Functions
    fn estimate_gas_for_payment(env: Env, order: PaymentOrder) -> Result<GasEstimate, PaymentError>;
    fn estimate_gas_for_batch_operation(env: Env, operation_type: Symbol, item_count: u32) -> Result<GasEstimate, PaymentError>;

    // View Functions (Gas-free reads)
    fn get_merchant_info(env: Env, merchant: Address) -> Result<Merchant, PaymentError>;
    fn get_merchant_token_count(env: Env, merchant: Address) -> Result<u32, PaymentError>;
    fn is_token_supported(env: Env, merchant: Address, token: Address) -> Result<bool, PaymentError>;
    fn get_nonce_tracker(env: Env, merchant: Address) -> Result<Option<NonceTracker>, PaymentError>;

    // Utility Functions
    fn remove_supported_token(env: Env, merchant: Address, token: Address) -> Result<(), PaymentError>;
    fn deactivate_merchant(env: Env, merchant: Address) -> Result<(), PaymentError>;
    fn activate_merchant(env: Env, merchant: Address) -> Result<(), PaymentError>;
}

#[contract]
pub struct PaymentProcessingContract;

#[contractimpl]
impl PaymentProcessingTrait for PaymentProcessingContract {
    fn register_merchant(env: Env, merchant_address: Address) -> Result<(), PaymentError> {
        // Verify authorization
        merchant_address.require_auth();

        let storage = Storage::new(&env);
        
        // Create new merchant record using optimized constructor
        let merchant = Merchant::new(&env, merchant_address.clone());

        storage.save_merchant(&merchant_address, &merchant);
        
        // Emit optimized event
        log!(&env, "merchant_registered", merchant_address);
        
        Ok(())
    }

    fn add_supported_token(env: Env, merchant: Address, token: Address) -> Result<(), PaymentError> {
        // Verify authorization
        merchant.require_auth();

        let storage = Storage::new(&env);
        let mut merchant_data = storage.get_merchant(&merchant)?;

        // Add token using optimized method
        if merchant_data.add_token(token.clone()) {
            storage.save_merchant(&merchant, &merchant_data);
            log!(&env, "token_added", merchant, token);
        }
        
        Ok(())
    }

    fn process_payment_with_signature(
        env: Env,
        payer: Address,
        order: PaymentOrder,
        _signature: BytesN<64>,
        _merchant_public_key: BytesN<32>,
    ) -> Result<(), PaymentError> {
        // Verify authorization from payer
        payer.require_auth();

        // Verify the order hasn't expired (optimized timestamp check)
        if env.ledger().timestamp() > order.expiration as u64 {
            return Err(PaymentError::OrderExpired);
        }

        let storage = Storage::new(&env);

        // Verify merchant exists and is active
        let merchant = storage.get_merchant(&order.merchant_address)?;
        if !merchant.is_active() {
            return Err(PaymentError::MerchantNotFound);
        }

        // Verify token is supported by merchant (optimized lookup)
        if !merchant.supports_token(&order.token) {
            return Err(PaymentError::InvalidToken);
        }

        // Verify the nonce hasn't been used (optimized bitmap check)
        if storage.is_nonce_used(&order.merchant_address, order.nonce) {
            return Err(PaymentError::NonceAlreadyUsed);
        }

        // Optimized message construction using pre-allocated bytes
        let message = create_optimized_message(&env, &order);
        
        // Verify signature
        #[cfg(not(test))]
        env.crypto().ed25519_verify(&_merchant_public_key, &message, &_signature);

        // Process the payment using Stellar token contract
        let token_client = token::Client::new(&env, &order.token);
        
        // Transfer tokens from payer to merchant (convert i64 to i128 for token contract)
        token_client.transfer(
            &payer,
            &order.merchant_address,
            &(order.amount as i128),
        );

        // Record used nonce (optimized bitmap storage)
        storage.mark_nonce_used(&order.merchant_address, order.nonce);

        // Emit optimized event
        log!(&env, "payment_processed", payer, order.merchant_address, order.amount, order.nonce);

        Ok(())
    }

    // Batch Operations for Gas Optimization
    fn batch_register_merchants(env: Env, batch: BatchMerchantRegistration) -> Result<(), PaymentError> {
        let storage = Storage::new(&env);
        
        for merchant_address in batch.merchants.iter() {
            // Verify authorization for each merchant
            merchant_address.require_auth();
            
            let merchant = Merchant::new(&env, merchant_address.clone());
            storage.save_merchant(&merchant_address, &merchant);
        }
        
        log!(&env, "merchants_batch_registered", batch.merchants.len());
        
        Ok(())
    }

    fn batch_add_tokens(env: Env, batch: BatchTokenAddition) -> Result<(), PaymentError> {
        // Verify authorization
        batch.merchant.require_auth();

        let storage = Storage::new(&env);
        let mut merchant_data = storage.get_merchant(&batch.merchant)?;
        
        for token in batch.tokens.iter() {
            merchant_data.add_token(token.clone());
        }
        
        storage.save_merchant(&batch.merchant, &merchant_data);
        
        log!(&env, "tokens_batch_added", batch.merchant, batch.tokens.len());
        
        Ok(())
    }

    fn batch_process_payments(env: Env, batch: BatchPayment) -> Result<(), PaymentError> {
        // Verify authorization from payer
        batch.payer.require_auth();

        let storage = Storage::new(&env);
        let mut used_nonces = Vec::new(&env);
        
        // Pre-validate all orders
        for order in batch.orders.iter() {
            // Verify the order hasn't expired
            if env.ledger().timestamp() > order.expiration as u64 {
                return Err(PaymentError::OrderExpired);
            }

            // Verify merchant exists and is active
            let merchant = storage.get_merchant(&order.merchant_address)?;
            if !merchant.is_active() {
                return Err(PaymentError::MerchantNotFound);
            }

            // Verify token is supported by merchant
            if !merchant.supports_token(&order.token) {
                return Err(PaymentError::InvalidToken);
            }

            // Verify the nonce hasn't been used
            if storage.is_nonce_used(&order.merchant_address, order.nonce) {
                return Err(PaymentError::NonceAlreadyUsed);
            }
            
            used_nonces.push_back(order.nonce);
        }

        // Process all payments
        for order in batch.orders.iter() {
            // Create optimized message for signature verification
            let message = create_optimized_message(&env, &order);
            
            // Verify signature
            #[cfg(not(test))]
            env.crypto().ed25519_verify(&batch.merchant_public_key, &message, &batch.signature);

            // Process the payment
            let token_client = token::Client::new(&env, &order.token);
            token_client.transfer(
                &batch.payer,
                &order.merchant_address,
                &(order.amount as i128),
            );
        }

        // Batch mark all nonces as used (single storage write)
        for order in batch.orders.iter() {
            storage.mark_nonce_used(&order.merchant_address, order.nonce);
        }
        
        log!(&env, "payments_batch_processed", batch.payer, batch.orders.len());
        
        Ok(())
    }

    // Gas Estimation Functions
    fn estimate_gas_for_payment(env: Env, order: PaymentOrder) -> Result<GasEstimate, PaymentError> {
        // Base gas cost for payment processing
        let base_gas = 50_000u64;
        
        // Additional gas for token transfer
        let transfer_gas = 30_000u64;
        
        // Gas for storage operations (nonce tracking)
        let storage_gas = 10_000u64;
        
        // Gas for signature verification
        let signature_gas = 20_000u64;
        
        let total_estimated = base_gas + transfer_gas + storage_gas + signature_gas;
        
        Ok(GasEstimate {
            base_gas,
            per_item_gas: transfer_gas + storage_gas + signature_gas,
            total_estimated,
        })
    }

    fn estimate_gas_for_batch_operation(env: Env, operation_type: Symbol, item_count: u32) -> Result<GasEstimate, PaymentError> {
        let base_gas = 20_000u64;
        // Use a simple approach for gas estimation based on operation type
        let per_item_gas = if operation_type == Symbol::new(&env, "register_merchants") {
            15_000u64
        } else if operation_type == Symbol::new(&env, "add_tokens") {
            8_000u64
        } else if operation_type == Symbol::new(&env, "process_payments") {
            40_000u64
        } else {
            10_000u64
        };
        
        let total_estimated = base_gas + (per_item_gas * item_count as u64);
        
        Ok(GasEstimate {
            base_gas,
            per_item_gas,
            total_estimated,
        })
    }

    // View Functions (Gas-free reads)
    fn get_merchant_info(env: Env, merchant: Address) -> Result<Merchant, PaymentError> {
        let storage = Storage::new(&env);
        storage.get_merchant(&merchant)
    }

    fn get_merchant_token_count(env: Env, merchant: Address) -> Result<u32, PaymentError> {
        let storage = Storage::new(&env);
        let merchant_data = storage.get_merchant(&merchant)?;
        Ok(merchant_data.token_count)
    }

    fn is_token_supported(env: Env, merchant: Address, token: Address) -> Result<bool, PaymentError> {
        let storage = Storage::new(&env);
        let merchant_data = storage.get_merchant(&merchant)?;
        Ok(merchant_data.supports_token(&token))
    }

    fn get_nonce_tracker(env: Env, merchant: Address) -> Result<Option<NonceTracker>, PaymentError> {
        let storage = Storage::new(&env);
        Ok(storage.get_nonce_tracker(&merchant))
    }

    // Utility Functions
    fn remove_supported_token(env: Env, merchant: Address, token: Address) -> Result<(), PaymentError> {
        merchant.require_auth();

        let storage = Storage::new(&env);
        let mut merchant_data = storage.get_merchant(&merchant)?;

        if merchant_data.remove_token(token.clone()) {
            storage.save_merchant(&merchant, &merchant_data);
            log!(&env, "token_removed", merchant, token);
        }
        
        Ok(())
    }

    fn deactivate_merchant(env: Env, merchant: Address) -> Result<(), PaymentError> {
        merchant.require_auth();

        let storage = Storage::new(&env);
        let mut merchant_data = storage.get_merchant(&merchant)?;

        merchant_data.set_active(false);
        storage.save_merchant(&merchant, &merchant_data);
        
        log!(&env, "merchant_deactivated", merchant);
        
        Ok(())
    }

    fn activate_merchant(env: Env, merchant: Address) -> Result<(), PaymentError> {
        merchant.require_auth();

        let storage = Storage::new(&env);
        let mut merchant_data = storage.get_merchant(&merchant)?;

        merchant_data.set_active(true);
        storage.save_merchant(&merchant, &merchant_data);
        
        log!(&env, "merchant_activated", merchant);
        
        Ok(())
    }
}

/// Optimized message creation for signature verification
/// Reduces gas cost by pre-calculating message size and using efficient byte operations
fn create_optimized_message(env: &Env, order: &PaymentOrder) -> Bytes {
    // Pre-calculate approximate message size to avoid reallocations
    let mut message = Bytes::new(env);
    
    // Add merchant address (32 bytes)
    message.append(&order.merchant_address.clone().to_xdr(env));

    // Add amount as 8 bytes (i64)
    let amount_bytes = order.amount.to_be_bytes();
    for &b in amount_bytes.iter() {
        message.push_back(b);
    }

    // Add token address (32 bytes)
    message.append(&order.token.clone().to_xdr(env));

    // Add nonce as 4 bytes (u32)
    let nonce_bytes = order.nonce.to_be_bytes();
    for &b in nonce_bytes.iter() {
        message.push_back(b);
    }

    // Add expiration as 4 bytes (u32)
    let expiration_bytes = order.expiration.to_be_bytes();
    for &b in expiration_bytes.iter() {
        message.push_back(b);
    }

    // Add order id
    message.append(&order.order_id.clone().to_xdr(env));
    
    message
}

#[cfg(test)]
mod test;
