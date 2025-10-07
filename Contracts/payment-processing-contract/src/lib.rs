#![no_std]

mod error;
mod storage;
mod types;

use soroban_sdk::{
    contract, contractimpl, token, Address, Env,
    Vec, BytesN, Bytes, xdr::ToXdr, Symbol, log, Map, panic_with_error,
};
// Note: In Soroban, we use the standard Vec from soroban_sdk, not alloc::vec

use crate::{
    error::PaymentError,
    types::{
        Merchant, PaymentOrder, BatchMerchantRegistration, BatchTokenAddition, 
        BatchPayment, GasEstimate, NonceTracker, Fee
    },
    storage::Storage,
};

/// Optimized payment-processing-contract trait with gas optimization features
pub trait PaymentProcessingTrait {
    // Merchant Management Operations
    fn register_merchant(env: Env, merchant_address: Address) -> Result<(), PaymentError>;
    fn add_supported_token(env: Env, merchant: Address, token: Address)
        -> Result<(), PaymentError>;

    // Fee Management Operations
    fn set_admin(env: Env, admin: Address) -> Result<(), PaymentError>;
    fn set_fee(
        env: Env,
        fee_rate: u64,
        fee_collector: Address,
        fee_token: Address,
    ) -> Result<(), PaymentError>;
    fn get_fee_info(env: Env) -> Result<(u64, Address, Address), PaymentError>;

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

    // Pause Management Operations
    fn set_pause_admin(env: Env, admin: Address, new_admin: Address) -> Result<(), PaymentError>;
    fn pause(env: Env, admin: Address) -> Result<(), PaymentError>;
    fn pause_for_duration(env: Env, admin: Address, duration: u64) -> Result<(), PaymentError>;
    fn unpause(env: Env, admin: Address) -> Result<(), PaymentError>;
    fn is_paused(env: &Env) -> bool;
}

#[contract]
pub struct PaymentProcessingContract;

#[contractimpl]
impl PaymentProcessingTrait for PaymentProcessingContract {

    fn set_admin(env: Env, admin: Address) -> Result<(), PaymentError> {
        let storage = Storage::new(&env);

        if let Some(current_admin) = storage.get_admin() {
            // Existing admin must authorize the change
            current_admin.require_auth();
        } else {
            // First-time setup: new admin must authorize themselves
            admin.require_auth();
        }
        storage.set_admin(&admin);
        Ok(())
    }

    fn set_fee(
        env: Env,
        fee_rate: u64,
        fee_collector: Address,
        fee_token: Address,
    ) -> Result<(), PaymentError> {
        let storage = Storage::new(&env);
        let fee = Fee {
            fee_rate,
            fee_collector,
            fee_token,
        };
        let admin = storage.get_admin().ok_or(PaymentError::AdminNotFound)?;
        admin.require_auth();
        storage.set_fee_info(&fee, &admin)?;
        Ok(())
    }

    fn get_fee_info(env: Env) -> Result<(u64, Address, Address), PaymentError> {
        let storage = Storage::new(&env);
        let rate = storage.get_fee_rate();
        let collector = storage
            .get_fee_collector()
            .ok_or(PaymentError::AdminNotFound)?;
        let token = storage.get_fee_token().ok_or(PaymentError::InvalidToken)?;
        Ok((rate, collector, token))
    }

