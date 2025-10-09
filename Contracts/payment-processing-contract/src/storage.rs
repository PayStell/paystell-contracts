use soroban_sdk::{
    contracttype, log, Address, Env, Map, Symbol,
};
use crate::{
    types::{
        Merchant, NonceTracker, Fee, MultiSigPayment, PaymentRecord, RefundRequest, 
        MultiSigPaymentRecord,
        PaymentIndexEntry, PaymentBucket, MerchantPaymentSummary, 
        PayerPaymentSummary, PaymentStats, CompressedPaymentRecord,
    },
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
     MerchantPaymentIndex,   
    PayerPaymentIndex,        
    TimeBasedIndex,           
    TokenBasedIndex,         
    PaymentMetadata,          
    MerchantStats,            
    PayerStats,               
    GlobalStats,              
    CompressedArchive, 
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
            DataKey::MerchantPaymentIndex => Symbol::new(env, "merch_pay_idx"),
            DataKey::PayerPaymentIndex => Symbol::new(env, "payer_pay_idx"),
            DataKey::TimeBasedIndex => Symbol::new(env, "time_idx"),
            DataKey::TokenBasedIndex => Symbol::new(env, "token_idx"),
            DataKey::PaymentMetadata => Symbol::new(env, "pay_meta"),
            DataKey::MerchantStats => Symbol::new(env, "merch_stats"),
            DataKey::PayerStats => Symbol::new(env, "payer_stats"),
            DataKey::GlobalStats => Symbol::new(env, "global_stats"),
            DataKey::CompressedArchive => Symbol::new(env, "comp_archive"),
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
    
    /// Index a payment record for efficient querying
    pub fn index_payment(&self, record: &PaymentRecord) {
        // 1. Index by merchant
        let mut merchant_index = self.get_merchant_payment_index(&record.merchant_address);
        let entry = PaymentIndexEntry {
            order_id: record.order_id.clone(),
            timestamp: record.paid_at,
            amount: record.amount,
        };
        merchant_index.push_back(entry.clone());
        self.save_merchant_payment_index(&record.merchant_address, &merchant_index);

        // 2. Index by payer
        let mut payer_index = self.get_payer_payment_index(&record.payer_address);
        payer_index.push_back(entry.clone());
        self.save_payer_payment_index(&record.payer_address, &payer_index);

        // 3. Time-based indexing (bucket by day)
        let bucket_timestamp = (record.paid_at / 86400) * 86400; // Start of day
        self.update_time_bucket(bucket_timestamp, record.amount);

        // 4. Token-based indexing
        self.add_to_token_index(&record.token, &record.order_id);

        // 5. Update statistics
        self.update_merchant_stats(&record.merchant_address, record);
        self.update_payer_stats(&record.payer_address, record);
        self.update_global_stats(record);
    }

    /// Get merchant payment index
    fn get_merchant_payment_index(&self, merchant: &Address) -> soroban_sdk::Vec<PaymentIndexEntry> {
        let all_indexes: Map<Address, soroban_sdk::Vec<PaymentIndexEntry>> = self.env
            .storage()
            .persistent()
            .get(&DataKey::MerchantPaymentIndex.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env));
        
        all_indexes.get(merchant.clone()).unwrap_or_else(|| soroban_sdk::Vec::new(self.env))
    }

    /// Save merchant payment index
    fn save_merchant_payment_index(&self, merchant: &Address, index: &soroban_sdk::Vec<PaymentIndexEntry>) {
        let mut all_indexes: Map<Address, soroban_sdk::Vec<PaymentIndexEntry>> = self.env
            .storage()
            .persistent()
            .get(&DataKey::MerchantPaymentIndex.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env));
        
        all_indexes.set(merchant.clone(), index.clone());
        self.env.storage().persistent().set(
            &DataKey::MerchantPaymentIndex.as_symbol(self.env),
            &all_indexes,
        );
    }

    /// Get payer payment index
    fn get_payer_payment_index(&self, payer: &Address) -> soroban_sdk::Vec<PaymentIndexEntry> {
        let all_indexes: Map<Address, soroban_sdk::Vec<PaymentIndexEntry>> = self.env
            .storage()
            .persistent()
            .get(&DataKey::PayerPaymentIndex.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env));
        
        all_indexes.get(payer.clone()).unwrap_or_else(|| soroban_sdk::Vec::new(self.env))
    }

    /// Save payer payment index
    fn save_payer_payment_index(&self, payer: &Address, index: &soroban_sdk::Vec<PaymentIndexEntry>) {
        let mut all_indexes: Map<Address, soroban_sdk::Vec<PaymentIndexEntry>> = self.env
            .storage()
            .persistent()
            .get(&DataKey::PayerPaymentIndex.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env));
        
        all_indexes.set(payer.clone(), index.clone());
        self.env.storage().persistent().set(
            &DataKey::PayerPaymentIndex.as_symbol(self.env),
            &all_indexes,
        );
    }

    /// Update time-based bucket
    fn update_time_bucket(&self, bucket_timestamp: u64, amount: i128) {
        let mut buckets: Map<u64, PaymentBucket> = self.env
            .storage()
            .persistent()
            .get(&DataKey::TimeBasedIndex.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env));
        
        let mut bucket = buckets.get(bucket_timestamp).unwrap_or(PaymentBucket {
            bucket_timestamp,
            payment_count: 0,
            total_volume: 0,
        });
        
        bucket.payment_count += 1;
        bucket.total_volume += amount;
        
        buckets.set(bucket_timestamp, bucket);
        self.env.storage().persistent().set(
            &DataKey::TimeBasedIndex.as_symbol(self.env),
            &buckets,
        );
    }

    /// Add to token index
    fn add_to_token_index(&self, token: &Address, order_id: &soroban_sdk::String) {
        let mut token_index: Map<Address, soroban_sdk::Vec<soroban_sdk::String>> = self.env
            .storage()
            .persistent()
            .get(&DataKey::TokenBasedIndex.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env));
        
        let mut order_ids = token_index.get(token.clone()).unwrap_or_else(|| soroban_sdk::Vec::new(self.env));
        order_ids.push_back(order_id.clone());
        token_index.set(token.clone(), order_ids);
        
        self.env.storage().persistent().set(
            &DataKey::TokenBasedIndex.as_symbol(self.env),
            &token_index,
        );
    }

    /// Update merchant statistics
    fn update_merchant_stats(&self, merchant: &Address, record: &PaymentRecord) {
        let mut stats_map: Map<Address, MerchantPaymentSummary> = self.env
            .storage()
            .persistent()
            .get(&DataKey::MerchantStats.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env));
        
        let mut stats = stats_map.get(merchant.clone()).unwrap_or(MerchantPaymentSummary {
            merchant_address: merchant.clone(),
            total_received: 0,
            payment_count: 0,
            refund_count: 0,
            total_refunded: 0,
            net_received: 0,
            active_since: record.paid_at,
            last_payment: None,
        });
        
        stats.total_received += record.amount;
        stats.payment_count += 1;
        stats.net_received = stats.total_received - stats.total_refunded;
        stats.last_payment = Some(record.paid_at);
        
        stats_map.set(merchant.clone(), stats);
        self.env.storage().persistent().set(
            &DataKey::MerchantStats.as_symbol(self.env),
            &stats_map,
        );
    }

    /// Update payer statistics
    fn update_payer_stats(&self, payer: &Address, record: &PaymentRecord) {
        let mut stats_map: Map<Address, PayerPaymentSummary> = self.env
            .storage()
            .persistent()
            .get(&DataKey::PayerStats.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env));
        
        let mut stats = stats_map.get(payer.clone()).unwrap_or(PayerPaymentSummary {
            payer_address: payer.clone(),
            total_spent: 0,
            payment_count: 0,
            unique_merchants: 0,
            first_payment: Some(record.paid_at),
            last_payment: None,
        });
        
        stats.total_spent += record.amount;
        stats.payment_count += 1;
        stats.last_payment = Some(record.paid_at);
        
        stats_map.set(payer.clone(), stats);
        self.env.storage().persistent().set(
            &DataKey::PayerStats.as_symbol(self.env),
            &stats_map,
        );
    }

    /// Update global statistics
    fn update_global_stats(&self, record: &PaymentRecord) {
        let mut stats = self.env
            .storage()
            .persistent()
            .get::<_, PaymentStats>(&DataKey::GlobalStats.as_symbol(self.env))
            .unwrap_or(PaymentStats {
                total_payments: 0,
                total_volume: 0,
                unique_payers: 0,
                average_payment: 0,
                first_payment_time: Some(record.paid_at),
                last_payment_time: None,
            });
        
        stats.total_payments += 1;
        stats.total_volume += record.amount;
        stats.average_payment = stats.total_volume / i128::from(stats.total_payments);
        stats.last_payment_time = Some(record.paid_at);
        
        self.env.storage().persistent().set(
            &DataKey::GlobalStats.as_symbol(self.env),
            &stats,
        );
    }

    // ===== Query Functions =====
    
    /// Get payments by merchant with pagination
    pub fn get_merchant_payments(
        &self,
        merchant: &Address,
        limit: u32,
        offset: u32,
    ) -> soroban_sdk::Vec<PaymentRecord> {
        let index = self.get_merchant_payment_index(merchant);
        let mut results = soroban_sdk::Vec::new(self.env);
        
        let start = offset as usize;
        let end = (offset + limit) as usize;
        
        for i in start..end.min(index.len() as usize) {
            if let Some(entry) = index.get(i as u32) {
                if let Ok(payment) = self.get_payment(&entry.order_id) {
                    results.push_back(payment);
                }
            }
        }
        
        results
    }

    /// Get payments by payer with pagination
    pub fn get_payer_payments(
        &self,
        payer: &Address,
        limit: u32,
        offset: u32,
    ) -> soroban_sdk::Vec<PaymentRecord> {
        let index = self.get_payer_payment_index(payer);
        let mut results = soroban_sdk::Vec::new(self.env);
        
        let start = offset as usize;
        let end = (offset + limit) as usize;
        
        for i in start..end.min(index.len() as usize) {
            if let Some(entry) = index.get(i as u32) {
                if let Ok(payment) = self.get_payment(&entry.order_id) {
                    results.push_back(payment);
                }
            }
        }
        
        results
    }

    /// Get merchant statistics
    pub fn get_merchant_stats(&self, merchant: &Address) -> Option<MerchantPaymentSummary> {
        let stats_map: Map<Address, MerchantPaymentSummary> = self.env
            .storage()
            .persistent()
            .get(&DataKey::MerchantStats.as_symbol(self.env))?;
        
        stats_map.get(merchant.clone())
    }

    /// Get payer statistics
    pub fn get_payer_stats(&self, payer: &Address) -> Option<PayerPaymentSummary> {
        let stats_map: Map<Address, PayerPaymentSummary> = self.env
            .storage()
            .persistent()
            .get(&DataKey::PayerStats.as_symbol(self.env))?;
        
        stats_map.get(payer.clone())
    }

    /// Get global statistics
    pub fn get_global_stats(&self) -> Option<PaymentStats> {
        self.env
            .storage()
            .persistent()
            .get(&DataKey::GlobalStats.as_symbol(self.env))
    }

    /// Get payments by time range
    pub fn get_payments_by_time_range(
        &self,
        start_time: u64,
        end_time: u64,
    ) -> soroban_sdk::Vec<PaymentBucket> {
        let buckets: Map<u64, PaymentBucket> = self.env
            .storage()
            .persistent()
            .get(&DataKey::TimeBasedIndex.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env));
        
        let mut results = soroban_sdk::Vec::new(self.env);
        let start_bucket = (start_time / 86400) * 86400;
        let end_bucket = (end_time / 86400) * 86400;
        
        let mut current = start_bucket;
        while current <= end_bucket {
            if let Some(bucket) = buckets.get(current) {
                results.push_back(bucket);
            }
            current += 86400;
        }
        
        results
    }

    /// Compress and archive old payment records in batches
    /// Returns number of records processed in this batch
    pub fn compress_old_payments(&self, cutoff_time: u64, batch_size: u32) -> u32 {
        let payments = self.get_payments_map();
        let mut compressed_archive: Map<soroban_sdk::String, CompressedPaymentRecord> = self.env
            .storage()
            .persistent()
            .get(&DataKey::CompressedArchive.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env));

        // Persistent cursor for batch progress
        let cursor_key = DataKey::CompressionCursor.as_symbol(self.env);
        let mut start_idx: usize = self.env
            .storage()
            .persistent()
            .get(&cursor_key)
            .unwrap_or(0u32) as usize;

        let mut processed = 0u32;
        let mut idx = 0usize;
        let mut next_cursor = None;

        for (order_id, payment) in payments.iter() {
            if idx < start_idx {
                idx += 1;
                continue;
            }
            if processed >= batch_size {
                next_cursor = Some(idx as u32);
                break;
            }
            if payment.paid_at < cutoff_time {
                let status = if payment.refunded_amount == payment.amount {
                    2 // Fully refunded
                } else if payment.refunded_amount > 0 {
                    1 // Partially refunded
                } else {
                    0 // Paid
                };

                let compressed = CompressedPaymentRecord {
                    order_id: order_id.clone(),
                    merchant_address: payment.merchant_address.clone(),
                    payer_address: payment.payer_address.clone(),
                    token: payment.token.clone(),
                    amount: payment.amount,
                    paid_at: payment.paid_at,
                    status,
                };

                compressed_archive.set(order_id, compressed);
                processed += 1;
            }
            idx += 1;
        }

        self.env.storage().persistent().set(
            &DataKey::CompressedArchive.as_symbol(self.env),
            &compressed_archive,
        );

        // Update or clear cursor
        if let Some(cursor) = next_cursor {
            self.env.storage().persistent().set(&cursor_key, &cursor);
        } else {
            self.env.storage().persistent().remove(&cursor_key);
        }

        processed
    }

}
