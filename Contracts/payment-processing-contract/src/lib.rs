#![no_std]

mod error;
mod storage;
mod types;
mod helper;

use soroban_sdk::{
    contract, contractimpl, token, Address, Env,
    Vec, BytesN, Bytes, xdr::ToXdr, Map, log, String, Symbol, panic_with_error,
};
// Note: In Soroban, we use the standard Vec from soroban_sdk, not alloc::vec

use crate::{
    error::PaymentError,
    types::{
        Merchant, PaymentOrder, BatchMerchantRegistration, BatchTokenAddition, 
        BatchPayment, GasEstimate, NonceTracker, Fee, MultiSigPayment, PaymentStatus, PaymentRecord,
        MerchantCategory, ProfileUpdateData, MerchantRegisteredEvent, ProfileUpdatedEvent, 
        MerchantDeactivatedEvent, LimitsUpdatedEvent, merchant_registered_topic, 
        profile_updated_topic, merchant_deactivated_topic, limits_updated_topic,
       RefundRequest, RefundStatus, MultiSigPaymentRecord,
        // NEW IMPORTS
        PaymentQueryParams, PaymentStats, MerchantPaymentSummary, PayerPaymentSummary,
        PaymentIndexEntry, PaymentBucket, CompressedPaymentRecord,
    },
    storage::Storage,
    helper::{validate_name, validate_description, validate_contact_info, 
             validate_transaction_limit, DEFAULT_TRANSACTION_LIMIT},
};

/// Optimized payment-processing-contract trait with gas optimization features
pub trait PaymentProcessingTrait {
    // Merchant Management Operations
    fn register_merchant(
        env: Env,
        merchant_address: Address,
        name: String,
        description: String,
        contact_info: String,
        category: MerchantCategory,
    ) -> Result<(), PaymentError>;
    
    fn add_supported_token(env: Env, merchant: Address, token: Address) -> Result<(), PaymentError>;

    // Profile Management Operations
    fn update_merchant_profile(
        env: Env,
        merchant: Address,
        update_data: ProfileUpdateData,
    ) -> Result<(), PaymentError>;
    
    fn get_merchant_profile(env: Env, merchant: Address) -> Result<Merchant, PaymentError>;
    
    fn set_merchant_limits(
        env: Env,
        merchant: Address,
        max_transaction_limit: i128,
    ) -> Result<(), PaymentError>;
    
    fn deactivate_merchant(env: Env, merchant: Address) -> Result<(), PaymentError>;

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
    fn activate_merchant(env: Env, merchant: Address) -> Result<(), PaymentError>;

    // Multi-signature Payment Operations
    fn initiate_multisig_payment(
        env: Env,
        amount: i128,
        token: Address,
        recipient: Address,
        signers: Vec<Address>,
        threshold: u32,
        expiry: u64,
    ) -> Result<u128, PaymentError>;

    fn add_signature(
        env: Env,
        payment_id: u128,
        signer: Address,
    ) -> Result<(), PaymentError>;

    fn execute_multisig_payment(
        env: Env,
        payment_id: u128,
        executor: Address,
    ) -> Result<(), PaymentError>;

    fn cancel_multisig_payment(
        env: Env,
        payment_id: u128,
        canceller: Address,
        reason: String,
    ) -> Result<(), PaymentError>;

    fn get_multisig_payment(
        env: Env,
        payment_id: u128,
    ) -> Result<MultiSigPayment, PaymentError>;

    fn batch_execute_payments(
        env: Env,
        payment_ids: Vec<u128>,
        executor: Address,
    ) -> Result<Vec<u128>, PaymentError>;

    // Pause Management Operations
    fn set_pause_admin(env: Env, admin: Address, new_admin: Address) -> Result<(), PaymentError>;
    fn pause(env: Env, admin: Address) -> Result<(), PaymentError>;
    fn pause_for_duration(env: Env, admin: Address, duration: u64) -> Result<(), PaymentError>;
    fn unpause(env: Env, admin: Address) -> Result<(), PaymentError>;
    fn is_paused(env: &Env) -> bool;

    // Refund Management Operations
    fn initiate_refund(
        env: Env,
        caller: Address,
        refund_id: String,
        order_id: String,
        amount: i128,
        reason: String,
    ) -> Result<(), PaymentError>;

