use soroban_sdk::{
    contracttype, symbol_short, Address, Env, String, Vec,
};
use crate::types::{PaymentRecord, PaymentStatus};

#[contracttype]
#[derive(Clone)]
pub struct PaymentRecordCreatedEvent {
    pub payment_id: String,
    pub payer: Address,
    pub merchant: Address,
    pub amount: i128,
    pub token: Address,
    pub order_id: String,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct PaymentStatusUpdatedEvent {
    pub payment_id: String,
    pub old_status: PaymentStatus,
    pub new_status: PaymentStatus,
    pub timestamp: u64,
    pub error_message: Option<String>,
}

#[contracttype]
#[derive(Clone)]
pub struct PaymentCompletedEvent {
    pub payment_id: String,
    pub payer: Address,
    pub merchant: Address,
    pub amount: i128,
    pub token: Address,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct PaymentFailedEvent {
    pub payment_id: String,
    pub payer: Address,
    pub merchant: Address,
    pub error_message: String,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct PaymentReconciliationEvent {
    pub payment_ids: Vec<String>,
    pub inconsistencies_found: u32,
    pub inconsistencies_fixed: u32,
    pub timestamp: u64,
}

pub struct Events;

impl Events {
    pub fn emit_payment_record_created(env: &Env, record: &PaymentRecord) {
        let event = PaymentRecordCreatedEvent {
            payment_id: record.payment_id.clone(),
            payer: record.payer.clone(),
            merchant: record.merchant.clone(),
            amount: record.amount,
            token: record.token.clone(),
            order_id: record.order_id.clone(),
            timestamp: record.created_at,
        };
        env.events().publish((symbol_short!("pay_creat"),), event);
    }

    pub fn emit_payment_status_updated(
        env: &Env,
        payment_id: &String,
        old_status: PaymentStatus,
        new_status: PaymentStatus,
        error_message: Option<String>,
    ) {
        let event = PaymentStatusUpdatedEvent {
            payment_id: payment_id.clone(),
            old_status,
            new_status,
            timestamp: env.ledger().timestamp(),
            error_message,
        };
        env.events().publish((symbol_short!("pay_stat"),), event);
    }

    pub fn emit_payment_completed(env: &Env, record: &PaymentRecord) {
        let event = PaymentCompletedEvent {
            payment_id: record.payment_id.clone(),
            payer: record.payer.clone(),
            merchant: record.merchant.clone(),
            amount: record.amount,
            token: record.token.clone(),
            timestamp: env.ledger().timestamp(),
        };
        env.events().publish((symbol_short!("pay_done"),), event);
    }

    pub fn emit_payment_failed(
        env: &Env,
        payment_id: &String,
        payer: &Address,
        merchant: &Address,
        error_message: String,
    ) {
        let event = PaymentFailedEvent {
            payment_id: payment_id.clone(),
            payer: payer.clone(),
            merchant: merchant.clone(),
            error_message,
            timestamp: env.ledger().timestamp(),
        };
        env.events().publish((symbol_short!("pay_fail"),), event);
    }

    pub fn emit_payment_reconciliation(
        env: &Env,
        payment_ids: Vec<String>,
        inconsistencies_found: u32,
        inconsistencies_fixed: u32,
    ) {
        let event = PaymentReconciliationEvent {
            payment_ids,
            inconsistencies_found,
            inconsistencies_fixed,
            timestamp: env.ledger().timestamp(),
        };
        env.events().publish((symbol_short!("pay_recon"),), event);
    }
}
