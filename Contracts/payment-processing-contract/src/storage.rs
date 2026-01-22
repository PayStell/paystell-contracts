use soroban_sdk::{
    contracttype, log, Address, Env, Map, Symbol, String, Vec,
};
use crate::{
    types::{Merchant, NonceTracker, Fee, MultiSigPayment, PaymentRecord, RefundRequest, MultiSigPaymentRecord, PaymentQueryFilter, SortField, SortOrder},
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
    // Payment history query indices
    MerchantPaymentIndices,  // Map<Address, Vec<String>> - merchant -> order_ids
    PayerPaymentIndices,     // Map<Address, Vec<String>> - payer -> order_ids
    PaymentCleanupPeriod,    // u64 - cleanup period in seconds
    PaymentArchive,          // Map<String, PaymentRecord> - archived payments
}

impl DataKey {
    pub fn as_symbol(self, env: &Env) -> Symbol {
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
            DataKey::MerchantPaymentIndices => Symbol::new(env, "merchant_pay_idx"),
            DataKey::PayerPaymentIndices => Symbol::new(env, "payer_pay_idx"),
            DataKey::PaymentCleanupPeriod => Symbol::new(env, "pay_cleanup_period"),
            DataKey::PaymentArchive => Symbol::new(env, "pay_archive"),
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

    pub fn get_payments_map(&self) -> Map<soroban_sdk::String, PaymentRecord> {
        self.env.storage().instance()
            .get(&DataKey::Payments.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env))
    }

