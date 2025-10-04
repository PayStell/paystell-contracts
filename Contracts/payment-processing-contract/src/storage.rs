use soroban_sdk::{
    contracttype,
    Env, Symbol, Map, Vec, Address, String,
};
use crate::{
    types::{Merchant, PaymentRecord, PaymentStatus, PaymentRecordQuery, QueryFilter},
    error::PaymentError,
};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Merchants,
    UsedNonces,
    PaymentRecords,
    PaymentsByMerchant,
    PaymentsByPayer,
    PaymentCounter,
}

impl DataKey {
    fn as_symbol(self, env: &Env) -> Symbol {
        match self {
            DataKey::Merchants => Symbol::new(env, "merchants"),
            DataKey::UsedNonces => Symbol::new(env, "used_nonces"),
            DataKey::PaymentRecords => Symbol::new(env, "pay_records"),
            DataKey::PaymentsByMerchant => Symbol::new(env, "pay_merchant"),
            DataKey::PaymentsByPayer => Symbol::new(env, "pay_payer"),
            DataKey::PaymentCounter => Symbol::new(env, "pay_counter"),
        }
    }
}

pub struct Storage<'a> {
    env: &'a Env,
}

impl<'a> Storage<'a> {
    pub fn new(env: &'a Env) -> Self {
        Self { env }
    }

    pub fn save_merchant(&self, address: &Address, merchant: &Merchant) {
        let mut merchants = self.get_merchants_map();
        merchants.set(address.clone(), merchant.clone());
        self.env.storage().instance().set(
            &DataKey::Merchants.as_symbol(self.env),
            &merchants,
        );
    }

    pub fn get_merchant(&self, address: &Address) -> Result<Merchant, PaymentError> {
        let merchants = self.get_merchants_map();
        merchants.get(address.clone())
            .ok_or(PaymentError::MerchantNotFound)
    }

    pub fn is_nonce_used(&self, merchant: &Address, nonce: u64) -> bool {
        let nonces = self.get_merchant_nonces(merchant);
        nonces.contains(&nonce)
    }

    pub fn mark_nonce_used(&self, merchant: &Address, nonce: u64) {
        let mut nonces = self.get_merchant_nonces(merchant);
        nonces.push_back(nonce);
        let mut used_nonces = self.get_used_nonces_map();
        used_nonces.set(merchant.clone(), nonces);
        self.env.storage().instance().set(
            &DataKey::UsedNonces.as_symbol(self.env),
            &used_nonces,
        );
    }

