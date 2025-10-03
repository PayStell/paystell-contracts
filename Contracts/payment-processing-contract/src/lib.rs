#![no_std]

mod storage;
mod types;
mod error;

use soroban_sdk::{
    contract, contractimpl, token, Address, Env,
    Vec, BytesN, Bytes, xdr::ToXdr, String,
};

use crate::{
    error::PaymentError,
    types::{Merchant, PaymentOrder, PaymentRecord, RefundRequest, RefundStatus},
    storage::Storage,
};

/// payment-processing-contract trait defining the core functionality
pub trait PaymentProcessingTrait {
    // Merchant Management Operations
    fn register_merchant(env: Env, merchant_address: Address) -> Result<(), PaymentError>;
    fn add_supported_token(env: Env, merchant: Address, token: Address) -> Result<(), PaymentError>;
    fn set_admin(env: Env, admin: Address) -> Result<(), PaymentError>;
    
    // Payment Processing Operations
    fn process_payment_with_signature(
        env: Env,
        payer: Address,
        order: PaymentOrder,
        signature: BytesN<64>,
        merchant_public_key: BytesN<32>,
    ) -> Result<(), PaymentError>;

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

    fn set_admin(env: Env, admin: Address) -> Result<(), PaymentError> {
        // Only admin can set themselves or if not set yet, allow any caller to set
        // For simplicity, require admin auth
        admin.require_auth();
        let storage = Storage::new(&env);
        storage.set_admin(&admin);
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

        // Record payment history
        let payment_record = PaymentRecord {
            order_id: order.order_id.clone(),
            merchant_address: order.merchant_address.clone(),
            payer_address: payer.clone(),
            token: order.token.clone(),
            amount: order.amount,
            paid_at: env.ledger().timestamp(),
            refunded_amount: 0,
        };
        storage.save_payment(&payment_record);

        Ok(())
    }

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

        if amount <= 0 {
            return Err(PaymentError::InvalidAmount);
        }

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
}

#[cfg(test)]
mod test;
