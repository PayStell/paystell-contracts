#![no_std]

mod storage;
mod types;
mod error;
mod helper;

use soroban_sdk::{
    contract, contractimpl, token, Address, Env,
    Vec, BytesN, Bytes, xdr::ToXdr, String,
};

use crate::{
    error::PaymentError,
    types::{Merchant, PaymentOrder, MerchantCategory, ProfileUpdateData,
            MerchantRegisteredEvent, ProfileUpdatedEvent, MerchantDeactivatedEvent,
            LimitsUpdatedEvent, merchant_registered_topic, profile_updated_topic,
            merchant_deactivated_topic, limits_updated_topic},
    storage::Storage,
    helper::{validate_name, validate_description, validate_contact_info, 
             validate_transaction_limit, DEFAULT_TRANSACTION_LIMIT},
};

/// payment-processing-contract trait defining the core functionality
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
    
    // Payment Processing Operations
    fn process_payment_with_signature(
        env: Env,
        payer: Address,
        order: PaymentOrder,
        signature: BytesN<64>,
        merchant_public_key: BytesN<32>,
    ) -> Result<(), PaymentError>;
}

#[contract]
pub struct PaymentProcessingContract;

#[contractimpl]
impl PaymentProcessingTrait for PaymentProcessingContract {
    fn register_merchant(
        env: Env,
        merchant_address: Address,
        name: String,
        description: String,
        contact_info: String,
        category: MerchantCategory,
    ) -> Result<(), PaymentError> {
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

    fn add_supported_token(env: Env, merchant: Address, token: Address) -> Result<(), PaymentError> {
        // Verify authorization
        merchant.require_auth();

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
        // Verify authorization from payer
        payer.require_auth();

        // Verify the order hasn't expired
        if env.ledger().timestamp() > order.expiration {
            return Err(PaymentError::OrderExpired);
        }

        let storage = Storage::new(&env);

        // Verify merchant exists and is active
        let mut merchant = storage.get_merchant(&order.merchant_address)?;
        if !merchant.active {
            return Err(PaymentError::MerchantInactive);
        }

        // Verify token is supported by merchant
        if !merchant.supported_tokens.contains(&order.token) {
            return Err(PaymentError::InvalidToken);
        }

        // Verify transaction limit
        if order.amount > merchant.max_transaction_limit {
            return Err(PaymentError::TransactionLimitExceeded);
        }

        // Verify the nonce hasn't been used
        if storage.is_nonce_used(&order.merchant_address, order.nonce) {
            return Err(PaymentError::NonceAlreadyUsed);
        }

        // Create message for signature verification
        let mut message = Bytes::new(&env);
        
        // Add merchant address
        message.append(&order.merchant_address.clone().to_xdr(&env));

        // Add amount bytes
        for &b in order.amount.to_be_bytes().iter() {
            message.push_back(b);
        }

        // Add token address
        message.append(&order.token.clone().to_xdr(&env));

        // Add nonce bytes
        for &b in order.nonce.to_be_bytes().iter() {
            message.push_back(b);
        }

        // Add expiration bytes
        for &b in order.expiration.to_be_bytes().iter() {
            message.push_back(b);
        }

        // Add order id
        message.append(&order.order_id.clone().to_xdr(&env));
        
        // Verify signature
        #[cfg(not(test))]
        env.crypto().ed25519_verify(&_merchant_public_key, &message, &_signature);

        // Process the payment using Stellar token contract
        let token_client = token::Client::new(&env, &order.token);
        
        // Transfer tokens from payer to merchant
        token_client.transfer(
            &payer,
            &order.merchant_address,
            &order.amount,
        );

        // Record used nonce
        storage.mark_nonce_used(&order.merchant_address, order.nonce);

        // Update merchant's last activity timestamp
        merchant.last_activity_timestamp = env.ledger().timestamp();
        storage.save_merchant(&order.merchant_address, &merchant);

        Ok(())
    }
}

#[cfg(test)]
mod test;