    fn get_refunds_map(&self) -> Map<soroban_sdk::String, RefundRequest> {
        self.env.storage().instance()
            .get(&DataKey::Refunds.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env))
    }

    // ===== Payment History Query & Management =====

    /// Get merchant payment indices map
    fn get_merchant_payment_indices_map(&self) -> Map<Address, Vec<String>> {
        self.env.storage().instance()
            .get(&DataKey::MerchantPaymentIndices.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env))
    }

    /// Get payer payment indices map
    fn get_payer_payment_indices_map(&self) -> Map<Address, Vec<String>> {
        self.env.storage().instance()
            .get(&DataKey::PayerPaymentIndices.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env))
    }

    /// Add order_id to merchant payment index
    pub fn save_merchant_payment_index(&self, merchant: &Address, order_id: &String) {
        let mut indices = self.get_merchant_payment_indices_map();
        let mut order_ids = indices.get(merchant.clone())
            .unwrap_or_else(|| Vec::new(self.env));
        order_ids.push_back(order_id.clone());
        indices.set(merchant.clone(), order_ids);
        self.env.storage().instance().set(
            &DataKey::MerchantPaymentIndices.as_symbol(self.env),
            &indices,
        );
    }

    /// Add order_id to payer payment index
    pub fn save_payer_payment_index(&self, payer: &Address, order_id: &String) {
        let mut indices = self.get_payer_payment_indices_map();
        let mut order_ids = indices.get(payer.clone())
            .unwrap_or_else(|| Vec::new(self.env));
        order_ids.push_back(order_id.clone());
        indices.set(payer.clone(), order_ids);
        self.env.storage().instance().set(
            &DataKey::PayerPaymentIndices.as_symbol(self.env),
            &indices,
        );
    }

    /// Get all order_ids for a merchant
    pub fn get_merchant_payment_indices(&self, merchant: &Address) -> Vec<String> {
        let indices = self.get_merchant_payment_indices_map();
        indices.get(merchant.clone())
            .unwrap_or_else(|| Vec::new(self.env))
    }

    /// Get all order_ids for a payer
    pub fn get_payer_payment_indices(&self, payer: &Address) -> Vec<String> {
        let indices = self.get_payer_payment_indices_map();
        indices.get(payer.clone())
            .unwrap_or_else(|| Vec::new(self.env))
    }

    /// Remove order_id from merchant payment index
    pub fn remove_merchant_payment_index(&self, merchant: &Address, order_id: &String) {
        let mut indices = self.get_merchant_payment_indices_map();
        if let Some(order_ids) = indices.get(merchant.clone()) {
            let mut new_order_ids = Vec::new(self.env);
            for id in order_ids.iter() {
                if id != *order_id {
                    new_order_ids.push_back(id);
                }
            }
            if new_order_ids.len() > 0 {
                indices.set(merchant.clone(), new_order_ids);
            } else {
                indices.remove(merchant.clone());
            }
            self.env.storage().instance().set(
                &DataKey::MerchantPaymentIndices.as_symbol(self.env),
                &indices,
            );
        }
    }

    /// Remove order_id from payer payment index
    pub fn remove_payer_payment_index(&self, payer: &Address, order_id: &String) {
        let mut indices = self.get_payer_payment_indices_map();
        if let Some(order_ids) = indices.get(payer.clone()) {
            let mut new_order_ids = Vec::new(self.env);
            for id in order_ids.iter() {
                if id != *order_id {
                    new_order_ids.push_back(id);
                }
            }
            if new_order_ids.len() > 0 {
                indices.set(payer.clone(), new_order_ids);
            } else {
                indices.remove(payer.clone());
            }
            self.env.storage().instance().set(
                &DataKey::PayerPaymentIndices.as_symbol(self.env),
                &indices,
            );
        }
    }

    /// Query payments with filters
    pub fn query_payments_with_filters(
        &self,
        order_ids: &Vec<String>,
        filter: &PaymentQueryFilter,
    ) -> Vec<PaymentRecord> {
        let payments = self.get_payments_map();
        let mut results = Vec::new(self.env);

        for order_id in order_ids.iter() {
            if let Some(record) = payments.get(order_id.clone()) {
                // Apply date filter
                if let Some(date_start) = filter.date_start {
                    if record.paid_at < date_start {
                        continue;
                    }
                }
                if let Some(date_end) = filter.date_end {
                    if record.paid_at > date_end {
                        continue;
                    }
                }

                // Apply amount filter
                if let Some(amount_min) = filter.amount_min {
                    if record.amount < amount_min {
                        continue;
                    }
                }
                if let Some(amount_max) = filter.amount_max {
                    if record.amount > amount_max {
                        continue;
                    }
                }

                // Apply token filter
                if let Some(ref token) = filter.token {
                    if record.token != *token {
                        continue;
                    }
                }

                // Apply status filter
                if filter.status != crate::types::PaymentRecordStatus::Any {
                    let record_status = record.get_status();
                    if record_status != filter.status {
                        continue;
                    }
                }

                results.push_back(record);
            }
        }

        results
    }

    /// Sort payments by field and order
    pub fn sort_payments(
        &self,
        records: Vec<PaymentRecord>,
        field: &SortField,
        order: &SortOrder,
    ) -> Vec<PaymentRecord> {
        // Build sorted Vec by comparing and inserting in order
        let mut sorted: Vec<PaymentRecord> = Vec::new(self.env);
        
        for record in records.iter() {
            let mut inserted = false;
            let mut new_sorted = Vec::new(self.env);
            
            // Find insertion point
            for sorted_record in sorted.iter() {
                let should_insert_before = match field {
                    SortField::Date => {
                        let cmp = sorted_record.paid_at.cmp(&record.paid_at);
                        match order {
                            SortOrder::Ascending => cmp == core::cmp::Ordering::Greater,
                            SortOrder::Descending => cmp == core::cmp::Ordering::Less,
                        }
                    }
                    SortField::Amount => {
                        let cmp = sorted_record.amount.cmp(&record.amount);
                        match order {
                            SortOrder::Ascending => cmp == core::cmp::Ordering::Greater,
                            SortOrder::Descending => cmp == core::cmp::Ordering::Less,
                        }
                    }
                };
                
                if should_insert_before && !inserted {
                    new_sorted.push_back(record.clone());
                    inserted = true;
                }
                new_sorted.push_back(sorted_record.clone());
            }
            
            if !inserted {
                new_sorted.push_back(record.clone());
            }
            
            sorted = new_sorted;
        }

        sorted
    }

    /// Paginate payments with cursor
    pub fn paginate_payments(
        &self,
        records: Vec<PaymentRecord>,
        cursor: Option<String>,
        limit: u32,
    ) -> (Vec<PaymentRecord>, Option<String>) {
        let mut start_idx = 0u32;
        
        // Find cursor position if provided
        if let Some(ref cursor_id) = cursor {
            for (idx, record) in records.iter().enumerate() {
                if record.order_id == *cursor_id {
                    start_idx = (idx + 1) as u32;
                    break;
                }
            }
        }

        let mut paginated = Vec::new(self.env);
        let mut next_cursor: Option<String> = None;
        let max_idx = core::cmp::min(start_idx + limit, records.len() as u32);

        for i in start_idx..max_idx {
            if let Some(record) = records.get(i) {
                paginated.push_back(record.clone());
            }
        }

        // Set next cursor if there are more results
        if max_idx < records.len() as u32 {
            if paginated.len() > 0 {
                if let Some(last_record) = paginated.get((paginated.len() - 1) as u32) {
                    next_cursor = Some(last_record.order_id.clone());
                }
            }
        }

        (paginated, next_cursor)
    }

    /// Set payment cleanup period (in seconds)
    pub fn set_cleanup_period(&self, period: u64) {
        self.env.storage().instance().set(
            &DataKey::PaymentCleanupPeriod.as_symbol(self.env),
            &period,
        );
    }

    /// Get payment cleanup period (in seconds)
    pub fn get_cleanup_period(&self) -> u64 {
        self.env.storage().instance()
            .get(&DataKey::PaymentCleanupPeriod.as_symbol(self.env))
            .unwrap_or(365 * 24 * 60 * 60) // Default: 365 days
    }

    /// Get payment archive map
    fn get_payment_archive_map(&self) -> Map<String, PaymentRecord> {
        self.env.storage().instance()
            .get(&DataKey::PaymentArchive.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env))
    }

    /// Archive a payment record
    pub fn archive_payment_record(&self, record: &PaymentRecord) {
        let mut archive = self.get_payment_archive_map();
        archive.set(record.order_id.clone(), record.clone());
        self.env.storage().instance().set(
            &DataKey::PaymentArchive.as_symbol(self.env),
            &archive,
        );
    }

    /// Get archived payment record
    pub fn get_archived_payment(&self, order_id: &String) -> Option<PaymentRecord> {
        let archive = self.get_payment_archive_map();
        archive.get(order_id.clone())
    }
}