    fn register_merchant(env: Env, merchant_address: Address) -> Result<(), PaymentError> {
        if Self::is_paused(&env) {
            return Err(PaymentError::ContractPaused);
        }
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


    fn add_supported_token(
        env: Env,
        merchant: Address,
        token: Address,
    ) -> Result<(), PaymentError> {
        // Verify authorization
        merchant.require_auth();

        if Self::is_paused(&env) {
            return Err(PaymentError::ContractPaused);
        }

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
        if Self::is_paused(&env) {
            return Err(PaymentError::ContractPaused);
        }
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
        env.crypto()
            .ed25519_verify(&_merchant_public_key, &message, &_signature);

        // Get fee information
        let fee_collector = storage
            .get_fee_collector()
            .ok_or(PaymentError::AdminNotFound)?;

        let fee_token = storage.get_fee_token().ok_or(PaymentError::InvalidToken)?;

        // Ensure fee token matches payment token
        if fee_token != order.token {
            return Err(PaymentError::InvalidToken);
        }

        let fee_amount = storage.calculate_fee(order.amount as i128);

        if fee_amount < 0 {
            return Err(PaymentError::InvalidAmount);
        }
        let merchant_amount = (order.amount as i128) - fee_amount;

        // Process the payment using Stellar token contract
        let payment_token_client = token::Client::new(&env, &order.token);

        // Transfer merchant amount first
        payment_token_client.transfer(&payer, &order.merchant_address, &merchant_amount);

        // Then transfer fee if applicable
        if fee_amount > 0 {
            let fee_token_client = token::Client::new(&env, &fee_token);
            fee_token_client.transfer(&payer, &fee_collector, &fee_amount);
            env.events().publish(
                ("fee_collected",),
                (fee_collector.clone(), fee_amount, order.order_id.clone()),
            );
        }

        // Record used nonce (optimized bitmap storage)
        storage.mark_nonce_used(&order.merchant_address, order.nonce);

        // Emit optimized event
        log!(&env, "payment_processed", payer, order.merchant_address, order.amount, order.nonce);

        Ok(())
    }

    // Batch Operations for Gas Optimization
    fn batch_register_merchants(env: Env, batch: BatchMerchantRegistration) -> Result<(), PaymentError> {
        if Self::is_paused(&env) {
            return Err(PaymentError::ContractPaused);
        }
        let storage = Storage::new(&env);
        
        for merchant_address in batch.merchants.iter() {
            merchant_address.require_auth();
            let merchant = Merchant::new(&env, merchant_address.clone());
            storage.save_merchant(&merchant_address, &merchant);
        }
        
        log!(&env, "merchants_batch_registered", batch.merchants.len());
        
        Ok(())
    }

    fn batch_add_tokens(env: Env, batch: BatchTokenAddition) -> Result<(), PaymentError> {
        if Self::is_paused(&env) {
            return Err(PaymentError::ContractPaused);
        }
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
        if Self::is_paused(&env) {
            return Err(PaymentError::ContractPaused);
        }
        batch.payer.require_auth();

        let storage = Storage::new(&env);
        
        // Validate signature count matches order count
        if batch.signatures.len() != batch.orders.len() {
            return Err(PaymentError::InvalidSignature);
        }
        
        let mut seen: Map<Address, Vec<u32>> = Map::new(&env);
        
        for order in batch.orders.iter() {
            if env.ledger().timestamp() > order.expiration as u64 {
                return Err(PaymentError::OrderExpired);
            }

            let merchant = storage.get_merchant(&order.merchant_address)?;
            if !merchant.is_active() {
                return Err(PaymentError::MerchantNotFound);
            }

            if !merchant.supports_token(&order.token) {
                return Err(PaymentError::InvalidToken);
            }

            if storage.is_nonce_used(&order.merchant_address, order.nonce) {
                return Err(PaymentError::NonceAlreadyUsed);
            }
            
            let mut merchant_nonces = seen.get(order.merchant_address.clone()).unwrap_or(Vec::new(&env));
            if merchant_nonces.iter().any(|n| n == order.nonce) {
                return Err(PaymentError::NonceAlreadyUsed);
            }
            merchant_nonces.push_back(order.nonce);
            seen.set(order.merchant_address.clone(), merchant_nonces);
        }

        for (idx, order) in batch.orders.iter().enumerate() {
            let message = create_optimized_message(&env, &order);
            
            #[cfg(not(test))]
            {
                let sig = batch.signatures.get(idx as u32).ok_or(PaymentError::InvalidSignature)?;
                env.crypto().ed25519_verify(&batch.merchant_public_key, &message, &sig);
            }

            let token_client = token::Client::new(&env, &order.token);
            token_client.transfer(
                &batch.payer,
                &order.merchant_address,
                &(order.amount as i128),
            );
        }

        for (merchant, nonces) in seen.iter() {
            // Convert Soroban Vec to standard Vec for batch operation - no hardcoded limit
            let mut nonces_vec = Vec::new(&env);
            for nonce in nonces.iter() {
                nonces_vec.push_back(nonce);
            }
            // Convert to array for batch operation
            let mut nonces_array = [0u32; 1000]; // Increased limit to 1000
            let mut i = 0;
            for nonce in nonces_vec.iter() {
                if i < 1000 {
                    nonces_array[i] = nonce;
                    i += 1;
                }
            }
            storage.batch_mark_nonces_used(&merchant, &nonces_array[..i]);
        }
        
        log!(&env, "payments_batch_processed", batch.payer, batch.orders.len());
        
        Ok(())
    }

    // Gas Estimation Functions
    fn estimate_gas_for_payment(_env: Env, _order: PaymentOrder) -> Result<GasEstimate, PaymentError> {
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
        let register_merchants = Symbol::new(&env, "reg_mer");
        let add_tokens = Symbol::new(&env, "add_tok");
        let process_payments = Symbol::new(&env, "proc_pay");
        
        let per_item_gas = if operation_type == register_merchants {
            15_000u64
        } else if operation_type == add_tokens {
            8_000u64
        } else if operation_type == process_payments {
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

        if Self::is_paused(&env) {
            return Err(PaymentError::ContractPaused);
        }

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

    // Pause Management Operations
    fn set_pause_admin(env: Env, admin: Address, new_admin: Address) -> Result<(), PaymentError> {
        admin.require_auth();

        let storage = Storage::new(&env);
        let _  = storage.set_pause_admin_internal(admin, new_admin);
        Ok(())
    }

    fn pause(env: Env, admin: Address) -> Result<(), PaymentError> {
        admin.require_auth();
        let storage = Storage::new(&env);
        let pause_admin = storage.get_pause_admin().unwrap_or_else(|_| panic_with_error!(env, PaymentError::AdminNotFound));

        if pause_admin != admin {
            return Err(PaymentError::NotAuthorized);
        }

        if Self::is_paused(&env) {
            return Err(PaymentError::AlreadyPaused);
        }

        storage.set_pause();

        env.events().publish(
            (Symbol::new(&env, "contract_paused"), admin),
            env.ledger().timestamp(),
        );
        Ok(())
    }

    fn pause_for_duration(env: Env, admin: Address, duration: u64) -> Result<(), PaymentError> {
        admin.require_auth();
        let storage = Storage::new(&env);
        let pause_admin = storage.get_pause_admin().unwrap_or_else(|_| panic_with_error!(env, PaymentError::AdminNotFound));

        if pause_admin != admin {
            return Err(PaymentError::NotAuthorized);
        }

        if Self::is_paused(&env) {
            return Err(PaymentError::AlreadyPaused);
        }

        let current_time = env.ledger().timestamp();
        let pause_until = current_time + duration;

        storage.set_pause_until(pause_until);

        env.events().publish(
            (Symbol::new(&env, "contract_paused"), admin),
            env.ledger().timestamp(),
        );
        Ok(())
    }

    fn unpause(env: Env, admin: Address) -> Result<(), PaymentError> {
        admin.require_auth();
        let storage = Storage::new(&env);
        let pause_admin = storage.get_pause_admin().unwrap_or_else(|_| panic_with_error!(env, PaymentError::AdminNotFound));

        if pause_admin != admin {
            return Err(PaymentError::NotAuthorized);
        }
        storage.set_unpause();
        storage.set_pause_until(0);

        env.events().publish(
            (Symbol::new(&env, "contract_unpaused"), admin),
            env.ledger().timestamp(),
        );
        Ok(())
    }

    fn is_paused(env: &Env) -> bool {
        let storage = Storage::new(&env);
        storage.is_paused()
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

    
    fn set_pause_admin(env: Env, admin: Address, new_admin: Address) -> Result<(), PaymentError> {
        admin.require_auth();

        let storage = Storage::new(&env);
        let _  = storage.set_pause_admin_internal(admin, new_admin);
        Ok(())
    }

    fn pause(env: Env, admin: Address) -> Result<(), PaymentError> {
        admin.require_auth();
        let storage = Storage::new(&env);
        let pause_admin = storage.get_pause_admin().unwrap_or_else(|_| panic_with_error!(env, PaymentError::AdminNotFound));

        if pause_admin != admin {
            return Err(PaymentError::NotAuthorized);
        }

        if Self::is_paused(&env) {
            return Err(PaymentError::AlreadyPaused);
        }

        storage.set_pause();

        env.events().publish(
            (Symbol::new(&env, "contract_paused"), admin),
            env.ledger().timestamp(),
        );
        Ok(())
    }

    fn pause_for_duration(env: Env, admin: Address, duration: u64) -> Result<(), PaymentError> {
        admin.require_auth();
        let storage = Storage::new(&env);
        let pause_admin = storage.get_pause_admin().unwrap_or_else(|_| panic_with_error!(env, PaymentError::AdminNotFound));

        if pause_admin != admin {
            return Err(PaymentError::NotAuthorized);
        }

        if Self::is_paused(&env) {
            return Err(PaymentError::AlreadyPaused);
        }

        let current_time = env.ledger().timestamp();
        let pause_until = current_time + duration;

        storage.set_pause_until(pause_until);

        env.events().publish(
            (Symbol::new(&env, "contract_paused"), admin),
            env.ledger().timestamp(),
        );
        Ok(())
    }


    fn unpause(env: Env, admin: Address) -> Result<(), PaymentError> {
        admin.require_auth();
        let storage = Storage::new(&env);
        let pause_admin = storage.get_pause_admin().unwrap_or_else(|_| panic_with_error!(env, PaymentError::AdminNotFound));

        if pause_admin != admin {
            return Err(PaymentError::NotAuthorized);
        }
        storage.set_unpause();
        storage.set_pause_until(0);

        env.events().publish(
            (Symbol::new(&env, "contract_unpaused"), admin),
            env.ledger().timestamp(),
        );
        Ok(())
    }

    fn is_paused(env: &Env) -> bool {
        let storage = Storage::new(&env);
        storage.is_paused()
    }

}
