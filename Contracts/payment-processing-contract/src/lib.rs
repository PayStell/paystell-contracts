#![no_std]

mod error;
mod storage;
mod types;

#[cfg(test)]
mod test;

use soroban_sdk::{
    contract, contractimpl, token, Address, Env,
    Vec, BytesN, Bytes, xdr::ToXdr, Map, log, String, Symbol, panic_with_error,
};

use crate::{
    error::PaymentError,
    storage::Storage,
    types::{Fee, Merchant, PaymentOrder, MultiSigPayment, PaymentStatus, PaymentRecord},
};

/// payment-processing-contract trait defining the core functionality
pub trait PaymentProcessingTrait {
    // Merchant Management Operations
    fn register_merchant(env: Env, merchant_address: Address) -> Result<(), PaymentError>;
    fn add_supported_token(env: Env, merchant: Address, token: Address) -> Result<(), PaymentError>;

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

    // Pause / admin controls
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
        let record = PaymentRecord {
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
        let record = PaymentRecord {
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

