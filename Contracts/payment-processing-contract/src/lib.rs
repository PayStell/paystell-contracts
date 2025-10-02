#![no_std]

mod storage;
mod types;
mod error;

use soroban_sdk::{
    contract, contractimpl, token, Address, Env,
    Vec, BytesN, Bytes, xdr::ToXdr, Symbol, panic_with_error
};

use crate::{
    error::PaymentError,
    types::{Merchant, PaymentOrder},
    storage::Storage,
};

/// payment-processing-contract trait defining the core functionality
pub trait PaymentProcessingTrait {
    fn initialize(env: Env, admin: Address) -> Result<(), PaymentError>;

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

    fn set_pause_admin(env: Env, admin: Address) -> Result<(), PaymentError>;
    fn pause(env: Env, admin: Address) -> Result<(), PaymentError>;
    fn pause_for_duration(env: Env, admin: Address, duration: u64) -> Result<(), PaymentError>;
    fn unpause(env: Env, admin: Address) -> Result<(), PaymentError>;
    fn is_paused(env: &Env) -> bool;
}

#[contract]
pub struct PaymentProcessingContract;

#[contractimpl]
impl PaymentProcessingTrait for PaymentProcessingContract {
    fn initialize(env: Env, admin: Address) -> Result<(), PaymentError> {
        admin.require_auth();

        let storage = Storage::new(&env);

        if storage.is_initialized() {
            panic_with_error!(env, PaymentError::AlreadyInitialized);
        }

        let storage = Storage::new(&env);
        storage.set_initialized();
        storage.set_pause_admin(&admin);
        storage.set_pause_until(0);

        env.events().publish(
            (Symbol::new(&env, "contract_initialized"), admin),
            env.ledger().timestamp(),
        );
        Ok(())
    }

    fn register_merchant(env: Env, merchant_address: Address) -> Result<(), PaymentError> {
        if Self::is_paused(&env) {
            return Err(PaymentError::ContractPaused);
        }
        // Verify authorization
        merchant_address.require_auth();

        let storage = Storage::new(&env);
        
        // Create new merchant record
        let merchant = Merchant {
            wallet_address: merchant_address.clone(),
            active: true,
            supported_tokens: Vec::new(&env),
        };

        storage.save_merchant(&merchant_address, &merchant);
        Ok(())
    }

    fn add_supported_token(env: Env, merchant: Address, token: Address) -> Result<(), PaymentError> {
        if Self::is_paused(&env) {
            return Err(PaymentError::ContractPaused);
        }
        // Verify authorization
        merchant.require_auth();

        let storage = Storage::new(&env);
        let mut merchant_data = storage.get_merchant(&merchant)?;

        // Add token to supported list
        merchant_data.supported_tokens.push_back(token);
        storage.save_merchant(&merchant, &merchant_data);
        
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

        // Verify the order hasn't expired
        if env.ledger().timestamp() > order.expiration {
            return Err(PaymentError::OrderExpired);
        }

        let storage = Storage::new(&env);

        // Verify merchant exists and is active
        let merchant = storage.get_merchant(&order.merchant_address)?;
        if !merchant.active {
            return Err(PaymentError::MerchantNotFound);
        }

        // Verify token is supported by merchant
        if !merchant.supported_tokens.contains(&order.token) {
            return Err(PaymentError::InvalidToken);
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

        Ok(())
    }

    
    fn set_pause_admin(env: Env, admin: Address, ) -> Result<(), PaymentError> {
        admin.require_auth();

        let storage = Storage::new(&env);
        storage.set_pause_admin(&admin);
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

#[cfg(test)]
mod test;
