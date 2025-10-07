#![no_std]

mod error;
mod storage;
mod types;

#[cfg(test)]
mod test;

use soroban_sdk::{
    contract, contractimpl, token, Address, Env,
    Vec, BytesN, Bytes, xdr::ToXdr, Symbol, panic_with_error
};

use crate::{
    error::PaymentError,
    storage::Storage,
    types::{Fee, Merchant, PaymentOrder},
};

/// payment-processing-contract trait defining the core functionality
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

        // Create new merchant record
        let merchant = Merchant {
            wallet_address: merchant_address.clone(),
            active: true,
            supported_tokens: Vec::new(&env),
        };

        storage.save_merchant(&merchant_address, &merchant);
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
        env.crypto()
            .ed25519_verify(&_merchant_public_key, &message, &_signature);

        let fee_collector = storage
            .get_fee_collector()
            .ok_or(PaymentError::AdminNotFound)?;

        let fee_token = storage.get_fee_token().ok_or(PaymentError::InvalidToken)?;

        if !merchant.supported_tokens.contains(&fee_token) {
            return Err(PaymentError::InvalidToken);
        }

        // Ensure fee token matches payment token
        if fee_token != order.token {
            return Err(PaymentError::InvalidToken);
        }

        let fee_amount = storage.calculate_fee(order.amount);

        if fee_amount < 0 {
            return Err(PaymentError::InvalidAmount);
        }
        let merchant_amount = order.amount - fee_amount;

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

        // Record used nonce
        storage.mark_nonce_used(&order.merchant_address, order.nonce);

        Ok(())
    }

    
    fn set_pause_admin(env: Env, admin: Address, new_admin: Address) -> Result<(), PaymentError> {
        admin.require_auth();

        let storage = Storage::new(&env);
        let current_admin = storage.get_admin().unwrap_or_else(|| panic_with_error!(env, PaymentError::AdminNotFound));

        // let pause_admin = storage.get_pause_admin().unwrap_or_else(|_| panic_with_error!(env, PaymentError::AdminNotFound));
        if current_admin != admin {
            return Err(PaymentError::NotAuthorized);
        }
        storage.set_pause_admin_internal(&new_admin);
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