    fn get_merchants_map(&self) -> Map<Address, Merchant> {
        self.env.storage().instance()
            .get(&DataKey::Merchants.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env))
    }

    fn get_used_nonces_map(&self) -> Map<Address, Vec<u64>> {
        self.env.storage().instance()
            .get(&DataKey::UsedNonces.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env))
    }

    fn get_merchant_nonces(&self, merchant: &Address) -> Vec<u64> {
        let used_nonces = self.get_used_nonces_map();
        used_nonces.get(merchant.clone())
            .unwrap_or_else(|| Vec::new(self.env))
    }

    // Payment History Operations
    pub fn create_payment_record(&self, record: &PaymentRecord) -> Result<(), PaymentError> {
        let mut records = self.get_payment_records_map();
        records.set(record.payment_id.clone(), record.clone());
        self.env.storage().instance().set(
            &DataKey::PaymentRecords.as_symbol(self.env),
            &records,
        );

        // Index by merchant
        let mut merchant_payments = self.get_merchant_payments(&record.merchant);
        merchant_payments.push_back(record.payment_id.clone());
        let mut merchant_map = self.get_payments_by_merchant_map();
        merchant_map.set(record.merchant.clone(), merchant_payments);
        self.env.storage().instance().set(
            &DataKey::PaymentsByMerchant.as_symbol(self.env),
            &merchant_map,
        );

        // Index by payer
        let mut payer_payments = self.get_payer_payments(&record.payer);
        payer_payments.push_back(record.payment_id.clone());
        let mut payer_map = self.get_payments_by_payer_map();
        payer_map.set(record.payer.clone(), payer_payments);
        self.env.storage().instance().set(
            &DataKey::PaymentsByPayer.as_symbol(self.env),
            &payer_map,
        );

        Ok(())
    }

    pub fn update_payment_record(&self, payment_id: &String, record: &PaymentRecord) -> Result<(), PaymentError> {
        let mut records = self.get_payment_records_map();
        if !records.contains_key(payment_id.clone()) {
            return Err(PaymentError::PaymentRecordNotFound);
        }
        records.set(payment_id.clone(), record.clone());
        self.env.storage().instance().set(
            &DataKey::PaymentRecords.as_symbol(self.env),
            &records,
        );
        Ok(())
    }

    pub fn get_payment_record(&self, payment_id: &String) -> Result<PaymentRecord, PaymentError> {
        let records = self.get_payment_records_map();
        records.get(payment_id.clone())
            .ok_or(PaymentError::PaymentRecordNotFound)
    }

    pub fn get_merchant_payment_records(&self, merchant: &Address) -> Vec<String> {
        self.get_merchant_payments(merchant)
    }

    pub fn get_payer_payment_records(&self, payer: &Address) -> Vec<String> {
        self.get_payer_payments(payer)
    }

    pub fn query_payment_records(&self, query: &PaymentRecordQuery) -> Vec<PaymentRecord> {
        let records = self.get_payment_records_map();
        let mut results = Vec::new(self.env);

        // Get candidate payment IDs based on query filter
        let payment_ids = match &query.filter {
            QueryFilter::ByMerchant(merchant) => self.get_merchant_payments(merchant),
            QueryFilter::ByPayer(payer) => self.get_payer_payments(payer),
        };

        // Filter records based on query criteria
        for payment_id in payment_ids.iter() {
            if let Some(record) = records.get(payment_id.clone()) {
                let mut matches = true;

                if let Some(from_ts) = query.from_timestamp {
                    if record.created_at < from_ts {
                        matches = false;
                    }
                }

                if let Some(to_ts) = query.to_timestamp {
                    if record.created_at > to_ts {
                        matches = false;
                    }
                }

                if matches {
                    results.push_back(record);
                }
            }
        }

        results
    }

    pub fn generate_payment_id(&self) -> String {
        let counter = self.get_payment_counter();
        let new_counter = counter + 1;
        self.env.storage().instance().set(
            &DataKey::PaymentCounter.as_symbol(self.env),
            &new_counter,
        );
        let mut bytes: [u8; 32] = [0; 32];
        bytes[0..4].copy_from_slice(b"PAY_");
        let mut len = 4usize;

        let mut value = new_counter;
        if value == 0 {
            bytes[len] = b'0';
            len += 1;
        } else {
            let mut digits: [u8; 20] = [0; 20];
            let mut index = 0usize;
            while value > 0 && index < digits.len() {
                digits[index] = (value % 10) as u8;
                value /= 10;
                index += 1;
            }

            while index > 0 {
                index -= 1;
                bytes[len] = b'0' + digits[index];
                len += 1;
            }
        }

        String::from_bytes(self.env, &bytes[..len])
    }

    pub fn validate_payment_record(&self, payment_id: &String) -> Result<bool, PaymentError> {
        let record = self.get_payment_record(payment_id)?;
        
        // Validate nonce usage matches payment status
        if record.status == PaymentStatus::Completed {
            if !self.is_nonce_used(&record.merchant, record.nonce) {
                return Ok(false);
            }
        }
        
        Ok(true)
    }

    fn get_payment_records_map(&self) -> Map<String, PaymentRecord> {
        self.env.storage().instance()
            .get(&DataKey::PaymentRecords.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env))
    }

    fn get_payments_by_merchant_map(&self) -> Map<Address, Vec<String>> {
        self.env.storage().instance()
            .get(&DataKey::PaymentsByMerchant.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env))
    }

    fn get_payments_by_payer_map(&self) -> Map<Address, Vec<String>> {
        self.env.storage().instance()
            .get(&DataKey::PaymentsByPayer.as_symbol(self.env))
            .unwrap_or_else(|| Map::new(self.env))
    }

    fn get_merchant_payments(&self, merchant: &Address) -> Vec<String> {
        let merchant_map = self.get_payments_by_merchant_map();
        merchant_map.get(merchant.clone())
            .unwrap_or_else(|| Vec::new(self.env))
    }

    fn get_payer_payments(&self, payer: &Address) -> Vec<String> {
        let payer_map = self.get_payments_by_payer_map();
        payer_map.get(payer.clone())
            .unwrap_or_else(|| Vec::new(self.env))
    }

    fn get_payment_counter(&self) -> u64 {
        self.env.storage().instance()
            .get(&DataKey::PaymentCounter.as_symbol(self.env))
            .unwrap_or(0)
    }
} 