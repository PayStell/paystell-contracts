#![no_std]

mod storage;
mod types;
mod error;

use soroban_sdk::{
    contract, contractimpl, token, Address, Env, String,
    Vec,
};

use crate::{
    error::PaymentError,
    types::{Merchant, PaymentLink},
    storage::Storage,
};

/// payment-processing-contract trait defining the core functionality
pub trait PaymentProcessingTrait {
    // Merchant Management Operations
    fn register_merchant(env: Env, merchant_address: Address) -> Result<(), PaymentError>;
    fn add_supported_token(env: Env, merchant: Address, token: Address) -> Result<(), PaymentError>;
    
    // Payment Link Operations
    fn create_payment_link(
        env: Env,
        merchant: Address,
        amount: i128,
        token: Address,
        description: String,
    ) -> Result<String, PaymentError>;
    
    // Payment Processing Operations
    fn process_payment(
        env: Env,
        payment_link_id: String,
        payer: Address,
    ) -> Result<(), PaymentError>;
}

#[contract]
pub struct PaymentProcessingContract;

#[contractimpl]
impl PaymentProcessingTrait for PaymentProcessingContract {
    fn register_merchant(env: Env, merchant_address: Address) -> Result<(), PaymentError> {
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
        // Verify authorization
        merchant.require_auth();

        let storage = Storage::new(&env);
        let mut merchant_data = storage.get_merchant(&merchant)?;

        // Add token to supported list
        merchant_data.supported_tokens.push_back(token);
        storage.save_merchant(&merchant, &merchant_data);
        
        Ok(())
    }

    fn create_payment_link(
        env: Env,
        merchant: Address,
        amount: i128,
        token: Address,
        description: String,
    ) -> Result<String, PaymentError> {
        // Verify authorization
        merchant.require_auth();

        let storage = Storage::new(&env);
        let merchant_data = storage.get_merchant(&merchant)?;

        // Validate token is supported
        if !merchant_data.supported_tokens.contains(&token) {
            return Err(PaymentError::InvalidAmount);
        }

        // Create payment link
        let payment_link = PaymentLink {
            merchant_id: merchant,
            amount,
            token,
            description,
            active: true,
        };

        // Generate unique ID for payment link
        let timestamp = env.ledger().timestamp();
        let mut buf = [0u8; 20];
        let mut i = 0;
        let mut n = timestamp;
        loop {
            buf[i] = (n % 10) as u8 + b'0';
            n /= 10;
            if n == 0 { break; }
            i += 1;
        }
        buf[..=i].reverse();
        let link_id = String::from_str(&env, core::str::from_utf8(&buf[..=i]).unwrap());
        storage.save_payment_link(&link_id, &payment_link);

        Ok(link_id)
    }

    fn process_payment(
        env: Env,
        payment_link_id: String,
        payer: Address,
    ) -> Result<(), PaymentError> {
        // Verify authorization
        payer.require_auth();

        let storage = Storage::new(&env);
        
        // Get and validate payment link
        let payment_link = storage.get_payment_link(&payment_link_id)?;
        
        // Check for duplicate payment
        if storage.is_payment_processed(&payment_link_id) {
            return Err(PaymentError::PaymentAlreadyProcessed);
        }

        // Process the payment using Stellar token contract
        let token_client = token::Client::new(&env, &payment_link.token);
        
        // Transfer tokens from payer to merchant
        token_client.transfer(
            &payer,
            &payment_link.merchant_id,
            &payment_link.amount,
        );

        // Record processed payment
        storage.mark_payment_processed(&payment_link_id);

        Ok(())
    }
}

#[cfg(test)]
mod test;
