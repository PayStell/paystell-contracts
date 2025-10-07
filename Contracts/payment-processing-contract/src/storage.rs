use soroban_sdk::{
    contracttype, log, Address, Env, Map, Symbol,
};
use crate::{
    types::{Merchant, NonceTracker, Fee, MultiSigPayment, PaymentRecord, RefundRequest, MultiSigPaymentRecord},
    error::PaymentError,
};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Initialized,
    Merchants,
    NonceTrackers,
    // Multi-signature
    MultiSigPayments,
    PaymentHistory,
    PaymentCounter,
    // Admin / pause / fee
    Paused,
    PausedAdmin,
    PausedUntil,
    Admin,
    Fee,
    // Payment records and refunds
    Payments,
    Refunds,
}

impl DataKey {
    fn as_symbol(self, env: &Env) -> Symbol {
        match self {
            DataKey::Initialized => Symbol::new(env, "initialized"),
            DataKey::Merchants => Symbol::new(env, "merchants"),
            DataKey::NonceTrackers => Symbol::new(env, "nonce_trackers"),
            DataKey::MultiSigPayments => Symbol::new(env, "multisig_payments"),
            DataKey::PaymentHistory => Symbol::new(env, "payment_history"),
            DataKey::PaymentCounter => Symbol::new(env, "payment_counter"),
            DataKey::Paused => Symbol::new(env, "paused"),
            DataKey::PausedAdmin => Symbol::new(env, "paused_admin"),
            DataKey::PausedUntil => Symbol::new(env, "paused_until"),
            DataKey::Admin => Symbol::new(env, "admin"),
            DataKey::Fee => Symbol::new(env, "fee"),
            DataKey::Payments => Symbol::new(env, "payments"),
            DataKey::Refunds => Symbol::new(env, "refunds"),
        }
    }
}

/// Optimized storage with efficient operations
pub struct Storage<'a> {
    env: &'a Env,
}

impl<'a> Storage<'a> {
    pub fn new(env: &'a Env) -> Self {
        Self { env }
    }

    // ===== Pause management =====
    pub fn set_pause_admin_internal(
        &self,
        admin: Address,
        new_admin: Address,
    ) -> Result<(), PaymentError> {
        self.require_admin(&admin)?;
        self.env
            .storage()
            .instance()
            .set(&DataKey::PausedAdmin.as_symbol(self.env), &new_admin);
        Ok(())
    }

    pub fn set_pause_until(&self, timestamp: u64) {
        self.env
            .storage()
            .instance()
            .set(&DataKey::PausedUntil.as_symbol(self.env), &timestamp);
    }

    pub fn get_pause_admin(&self) -> Result<Address, PaymentError> {
        self.env
            .storage()
            .instance()
            .get(&DataKey::PausedAdmin.as_symbol(self.env))
            .ok_or(PaymentError::AdminNotFound)
    }

    pub fn set_pause(&self) {
        self.env
            .storage()
            .instance()
            .set(&DataKey::Paused.as_symbol(self.env), &true);
    }

    pub fn set_unpause(&self) {
        self.env
            .storage()
            .instance()
            .set(&DataKey::Paused.as_symbol(self.env), &false);
    }

    pub fn is_paused(&self) -> bool {
        let pause = self
            .env
            .storage()
            .instance()
            .get(&DataKey::Paused.as_symbol(self.env))
            .unwrap_or(false);
        log!(&self.env, "pause: {}", pause);

        let pause_until: u64 = self
            .env
            .storage()
            .instance()
            .get(&DataKey::PausedUntil.as_symbol(self.env))
            .unwrap_or(0);
        log!(&self.env, "pause_until: {}", pause_until);

        let current_time = self.env.ledger().timestamp();
        if !pause && (pause_until == 0 || current_time > pause_until) {
            return false;
        }
        true
    }

    // ===== Merchant management =====
    pub fn save_merchant(&self, address: &Address, merchant: &Merchant) {
        let mut merchants = self.get_merchants_map();
        merchants.set(address.clone(), merchant.clone());
        self.env
            .storage()
            .instance()
            .set(&DataKey::Merchants.as_symbol(self.env), &merchants);
    }