    fn approve_refund(env: Env, caller: Address, refund_id: String) -> Result<(), PaymentError>;
    fn reject_refund(env: Env, caller: Address, refund_id: String) -> Result<(), PaymentError>;
    fn execute_refund(env: Env, refund_id: String) -> Result<(), PaymentError>;
    fn get_refund_status(env: Env, refund_id: String) -> Result<RefundStatus, PaymentError>;

    // Payment History Query and Management Functions
    fn get_merchant_payment_history(
        env: Env,
        merchant: Address,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<PaymentRecord>, PaymentError>;

    fn get_payer_payment_history(
        env: Env,
        payer: Address,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<PaymentRecord>, PaymentError>;

    fn get_payment_by_order_id(
        env: Env,
        order_id: String,
    ) -> Result<PaymentRecord, PaymentError>;

    fn query_payments(
        env: Env,
        params: PaymentQueryParams,
    ) -> Result<Vec<PaymentRecord>, PaymentError>;

    fn get_merchant_payment_stats(
        env: Env,
        merchant: Address,
    ) -> Result<MerchantPaymentSummary, PaymentError>;

    fn get_payer_payment_stats(
        env: Env,
        payer: Address,
    ) -> Result<PayerPaymentSummary, PaymentError>;

    fn get_global_payment_stats(env: Env) -> Result<PaymentStats, PaymentError>;

    fn get_payments_by_time_range(
        env: Env,
        start_time: u64,
        end_time: u64,
    ) -> Result<Vec<PaymentBucket>, PaymentError>;

    fn get_payments_by_token(
        env: Env,
        token: Address,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<PaymentRecord>, PaymentError>;

    fn archive_old_payments(
        env: Env,
        admin: Address,
        cutoff_time: u64,
    ) -> Result<(), PaymentError>;
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

    fn register_merchant(
        env: Env,
        merchant_address: Address,
        name: String,
        description: String,
        contact_info: String,
        category: MerchantCategory,
    ) -> Result<(), PaymentError> {
        if Self::is_paused(&env) {
            return Err(PaymentError::ContractPaused);
        }
        // Verify authorization
        merchant_address.require_auth();

        let storage = Storage::new(&env);
        
        // Check if merchant already exists
        if storage.merchant_exists(&merchant_address) {
            return Err(PaymentError::MerchantAlreadyExists);
        }
        
        // Validate profile data
        validate_name(&name)?;
        validate_description(&description)?;
        validate_contact_info(&contact_info)?;
        
        let current_time = env.ledger().timestamp();
        
        // Create new merchant record
        let merchant = Merchant {
            wallet_address: merchant_address.clone(),
            active: true,
            supported_tokens: Vec::new(&env),
            name: name.clone(),
            description,
            contact_info,
            registration_timestamp: current_time,
            last_activity_timestamp: current_time,
            category: category.clone(),
            max_transaction_limit: DEFAULT_TRANSACTION_LIMIT,
        };

        storage.save_merchant(&merchant_address, &merchant);
        
        // Emit registration event
        env.events().publish(
            (merchant_registered_topic(&env),),
            MerchantRegisteredEvent {
                merchant: merchant_address,
                name,
                category,
                timestamp: current_time,
            }
        );
        
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

        // Add token to supported list
        merchant_data.supported_tokens.push_back(token);
        
        // Update last activity timestamp
        merchant_data.last_activity_timestamp = env.ledger().timestamp();
        
        storage.save_merchant(&merchant, &merchant_data);
        Ok(())
    }

    fn update_merchant_profile(
        env: Env,
        merchant: Address,
        update_data: ProfileUpdateData,
    ) -> Result<(), PaymentError> {
        // Verify authorization - only merchant can update their own profile
        merchant.require_auth();

        let storage = Storage::new(&env);
        let mut merchant_data = storage.get_merchant(&merchant)?;

        // Check if merchant is active
        if !merchant_data.active {
            return Err(PaymentError::MerchantInactive);
        }

        // Update fields if requested
        if update_data.update_name {
            validate_name(&update_data.name)?;
            merchant_data.name = update_data.name;
        }

        if update_data.update_description {
            validate_description(&update_data.description)?;
            merchant_data.description = update_data.description;
        }

        if update_data.update_contact_info {
            validate_contact_info(&update_data.contact_info)?;
            merchant_data.contact_info = update_data.contact_info;
        }

        if update_data.update_category {
            merchant_data.category = update_data.category;
        }

        // Update last activity timestamp
        let current_time = env.ledger().timestamp();
        merchant_data.last_activity_timestamp = current_time;

        storage.save_merchant(&merchant, &merchant_data);

        // Emit profile update event
        env.events().publish(
            (profile_updated_topic(&env),),
            ProfileUpdatedEvent {
                merchant,
                timestamp: current_time,
            }
        );

        Ok(())
    }

    fn get_merchant_profile(env: Env, merchant: Address) -> Result<Merchant, PaymentError> {
        let storage = Storage::new(&env);
        storage.get_merchant(&merchant)
    }

    fn set_merchant_limits(
        env: Env,
        merchant: Address,
        max_transaction_limit: i128,
    ) -> Result<(), PaymentError> {
        // Verify authorization - only merchant can set their own limits
        merchant.require_auth();

        let storage = Storage::new(&env);
        let mut merchant_data = storage.get_merchant(&merchant)?;

        // Check if merchant is active
        if !merchant_data.active {
            return Err(PaymentError::MerchantInactive);
        }

        // Validate transaction limit
        validate_transaction_limit(max_transaction_limit)?;

        merchant_data.max_transaction_limit = max_transaction_limit;
        
        // Update last activity timestamp
        let current_time = env.ledger().timestamp();
        merchant_data.last_activity_timestamp = current_time;

        storage.save_merchant(&merchant, &merchant_data);

        // Emit limits update event
        env.events().publish(
            (limits_updated_topic(&env),),
            LimitsUpdatedEvent {
                merchant,
                max_transaction_limit,
                timestamp: current_time,
            }
        );

        Ok(())
    }

    fn deactivate_merchant(env: Env, merchant: Address) -> Result<(), PaymentError> {
        // Verify authorization - only merchant can deactivate their own account
        merchant.require_auth();

        let storage = Storage::new(&env);
        let mut merchant_data = storage.get_merchant(&merchant)?;

        // Set merchant as inactive
        merchant_data.active = false;
        
        // Update last activity timestamp
        let current_time = env.ledger().timestamp();
        merchant_data.last_activity_timestamp = current_time;

        storage.save_merchant(&merchant, &merchant_data);

        // Emit deactivation event
        env.events().publish(
            (merchant_deactivated_topic(&env),),
            MerchantDeactivatedEvent {
                merchant,
                timestamp: current_time,
            }
        );

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
        let mut merchant = storage.get_merchant(&order.merchant_address)?;
        if !merchant.active {
            return Err(PaymentError::MerchantInactive);
        }

        // Verify token is supported by merchant (optimized lookup)
        if !merchant.supports_token(&order.token) {
            return Err(PaymentError::InvalidToken);
        }

        // Verify transaction limit
        if i128::from(order.amount) > merchant.max_transaction_limit {
            return Err(PaymentError::TransactionLimitExceeded);
        }

        // Verify the nonce hasn't been used
        if storage.is_nonce_used(&order.merchant_address, order.nonce) {
            return Err(PaymentError::NonceAlreadyUsed);
        }

        // Optimized message construction using pre-allocated bytes
        let _message = create_optimized_message(&env, &order);
        // Verify signature
        #[cfg(not(test))]
        env.crypto()
            .ed25519_verify(&_merchant_public_key, &_message, &_signature);

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

        // Update merchant's last activity timestamp
        merchant.last_activity_timestamp = env.ledger().timestamp();
        storage.save_merchant(&order.merchant_address, &merchant);

        // Record payment history
        let payment_record = PaymentRecord {
            order_id: order.order_id.clone(),
            merchant_address: order.merchant_address.clone(),
            payer_address: payer.clone(),
            token: order.token.clone(),
            amount: order.amount as i128,
            paid_at: env.ledger().timestamp(),
            refunded_amount: 0,
        };
        storage.save_payment(&payment_record);
        
        // Index the payment for efficient querying
        storage.index_payment(&payment_record);

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
            let merchant = Merchant {
                wallet_address: merchant_address.clone(),
                active: true,
                supported_tokens: Vec::new(&env),
                name: String::from_str(&env, "Batch Merchant"),
                description: String::from_str(&env, "Batch registered merchant"),
                contact_info: String::from_str(&env, "N/A"),
                registration_timestamp: env.ledger().timestamp(),
                last_activity_timestamp: env.ledger().timestamp(),
                category: MerchantCategory::Other,
                max_transaction_limit: 1000000, // Default limit
            };
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
            if !merchant.active {
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

        for (_idx, order) in batch.orders.iter().enumerate() {
            let _message = create_optimized_message(&env, &order);
            
            #[cfg(not(test))]
            {
                let sig = batch.signatures.get(_idx as u32).ok_or(PaymentError::InvalidSignature)?;
                env.crypto().ed25519_verify(&batch.merchant_public_key, &_message, &sig);
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
        Ok(merchant_data.supported_tokens.len() as u32)
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


    fn activate_merchant(env: Env, merchant: Address) -> Result<(), PaymentError> {
        merchant.require_auth();

        let storage = Storage::new(&env);
        let mut merchant_data = storage.get_merchant(&merchant)?;

        merchant_data.active = true;
        storage.save_merchant(&merchant, &merchant_data);
        
        log!(&env, "merchant_activated", merchant);
        
        Ok(())
    }

    // Multi-signature Payment Operations
    fn initiate_multisig_payment(
        env: Env,
        amount: i128,
        token: Address,
        recipient: Address,
        signers: Vec<Address>,
        threshold: u32,
        expiry: u64,
    ) -> Result<u128, PaymentError> {
        // Validate inputs
        if amount <= 0 {
            return Err(PaymentError::InvalidAmount);
        }

        if signers.is_empty() {
            return Err(PaymentError::EmptySignersList);
        }

        if threshold == 0 || threshold > signers.len() {
            return Err(PaymentError::InvalidThreshold);
        }

        if expiry <= env.ledger().timestamp() {
            return Err(PaymentError::PaymentExpired);
        }

        // Check for duplicate signers
        for i in 0..signers.len() {
            for j in (i + 1)..signers.len() {
                if signers.get(i).unwrap() == signers.get(j).unwrap() {
                    return Err(PaymentError::DuplicateSigner);
                }
            }
        }

        let storage = Storage::new(&env);
        let payment_id = storage.get_next_payment_id();

        // Create new multi-sig payment
        let payment = MultiSigPayment {
            payment_id,
            amount,
            token: token.clone(),
            recipient: recipient.clone(),
            signers: signers.clone(),
            threshold,
            signatures: Map::new(&env),
            status: PaymentStatus::Pending,
            expiry,
            created_at: env.ledger().timestamp(),
            reason: None,
        };

        // Save payment
        storage.save_multisig_payment(&payment);

        // Emit event
        log!(&env, "PaymentInitiated: payment_id={}, amount={}, recipient={}, threshold={}",
             payment_id, amount, recipient, threshold);

        Ok(payment_id)
    }

    fn add_signature(
        env: Env,
        payment_id: u128,
        signer: Address,
    ) -> Result<(), PaymentError> {
        // Require authorization from the signer
        signer.require_auth();

        let storage = Storage::new(&env);
        let mut payment = storage.get_multisig_payment(payment_id)?;

        // Validate payment status
        if payment.status != PaymentStatus::Pending {
            return Err(PaymentError::InvalidStatus);
        }

        // Check if payment has expired
        if env.ledger().timestamp() > payment.expiry {
            return Err(PaymentError::PaymentExpired);
        }

        // Verify signer is in the signers list
        let mut is_valid_signer = false;
        for i in 0..payment.signers.len() {
            if payment.signers.get(i).unwrap() == signer {
                is_valid_signer = true;
                break;
            }
        }

        if !is_valid_signer {
            return Err(PaymentError::NotASigner);
        }

        // Check if already signed
        if payment.signatures.contains_key(signer.clone()) {
            return Err(PaymentError::AlreadySigned);
        }

        // Add signature
        payment.signatures.set(signer.clone(), true);

        // Save updated payment
        storage.save_multisig_payment(&payment);

        // Emit event
        log!(&env, "SignatureAdded: payment_id={}, signer={}, signatures_count={}",
             payment_id, signer, payment.signatures.len());

        Ok(())
    }

    fn execute_multisig_payment(
        env: Env,
        payment_id: u128,
        executor: Address,
    ) -> Result<(), PaymentError> {
        // Require authorization from the executor
        executor.require_auth();

        let storage = Storage::new(&env);

        // Use the helper function for execution
        Self::execute_single_payment(&env, &storage, payment_id, &executor)?;

        // Emit event
        log!(&env, "PaymentExecuted: payment_id={}, executor={}",
             payment_id, executor);

        Ok(())
    }

    fn cancel_multisig_payment(
        env: Env,
        payment_id: u128,
        canceller: Address,
        reason: String,
    ) -> Result<(), PaymentError> {
        // Require authorization from the canceller
        canceller.require_auth();

        let storage = Storage::new(&env);
        let mut payment = storage.get_multisig_payment(payment_id)?;

        // Validate payment status - can only cancel pending payments
        if payment.status != PaymentStatus::Pending {
            return match payment.status {
                PaymentStatus::Executed => Err(PaymentError::AlreadyExecuted),
                PaymentStatus::Cancelled => Err(PaymentError::AlreadyCancelled),
                _ => Err(PaymentError::InvalidStatus),
            };
        }

        // Verify canceller is a signer (only signers can cancel)
        let mut is_valid_canceller = false;
        for i in 0..payment.signers.len() {
            if payment.signers.get(i).unwrap() == canceller {
                is_valid_canceller = true;
                break;
            }
        }

        if !is_valid_canceller {
            return Err(PaymentError::NotASigner);
        }

        // Update payment status and reason
        payment.status = PaymentStatus::Cancelled;
        payment.reason = Some(reason.clone());

        // Archive the cancelled payment
        let record = MultiSigPaymentRecord {
            payment_id: payment.payment_id,
            amount: payment.amount,
            token: payment.token.clone(),
            recipient: payment.recipient.clone(),
            signers: payment.signers.clone(),
            threshold: payment.threshold,
            status: PaymentStatus::Cancelled,
            executed_at: env.ledger().timestamp(),
            executor: Some(canceller.clone()),
            reason: Some(reason.clone()),
        };

        storage.archive_payment(&record);
        storage.remove_multisig_payment(payment_id);

        // Emit event
        log!(&env, "PaymentCancelled: payment_id={}, canceller={}, reason={}",
             payment_id, canceller, reason);

        Ok(())
    }

    fn get_multisig_payment(
        env: Env,
        payment_id: u128,
    ) -> Result<MultiSigPayment, PaymentError> {
        let storage = Storage::new(&env);
        storage.get_multisig_payment(payment_id)
    }

    fn batch_execute_payments(
        env: Env,
        payment_ids: Vec<u128>,
        executor: Address,
    ) -> Result<Vec<u128>, PaymentError> {
        // Require authorization from the executor
        executor.require_auth();

        let mut executed_payments = Vec::new(&env);
        let storage = Storage::new(&env);

        // Process each payment
        for i in 0..payment_ids.len() {
            let payment_id = payment_ids.get(i).unwrap();

            // Try to execute each payment, continue on errors
            match Self::execute_single_payment(&env, &storage, payment_id, &executor) {
                Ok(()) => {
                    executed_payments.push_back(payment_id);
                    log!(&env, "BatchExecution: payment_id={} executed successfully", payment_id);
                }
                Err(e) => {
                    log!(&env, "BatchExecution: payment_id={} failed with error={:?}", payment_id, e);
                    // Continue with other payments even if one fails
                }
            }
        }

        // Emit batch completion event
        log!(&env, "BatchExecutionCompleted: total_requested={}, executed={}",
             payment_ids.len(), executed_payments.len());

        Ok(executed_payments)
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

    // Refund Management Operations
    fn initiate_refund(
        env: Env,
        caller: Address,
        refund_id: String,
        order_id: String,
        amount: i128,
        reason: String,
    ) -> Result<(), PaymentError> {
        // Caller may be merchant or payer; require auth
        caller.require_auth();

        let storage = Storage::new(&env);
        let payment = storage.get_payment(&order_id)?;

        // Validate caller is merchant or payer of the original payment
        let is_merchant = caller == payment.merchant_address;
        let is_payer = caller == payment.payer_address;
        if !is_merchant && !is_payer {
            return Err(PaymentError::NotAuthorized);
        }

        // Validate amount does not exceed remaining refundable
        let already_refunded = payment.refunded_amount;
        if amount > payment.amount - already_refunded {
            return Err(PaymentError::ExceedsOriginalAmount);
        }

        // Validate refund window (30 days)
        const MAX_REFUND_WINDOW: u64 = 30 * 24 * 60 * 60;
        if env.ledger().timestamp() > payment.paid_at + MAX_REFUND_WINDOW {
            return Err(PaymentError::RefundWindowExceeded);
        }

        // Create refund request (Pending)
        let request = RefundRequest {
            refund_id: refund_id.clone(),
            order_id: order_id.clone(),
            merchant_address: payment.merchant_address.clone(),
            payer_address: payment.payer_address.clone(),
            token: payment.token.clone(),
            amount,
            reason: reason.clone(),
            requested_at: env.ledger().timestamp(),
            status: RefundStatus::Pending,
            approved_by: None,
        };
        storage.save_refund(&request);

        // Event
        env.events().publish(("refund_initiated",), (refund_id.clone(), order_id.clone(), amount));
        Ok(())
    }

    fn approve_refund(env: Env, caller: Address, refund_id: String) -> Result<(), PaymentError> {
        caller.require_auth();
        let storage = Storage::new(&env);
        let mut req = storage.get_refund(&refund_id)?;

        // Authorization: merchant of payment or admin
        let admin = storage.get_admin();
        let authorized = Some(caller.clone()) == admin || caller == req.merchant_address;
        if !authorized { return Err(PaymentError::NotAuthorized); }

        if let RefundStatus::Pending = req.status {
            req.status = RefundStatus::Approved;
            req.approved_by = Some(caller.clone());
            storage.update_refund(&req);
            env.events().publish(("refund_approved",), (refund_id.clone(),));
            Ok(())
        } else {
            Err(PaymentError::InvalidRefundStatus)
        }
    }

    fn reject_refund(env: Env, caller: Address, refund_id: String) -> Result<(), PaymentError> {
        caller.require_auth();
        let storage = Storage::new(&env);
        let mut req = storage.get_refund(&refund_id)?;
        let admin = storage.get_admin();
        let authorized = Some(caller.clone()) == admin || caller == req.merchant_address;
        if !authorized { return Err(PaymentError::NotAuthorized); }

        if let RefundStatus::Pending = req.status {
            req.status = RefundStatus::Rejected;
            req.approved_by = Some(caller.clone());
            storage.update_refund(&req);
            env.events().publish(("refund_rejected",), (refund_id.clone(),));
            Ok(())
        } else {
            Err(PaymentError::InvalidRefundStatus)
        }
    }

    fn execute_refund(env: Env, refund_id: String) -> Result<(), PaymentError> {
        let storage = Storage::new(&env);
        let mut req = storage.get_refund(&refund_id)?;

        // Must be approved
        if let RefundStatus::Approved = req.status { } else { return Err(PaymentError::InvalidRefundStatus); }

        // Load payment to update and validate remaining amount again
        let mut payment = storage.get_payment(&req.order_id)?;
        if req.amount > payment.amount - payment.refunded_amount {
            return Err(PaymentError::ExceedsOriginalAmount);
        }

        // Require merchant authorization for token transfer
        req.merchant_address.require_auth();

        // Transfer from merchant to payer
        let token_client = token::Client::new(&env, &req.token);
        // Optional balance check for clearer error
        let merchant_balance = token_client.balance(&req.merchant_address);
        if merchant_balance < req.amount {
            return Err(PaymentError::InsufficientBalance);
        }
        token_client.transfer(
            &req.merchant_address,
            &req.payer_address,
            &req.amount,
        );

        // Update payment refunded amount
        payment.refunded_amount = payment.refunded_amount + req.amount;
        storage.update_payment(&payment);

        // Mark refund completed
        req.status = RefundStatus::Completed;
        storage.update_refund(&req);

        env.events().publish(("refund_executed",), (refund_id.clone(), req.amount));
        Ok(())
    }

      fn get_refund_status(env: Env, refund_id: String) -> Result<RefundStatus, PaymentError> {
        let storage = Storage::new(&env);
        let req = storage.get_refund(&refund_id)?;
        Ok(req.status)
    }

    // Payment History Query and Management Functions
     fn get_merchant_payment_history(
        env: Env,
        merchant: Address,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<PaymentRecord>, PaymentError> {
        let storage = Storage::new(&env);
        // Validate pagination parameters
        if limit == 0 || limit > 100 {
            return Err(PaymentError::InvalidPaginationParams);
        }
        let payments = storage.get_merchant_payments(&merchant, limit, offset);
        Ok(payments)
    }

    fn get_payer_payment_history(
        env: Env,
        payer: Address,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<PaymentRecord>, PaymentError> {
        let storage = Storage::new(&env);
        // Validate pagination parameters
        if limit == 0 || limit > 100 {
            return Err(PaymentError::InvalidPaginationParams);
        }
        let payments = storage.get_payer_payments(&payer, limit, offset);
        Ok(payments)
    }

    fn get_payment_by_order_id(
        env: Env,
        order_id: String,
    ) -> Result<PaymentRecord, PaymentError> {
        let storage = Storage::new(&env);
        storage.get_payment(&order_id)
    }

    fn query_payments(
        env: Env,
        params: PaymentQueryParams,
    ) -> Result<Vec<PaymentRecord>, PaymentError> {
        let storage = Storage::new(&env);
        // Validate pagination parameters
        if params.limit == 0 || params.limit > 100 {
            return Err(PaymentError::InvalidPaginationParams);
        }
        // Get all payments from storage
        let all_payments = storage.get_payments_map();
        let mut filtered_payments = Vec::new(&env);
        
        let mut count = 0u32;
        let mut skipped = 0u32;
        
        for (_order_id, payment) in all_payments.iter() {
            // Apply filters
            let mut matches = true;
            
            // Time range filter
            if let Some(start_time) = params.start_time {
                if payment.paid_at < start_time {
                    matches = false;
                }
            }
            if let Some(end_time) = params.end_time {
                if payment.paid_at > end_time {
                    matches = false;
                }
            }
            
            // Amount range filter
            if let Some(min_amount) = params.min_amount {
                if payment.amount < min_amount {
                    matches = false;
                }
            }
            if let Some(max_amount) = params.max_amount {
                if payment.amount > max_amount {
                    matches = false;
                }
            }
            
            // Token filter
            if let Some(ref token) = params.token {
                if payment.token != *token {
                    matches = false;
                }
            }
            
            if matches {
                // Handle offset
                if skipped < params.offset {
                    skipped += 1;
                    continue;
                }
                
                // Add to results
                filtered_payments.push_back(payment);
                count += 1;
                
                // Check limit
                if count >= params.limit {
                    break;
                }
            }
        }
        
        Ok(filtered_payments)
    }

    fn get_merchant_payment_stats(
        env: Env,
        merchant: Address,
    ) -> Result<MerchantPaymentSummary, PaymentError> {
        let storage = Storage::new(&env);
        storage.get_merchant_stats(&merchant)
            .ok_or(PaymentError::MerchantNotFound)
    }

    fn get_payer_payment_stats(
        env: Env,
        payer: Address,
    ) -> Result<PayerPaymentSummary, PaymentError> {
        let storage = Storage::new(&env);
        storage.get_payer_stats(&payer)
            .ok_or(PaymentError::PaymentNotFound)
    }

    fn get_global_payment_stats(env: Env) -> Result<PaymentStats, PaymentError> {
        let storage = Storage::new(&env);
        storage.get_global_stats()
            .ok_or(PaymentError::PaymentNotFound)
    }

    fn get_payments_by_time_range(
        env: Env,
        start_time: u64,
        end_time: u64,
    ) -> Result<Vec<PaymentBucket>, PaymentError> {
        // Validate time range
        if start_time >= end_time {
            return Err(PaymentError::InvalidAmount);
        }
        
        let storage = Storage::new(&env);
        let buckets = storage.get_payments_by_time_range(start_time, end_time);
        Ok(buckets)
    }

    fn get_payments_by_token(
        env: Env,
        token: Address,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<PaymentRecord>, PaymentError> {
        // Validate pagination parameters
        if limit == 0 || limit > 100 {
            return Err(PaymentError::InvalidPaginationParams);
        }
        let storage = Storage::new(&env);
        // Retrieve token index
        let token_index: Map<Address, soroban_sdk::Vec<soroban_sdk::String>> = env
            .storage()
            .persistent()
            .get(&DataKey::TokenBasedIndex.as_symbol(&env))
            .unwrap_or_else(|| Map::new(&env));
        let order_ids = token_index
            .get(token)
            .unwrap_or_else(|| soroban_sdk::Vec::new(&env));
        let mut results = Vec::new(&env);
        let mut count = 0u32;
        let mut skipped = 0u32;
        for i in 0..order_ids.len() {
            if let Some(order_id) = order_ids.get(i) {
                // Handle offset
                if skipped < offset {
                    skipped += 1;
                    continue;
                }
                if let Ok(payment) = storage.get_payment(&order_id) {
                    results.push_back(payment);
                    count += 1;
                    // Check limit
                    if count >= limit {
                        break;
                    }
                }
            }
        }
        Ok(results)
    }

    fn archive_old_payments(
        env: Env,
        admin: Address,
        cutoff_time: u64,
    ) -> Result<(), PaymentError> {
        // Require admin authorization
        admin.require_auth();
        
        let storage = Storage::new(&env);
        
        // Verify admin
        let stored_admin = storage.get_admin()
            .ok_or(PaymentError::AdminNotFound)?;
        
        if stored_admin != admin {
            return Err(PaymentError::NotAuthorized);
        }
        
        // Validate cutoff time (must be in the past)
        if cutoff_time >= env.ledger().timestamp() {
            return Err(PaymentError::InvalidAmount);
        }
        
         // Perform compression and archival (batch size of 100)
      let batch_size = 100u32;
        storage.compress_old_payments(cutoff_time, batch_size);
        
        // Emit event
        log!(&env, "PaymentsArchived: cutoff_time={}", cutoff_time);
        
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

// Helper functions for multi-signature payments
impl PaymentProcessingContract {
    // Helper function for payment completion
    fn complete_payment(
        env: &Env,
        storage: &Storage,
        payment: &MultiSigPayment,
        executor: &Address,
    ) -> Result<(), PaymentError> {
        // Create payment record for history
        let record = MultiSigPaymentRecord {
            payment_id: payment.payment_id,
            amount: payment.amount,
            token: payment.token.clone(),
            recipient: payment.recipient.clone(),
            signers: payment.signers.clone(),
            threshold: payment.threshold,
            status: PaymentStatus::Executed,
            executed_at: env.ledger().timestamp(),
            executor: Some(executor.clone()),
            reason: None,
        };

        // Archive the payment record
        storage.archive_payment(&record);

        // Remove from active payments for cleanup
        storage.remove_multisig_payment(payment.payment_id);

        // Emit completion event
        log!(env, "PaymentCompleted: payment_id={}, archived_at={}",
             payment.payment_id, env.ledger().timestamp());

        Ok(())
    }

    // Helper function for single payment execution (used by both single and batch)
    fn execute_single_payment(
        env: &Env,
        storage: &Storage,
        payment_id: u128,
        executor: &Address,
    ) -> Result<(), PaymentError> {
        let mut payment = storage.get_multisig_payment(payment_id)?;

        // Validate payment status
        if payment.status != PaymentStatus::Pending {
            return match payment.status {
                PaymentStatus::Executed => Err(PaymentError::AlreadyExecuted),
                PaymentStatus::Cancelled => Err(PaymentError::AlreadyCancelled),
                _ => Err(PaymentError::InvalidStatus),
            };
        }

        // Check if payment has expired
        if env.ledger().timestamp() > payment.expiry {
            return Err(PaymentError::PaymentExpired);
        }

        // Verify executor is a signer
        let mut is_valid_executor = false;
        for i in 0..payment.signers.len() {
            if payment.signers.get(i).unwrap() == *executor {
                is_valid_executor = true;
                break;
            }
        }

        if !is_valid_executor {
            return Err(PaymentError::NotASigner);
        }

        // Check if threshold is met
        if payment.signatures.len() < payment.threshold {
            return Err(PaymentError::ThresholdNotMet);
        }

        // Execute the payment using Stellar token contract
        let token_client = token::Client::new(env, &payment.token);

        // Transfer tokens to recipient
        token_client.transfer(
            executor, // The executor must have the tokens or be authorized
            &payment.recipient,
            &payment.amount,
        );

        // Update payment status
        payment.status = PaymentStatus::Executed;

        // Complete the payment (archive and cleanup)
        Self::complete_payment(env, storage, &payment, executor)?;

        Ok(())
    }
}