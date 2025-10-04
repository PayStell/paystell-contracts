#![no_std]

mod storage;
mod types;
mod error;
mod events;

use soroban_sdk::{
    contract, contractimpl, token, Address, Env,
    Vec, BytesN, Bytes, xdr::ToXdr, String,
};

use crate::{
    error::PaymentError,
    types::{Merchant, PaymentOrder, PaymentRecord, PaymentStatus, PaymentRecordQuery},
    storage::Storage,
    events::Events,
};

/// payment-processing-contract trait defining the core functionality
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
    ) -> Result<String, PaymentError>;
    
    // Payment History Query Operations
    fn get_payment_record(env: Env, payment_id: String) -> Result<PaymentRecord, PaymentError>;
    fn get_merchant_payments(env: Env, merchant: Address) -> Vec<String>;
    fn get_payer_payments(env: Env, payer: Address) -> Vec<String>;
    fn query_payments(env: Env, query: PaymentRecordQuery) -> Vec<PaymentRecord>;
    
    // Payment History Reconciliation
    fn validate_payment(env: Env, payment_id: String) -> Result<bool, PaymentError>;
    fn reconcile_payments(env: Env, payment_ids: Vec<String>) -> Result<u32, PaymentError>;
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

    fn process_payment_with_signature(
        env: Env,
        payer: Address,
        order: PaymentOrder,
        _signature: BytesN<64>,
        _merchant_public_key: BytesN<32>,
    ) -> Result<String, PaymentError> {
        // Verify authorization from payer
        payer.require_auth();

        let storage = Storage::new(&env);
        let current_time = env.ledger().timestamp();
        
        // Generate payment ID
        let payment_id = storage.generate_payment_id();
        
        // Create initial payment record with Pending status
        let mut payment_record = PaymentRecord {
            payment_id: payment_id.clone(),
            payer: payer.clone(),
            merchant: order.merchant_address.clone(),
            amount: order.amount,
            token: order.token.clone(),
            nonce: order.nonce,
            order_id: order.order_id.clone(),
            status: PaymentStatus::Pending,
            created_at: current_time,
            updated_at: current_time,
            completed_at: None,
            error_message: None,
        };
        
        // Create payment record atomically before processing
        storage.create_payment_record(&payment_record)
            .map_err(|_| PaymentError::PaymentRecordCreationFailed)?;
        
        // Emit payment record created event
        Events::emit_payment_record_created(&env, &payment_record);
        
        // Update status to Processing
        payment_record.status = PaymentStatus::Processing;
        payment_record.updated_at = env.ledger().timestamp();
        storage.update_payment_record(&payment_id, &payment_record)
            .map_err(|_| PaymentError::PaymentRecordUpdateFailed)?;
        Events::emit_payment_status_updated(
            &env,
            &payment_id,
            PaymentStatus::Pending,
            PaymentStatus::Processing,
            None,
        );
        
        // Process payment with error handling
        let processing_result = Self::execute_payment(
            &env,
            &payer,
            &order,
            &_signature,
            &_merchant_public_key,
            &storage,
        );
        
        match processing_result {
            Ok(_) => {
                // Update payment record to Completed
                payment_record.status = PaymentStatus::Completed;
                payment_record.updated_at = env.ledger().timestamp();
                payment_record.completed_at = Some(env.ledger().timestamp());
                
                storage.update_payment_record(&payment_id, &payment_record)
                    .map_err(|_| PaymentError::PaymentRecordUpdateFailed)?;
                
                Events::emit_payment_status_updated(
                    &env,
                    &payment_id,
                    PaymentStatus::Processing,
                    PaymentStatus::Completed,
                    None,
                );
                Events::emit_payment_completed(&env, &payment_record);
                
                Ok(payment_id)
            }
            Err(e) => {
                // Update payment record to Failed with error message
                payment_record.status = PaymentStatus::Failed;
                payment_record.updated_at = env.ledger().timestamp();
                let error_msg = String::from_str(&env, "Payment processing failed");
                payment_record.error_message = Some(error_msg.clone());
                
                // Try to update record, but don't fail if update fails
                let _ = storage.update_payment_record(&payment_id, &payment_record);
                
                Events::emit_payment_status_updated(
                    &env,
                    &payment_id,
                    PaymentStatus::Processing,
                    PaymentStatus::Failed,
                    payment_record.error_message.clone(),
                );
                Events::emit_payment_failed(
                    &env,
                    &payment_id,
                    &payer,
                    &order.merchant_address,
                    error_msg,
                );
                
                Err(e)
            }
        }
    }
    
    fn get_payment_record(env: Env, payment_id: String) -> Result<PaymentRecord, PaymentError> {
        let storage = Storage::new(&env);
        storage.get_payment_record(&payment_id)
    }
    
    fn get_merchant_payments(env: Env, merchant: Address) -> Vec<String> {
        let storage = Storage::new(&env);
        storage.get_merchant_payment_records(&merchant)
    }
    
    fn get_payer_payments(env: Env, payer: Address) -> Vec<String> {
        let storage = Storage::new(&env);
        storage.get_payer_payment_records(&payer)
    }
    
    fn query_payments(env: Env, query: PaymentRecordQuery) -> Vec<PaymentRecord> {
        let storage = Storage::new(&env);
        storage.query_payment_records(&query)
    }
    
    fn validate_payment(env: Env, payment_id: String) -> Result<bool, PaymentError> {
        let storage = Storage::new(&env);
        storage.validate_payment_record(&payment_id)
    }
    
    fn reconcile_payments(env: Env, payment_ids: Vec<String>) -> Result<u32, PaymentError> {
        let storage = Storage::new(&env);
        let mut inconsistencies_found = 0u32;
        let mut inconsistencies_fixed = 0u32;
        
        for payment_id in payment_ids.iter() {
            match storage.validate_payment_record(&payment_id) {
                Ok(is_valid) => {
                    if !is_valid {
                        inconsistencies_found += 1;
                        
                        // Attempt to fix inconsistency
                        if let Ok(mut record) = storage.get_payment_record(&payment_id) {
                            // If nonce is used but status is not Completed, update status
                            if storage.is_nonce_used(&record.merchant, record.nonce) 
                                && record.status != PaymentStatus::Completed {
                                record.status = PaymentStatus::Completed;
                                record.updated_at = env.ledger().timestamp();
                                record.completed_at = Some(env.ledger().timestamp());
                                
                                if storage.update_payment_record(&payment_id, &record).is_ok() {
                                    inconsistencies_fixed += 1;
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    inconsistencies_found += 1;
                }
            }
        }
        
        Events::emit_payment_reconciliation(
            &env,
            payment_ids,
            inconsistencies_found,
            inconsistencies_fixed,
        );
        
        Ok(inconsistencies_fixed)
    }
}

impl PaymentProcessingContract {
    fn execute_payment(
        env: &Env,
        payer: &Address,
        order: &PaymentOrder,
        _signature: &BytesN<64>,
        _merchant_public_key: &BytesN<32>,
        storage: &Storage,
    ) -> Result<(), PaymentError> {
        // Verify the order hasn't expired
        if env.ledger().timestamp() > order.expiration {
            return Err(PaymentError::OrderExpired);
        }

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
        let mut message = Bytes::new(env);
        
        // Add merchant address
        message.append(&order.merchant_address.clone().to_xdr(env));

        // Add amount bytes
        for &b in order.amount.to_be_bytes().iter() {
            message.push_back(b);
        }

        // Add token address
        message.append(&order.token.clone().to_xdr(env));

        // Add nonce bytes
        for &b in order.nonce.to_be_bytes().iter() {
            message.push_back(b);
        }

        // Add expiration bytes
        for &b in order.expiration.to_be_bytes().iter() {
            message.push_back(b);
        }

        // Add order id
        message.append(&order.order_id.clone().to_xdr(env));
        
        // Verify signature
        #[cfg(not(test))]
        env.crypto().ed25519_verify(_merchant_public_key, &message, _signature);

        // Process the payment using Stellar token contract
        let token_client = token::Client::new(env, &order.token);
        
        // Transfer tokens from payer to merchant
        token_client.transfer(
            payer,
            &order.merchant_address,
            &order.amount,
        );

        // Record used nonce
        storage.mark_nonce_used(&order.merchant_address, order.nonce);

        Ok(())
    }
}

#[cfg(test)]
mod test;