    pub fn get_merchant(&self, address: &Address) -> Result<Merchant, PaymentError> {
        let merchants = self.get_merchants_map();
        merchants
            .get(address.clone())
            .ok_or(PaymentError::MerchantNotFound)
    }
    fn get_merchants_map(&self) -> Map<Address, Merchant> {
        self.env
            .storage()
            .instance()
            .get(&DataKey::Merchants.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env))
    }

    /// Get nonce trackers map
    fn get_nonce_trackers_map(&self) -> Map<Address, NonceTracker> {
        self.env.storage().instance()
            .get(&DataKey::NonceTrackers.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env))
    }


    /// Check if nonce is used with bitmap optimization
    pub fn is_nonce_used(&self, merchant: &Address, nonce: u32) -> bool {
        let trackers = self.get_nonce_trackers_map();
        if let Some(tracker) = trackers.get(merchant.clone()) {
            tracker.is_nonce_used(nonce)
        } else {
            false
        }
    }

    /// Mark nonce as used with bitmap optimization
    pub fn mark_nonce_used(&self, merchant: &Address, nonce: u32) {
        let mut trackers = self.get_nonce_trackers_map();
        let mut tracker = trackers.get(merchant.clone())
            .unwrap_or_else(|| NonceTracker::new(self.env));
        
        tracker.mark_nonce_used(nonce);
        trackers.set(merchant.clone(), tracker);
        
        // Persist to storage
        self.env.storage().instance().set(
            &DataKey::NonceTrackers.as_symbol(self.env),
            &trackers,
        );
    }

    /// Batch save multiple merchants (gas optimization)
    pub fn batch_save_merchants(&self, merchants_data: &[(Address, Merchant)]) {
        let mut merchants = self.get_merchants_map();
        
        for (address, merchant) in merchants_data {
            merchants.set(address.clone(), merchant.clone());
        }
        
        // Single storage write for all merchants
        self.env.storage().instance().set(
            &DataKey::Merchants.as_symbol(self.env),
            &merchants,
        );
    }

    /// Batch mark multiple nonces as used (gas optimization)
    pub fn batch_mark_nonces_used(&self, merchant: &Address, nonces: &[u32]) {
        let mut trackers = self.get_nonce_trackers_map();
        let mut tracker = trackers.get(merchant.clone())
            .unwrap_or_else(|| NonceTracker::new(self.env));
        
        for &nonce in nonces {
            tracker.mark_nonce_used(nonce);
        }
        
        trackers.set(merchant.clone(), tracker);
        
        // Single storage write
        self.env.storage().instance().set(
            &DataKey::NonceTrackers.as_symbol(self.env),
            &trackers,
        );
    }

    pub fn merchant_exists(&self, address: &Address) -> bool {
        let merchants = self.get_merchants_map();
        merchants.contains_key(address.clone())
    }

    /// Get merchant count (for gas estimation)
    pub fn get_merchant_count(&self) -> u32 {
        let merchants = self.get_merchants_map();
        merchants.len() as u32
    }

    /// Get nonce tracker for a merchant
    pub fn get_nonce_tracker(&self, merchant: &Address) -> Option<NonceTracker> {
        let trackers = self.get_nonce_trackers_map();
        trackers.get(merchant.clone())
    }

    // ===== Multi-signature payment management =====
    pub fn save_multisig_payment(&self, payment: &MultiSigPayment) {
        let mut payments = self.get_multisig_payments_map();
        payments.set(payment.payment_id, payment.clone());
        self.env
            .storage()
            .instance()
            .set(&DataKey::MultiSigPayments.as_symbol(self.env), &payments);
    }

    pub fn get_multisig_payment(
        &self,
        payment_id: u128,
    ) -> Result<MultiSigPayment, PaymentError> {
        let payments = self.get_multisig_payments_map();
        payments.get(payment_id).ok_or(PaymentError::PaymentNotFound)
    }

    pub fn remove_multisig_payment(&self, payment_id: u128) {
        let mut payments = self.get_multisig_payments_map();
        payments.remove(payment_id);
        self.env
            .storage()
            .instance()
            .set(&DataKey::MultiSigPayments.as_symbol(self.env), &payments);
    }

    pub fn archive_payment(&self, record: &MultiSigPaymentRecord) {
        let mut history = self.get_payment_history_map();
        history.set(record.payment_id, record.clone());
        self.env
            .storage()
            .instance()
            .set(&DataKey::PaymentHistory.as_symbol(self.env), &history);
    }

    #[allow(dead_code)]
    pub fn get_payment_record(&self, payment_id: u128) -> Option<MultiSigPaymentRecord> {
        let history = self.get_payment_history_map();
        history.get(payment_id)
    }

    pub fn get_next_payment_id(&self) -> u128 {
        let current_counter: u128 = self
            .env
            .storage()
            .instance()
            .get(&DataKey::PaymentCounter.as_symbol(self.env))
            .unwrap_or(0);
        let next_id = current_counter + 1;
        self.env
            .storage()
            .instance()
            .set(&DataKey::PaymentCounter.as_symbol(self.env), &next_id);
        next_id
    }

    // ===== Payment records management =====
    pub fn save_payment(&self, record: &PaymentRecord) {
        let mut payments = self.get_payments_map();
        payments.set(record.order_id.clone(), record.clone());
        self.env.storage().instance().set(
            &DataKey::Payments.as_symbol(self.env),
            &payments,
        );
    }

    pub fn get_payment(&self, order_id: &soroban_sdk::String) -> Result<PaymentRecord, PaymentError> {
        let payments = self.get_payments_map();
        payments.get(order_id.clone()).ok_or(PaymentError::PaymentNotFound)
    }

    pub fn update_payment(&self, record: &PaymentRecord) {
        let mut payments = self.get_payments_map();
        payments.set(record.order_id.clone(), record.clone());
        self.env.storage().instance().set(
            &DataKey::Payments.as_symbol(self.env),
            &payments,
        );
    }

    // ===== Refund requests management =====
    pub fn save_refund(&self, request: &RefundRequest) {
        let mut refunds = self.get_refunds_map();
        refunds.set(request.refund_id.clone(), request.clone());
        self.env.storage().instance().set(
            &DataKey::Refunds.as_symbol(self.env),
            &refunds,
        );
    }

    pub fn get_refund(&self, refund_id: &soroban_sdk::String) -> Result<RefundRequest, PaymentError> {
        let refunds = self.get_refunds_map();
        refunds.get(refund_id.clone()).ok_or(PaymentError::RefundNotFound)
    }

    pub fn update_refund(&self, request: &RefundRequest) {
        let mut refunds = self.get_refunds_map();
        refunds.set(request.refund_id.clone(), request.clone());
        self.env.storage().instance().set(
            &DataKey::Refunds.as_symbol(self.env),
            &refunds,
        );
    }

    fn get_multisig_payments_map(&self) -> Map<u128, MultiSigPayment> {
        self.env
            .storage()
            .instance()
            .get(&DataKey::MultiSigPayments.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env))
    }

    fn get_payment_history_map(&self) -> Map<u128, MultiSigPaymentRecord> {
        self.env
            .storage()
            .instance()
            .get(&DataKey::PaymentHistory.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env))
    }

    // ===== Admin and fee management =====
    pub fn set_admin(&self, admin: &Address) {
        self.env
            .storage()
            .instance()
            .set(&DataKey::Admin.as_symbol(self.env), &admin);
    }

    fn require_admin(&self, admin: &Address) -> Result<(), PaymentError> {
        let stored_admin: Address = self
            .env
            .storage()
            .instance()
            .get(&DataKey::Admin.as_symbol(self.env))
            .ok_or(PaymentError::AdminNotFound)?;
        if stored_admin != *admin {
            return Err(PaymentError::NotAuthorized);
        }
        Ok(())
    }

    pub fn set_fee_info(
        &self,
        fee: &Fee,
        admin: &Address,
    ) -> Result<(), PaymentError> {
        self.require_admin(admin)?;
        if fee.fee_rate > 10 {
            return Err(PaymentError::InvalidFeeRate);
        }
        self.env
            .storage()
            .instance()
            .set(&DataKey::Fee.as_symbol(self.env), &fee.clone());
        self.env.events().publish(
            ("fee_info_set",),
            (
                fee.fee_rate,
                fee.fee_collector.clone(),
                fee.fee_token.clone(),
            ),
        );
        Ok(())
    }

    pub fn get_fee_rate(&self) -> u64 {
        self.env
            .storage()
            .instance()
            .get::<_, Fee>(&DataKey::Fee.as_symbol(self.env))
            .map(|f| f.fee_rate)
            .unwrap_or(0)
    }

    pub fn get_fee_collector(&self) -> Option<Address> {
        self.env
            .storage()
            .instance()
            .get::<_, Fee>(&DataKey::Fee.as_symbol(self.env))
            .map(|f| f.fee_collector)
    }

    pub fn get_fee_token(&self) -> Option<Address> {
        self.env
            .storage()
            .instance()
            .get::<_, Fee>(&DataKey::Fee.as_symbol(self.env))
            .map(|f| f.fee_token)
    }

    pub fn calculate_fee(&self, amount: i128) -> i128 {
        let rate = i128::from(self.get_fee_rate());
        let quotient = amount / 100;
        let remainder = amount % 100;
        quotient * rate + (remainder * rate) / 100
    }

    pub fn get_admin(&self) -> Option<Address> {
        self.env
            .storage()
            .instance()
            .get(&DataKey::Admin.as_symbol(self.env))
    }

    fn get_payments_map(&self) -> Map<soroban_sdk::String, PaymentRecord> {
        self.env.storage().instance()
            .get(&DataKey::Payments.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env))
    }

    fn get_refunds_map(&self) -> Map<soroban_sdk::String, RefundRequest> {
        self.env.storage().instance()
            .get(&DataKey::Refunds.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env))
    }
}
