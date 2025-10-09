#![cfg(test)]

use crate::{
    PaymentProcessingContract, PaymentProcessingContractClient,
    types::{MerchantCategory, PaymentOrder, PaymentQueryParams},
};
use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    token, Address, Env, String, Vec,
};

fn create_token_contract<'a>(env: &Env, admin: &Address) -> token::Client<'a> {
    token::Client::new(env, &env.register_stellar_asset_contract(admin.clone()))
}

fn setup_test_environment() -> (
    Env,
    PaymentProcessingContractClient<'static>,
    Address,
    Address,
    Address,
    Address,
    token::Client<'static>,
) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, PaymentProcessingContract);
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let merchant = Address::generate(&env);
    let payer = Address::generate(&env);
    let fee_collector = Address::generate(&env);
    
    let token_admin = Address::generate(&env);
    let token = create_token_contract(&env, &token_admin);

    // Mint tokens to payer
    token.mint(&payer, &1_000_000);

    // Initialize contract
    client.set_admin(&admin);
    client.set_fee(&5, &fee_collector, &token.address);

    // Register merchant
    client.register_merchant(
        &merchant,
        &String::from_str(&env, "Test Merchant"),
        &String::from_str(&env, "A test merchant"),
        &String::from_str(&env, "contact@test.com"),
        &MerchantCategory::Retail,
    );

    // Add supported token
    client.add_supported_token(&merchant, &token.address);

    (env, client, admin, merchant, payer, fee_collector, token)
}

#[test]
fn test_payment_history_recording() {
    let (env, client, _admin, merchant, payer, _fee_collector, token) = setup_test_environment();

    // Create and process a payment
    let order = PaymentOrder {
        merchant_address: merchant.clone(),
        amount: 1000,
        token: token.address.clone(),
        nonce: 1,
        expiration: env.ledger().timestamp() as u32 + 3600,
        order_id: String::from_str(&env, "ORDER001"),
        fee_amount: 50,
    };

    let signature = soroban_sdk::BytesN::from_array(&env, &[0u8; 64]);
    let public_key = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);

    client.process_payment_with_signature(&payer, &order, &signature, &public_key);

    // Verify payment was recorded
    let payment = client.get_payment_by_order_id(&String::from_str(&env, "ORDER001"));
    assert_eq!(payment.amount, 1000);
    assert_eq!(payment.merchant_address, merchant);
    assert_eq!(payment.payer_address, payer);
}

#[test]
fn test_merchant_payment_history_query() {
    let (env, client, _admin, merchant, payer, _fee_collector, token) = setup_test_environment();

    // Process multiple payments
    for i in 1..=5 {
        let order = PaymentOrder {
            merchant_address: merchant.clone(),
            amount: 1000 + (i as i64 * 100),
            token: token.address.clone(),
            nonce: i,
            expiration: env.ledger().timestamp() as u32 + 3600,
            order_id: String::from_str(&env, &format!("ORDER{:03}", i)),
            fee_amount: 50,
        };

        let signature = soroban_sdk::BytesN::from_array(&env, &[0u8; 64]);
        let public_key = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);

        client.process_payment_with_signature(&payer, &order, &signature, &public_key);
    }

    // Query merchant payment history
    let history = client.get_merchant_payment_history(&merchant, &10, &0);
    assert_eq!(history.len(), 5);

    // Verify pagination
    let page1 = client.get_merchant_payment_history(&merchant, &2, &0);
    assert_eq!(page1.len(), 2);

    let page2 = client.get_merchant_payment_history(&merchant, &2, &2);
    assert_eq!(page2.len(), 2);
}

#[test]
fn test_payer_payment_history_query() {
    let (env, client, _admin, merchant, payer, _fee_collector, token) = setup_test_environment();

    // Process multiple payments
    for i in 1..=3 {
        let order = PaymentOrder {
            merchant_address: merchant.clone(),
            amount: 2000 + (i as i64 * 100),
            token: token.address.clone(),
            nonce: i,
            expiration: env.ledger().timestamp() as u32 + 3600,
            order_id: String::from_str(&env, &format!("ORDER{:03}", i)),
            fee_amount: 50,
        };

        let signature = soroban_sdk::BytesN::from_array(&env, &[0u8; 64]);
        let public_key = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);

        client.process_payment_with_signature(&payer, &order, &signature, &public_key);
    }

    // Query payer payment history
    let history = client.get_payer_payment_history(&payer, &10, &0);
    assert_eq!(history.len(), 3);

    // Verify amounts
    assert_eq!(history.get(0).unwrap().amount, 2100);
    assert_eq!(history.get(1).unwrap().amount, 2200);
    assert_eq!(history.get(2).unwrap().amount, 2300);
}

#[test]
fn test_payment_query_with_filters() {
    let (env, client, _admin, merchant, payer, _fee_collector, token) = setup_test_environment();

    // Process payments with different amounts
    for i in 1..=5 {
        let order = PaymentOrder {
            merchant_address: merchant.clone(),
            amount: i as i64 * 1000,
            token: token.address.clone(),
            nonce: i,
            expiration: env.ledger().timestamp() as u32 + 3600,
            order_id: String::from_str(&env, &format!("ORDER{:03}", i)),
            fee_amount: 50,
        };

        let signature = soroban_sdk::BytesN::from_array(&env, &[0u8; 64]);
        let public_key = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);

        client.process_payment_with_signature(&payer, &order, &signature, &public_key);
    }

    // Query with amount filter
    let params = PaymentQueryParams {
        start_time: None,
        end_time: None,
        min_amount: Some(2000),
        max_amount: Some(4000),
        token: None,
        limit: 10,
        offset: 0,
    };

    let filtered = client.query_payments(&params);
    assert_eq!(filtered.len(), 3); // Payments with amounts 2000, 3000, 4000
}

#[test]
fn test_merchant_payment_stats() {
    let (env, client, _admin, merchant, payer, _fee_collector, token) = setup_test_environment();

    // Process multiple payments
    let mut total_amount = 0i128;
    for i in 1..=5 {
        let amount = 1000 + (i * 100);
        total_amount += amount as i128;
        
        let order = PaymentOrder {
            merchant_address: merchant.clone(),
            amount: amount as i64,
            token: token.address.clone(),
            nonce: i as u32,
            expiration: env.ledger().timestamp() as u32 + 3600,
            order_id: String::from_str(&env, &format!("ORDER{:03}", i)),
            fee_amount: 50,
        };

        let signature = soroban_sdk::BytesN::from_array(&env, &[0u8; 64]);
        let public_key = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);

        client.process_payment_with_signature(&payer, &order, &signature, &public_key);
    }

    // Get merchant stats
    let stats = client.get_merchant_payment_stats(&merchant);
    assert_eq!(stats.payment_count, 5);
    assert_eq!(stats.total_received, total_amount);
    assert_eq!(stats.merchant_address, merchant);
}

#[test]
fn test_payer_payment_stats() {
    let (env, client, _admin, merchant, payer, _fee_collector, token) = setup_test_environment();

    // Process multiple payments
    let mut total_spent = 0i128;
    for i in 1..=3 {
        let amount = 2000 + (i * 100);
        total_spent += amount as i128;
        
        let order = PaymentOrder {
            merchant_address: merchant.clone(),
            amount: amount as i64,
            token: token.address.clone(),
            nonce: i as u32,
            expiration: env.ledger().timestamp() as u32 + 3600,
            order_id: String::from_str(&env, &format!("ORDER{:03}", i)),
            fee_amount: 50,
        };

        let signature = soroban_sdk::BytesN::from_array(&env, &[0u8; 64]);
        let public_key = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);

        client.process_payment_with_signature(&payer, &order, &signature, &public_key);
    }

    // Get payer stats
    let stats = client.get_payer_payment_stats(&payer);
    assert_eq!(stats.payment_count, 3);
    assert_eq!(stats.total_spent, total_spent);
    assert_eq!(stats.payer_address, payer);
}

#[test]
fn test_global_payment_stats() {
    let (env, client, _admin, merchant, payer, _fee_collector, token) = setup_test_environment();

    // Process multiple payments
    for i in 1..=5 {
        let order = PaymentOrder {
            merchant_address: merchant.clone(),
            amount: 1000 + (i as i64 * 100),
            token: token.address.clone(),
            nonce: i,
            expiration: env.ledger().timestamp() as u32 + 3600,
            order_id: String::from_str(&env, &format!("ORDER{:03}", i)),
            fee_amount: 50,
        };

        let signature = soroban_sdk::BytesN::from_array(&env, &[0u8; 64]);
        let public_key = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);

        client.process_payment_with_signature(&payer, &order, &signature, &public_key);
    }

    // Get global stats
    let stats = client.get_global_payment_stats();
    assert_eq!(stats.total_payments, 5);
    assert!(stats.total_volume > 0);
    assert!(stats.average_payment > 0);
}

#[test]
fn test_payments_by_time_range() {
    let (env, client, _admin, merchant, payer, _fee_collector, token) = setup_test_environment();

    let start_time = env.ledger().timestamp();

    // Process payments at different times
    for i in 1..=3 {
        // Advance ledger time
        env.ledger().set(LedgerInfo {
            timestamp: start_time + (i * 86400), // Each day
            protocol_version: 20,
            sequence_number: i * 10,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: 1,
            min_persistent_entry_ttl: 1,
            max_entry_ttl: 3110400,
        });

        let order = PaymentOrder {
            merchant_address: merchant.clone(),
            amount: 1000 + (i as i64 * 100),
            token: token.address.clone(),
            nonce: i,
            expiration: env.ledger().timestamp() as u32 + 3600,
            order_id: String::from_str(&env, &format!("ORDER{:03}", i)),
            fee_amount: 50,
        };

        let signature = soroban_sdk::BytesN::from_array(&env, &[0u8; 64]);
        let public_key = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);

        client.process_payment_with_signature(&payer, &order, &signature, &public_key);
    }

    // Query by time range
    let end_time = start_time + (4 * 86400);
    let buckets = client.get_payments_by_time_range(&start_time, &end_time);
    
    assert!(buckets.len() > 0);
}

#[test]
fn test_payments_by_token() {
    let (env, client, _admin, merchant, payer, _fee_collector, token) = setup_test_environment();

    // Create another token
    let token_admin2 = Address::generate(&env);
    let token2 = create_token_contract(&env, &token_admin2);
    token2.mint(&payer, &1_000_000);
    client.add_supported_token(&merchant, &token2.address);

    // Process payments with first token
    for i in 1..=3 {
        let order = PaymentOrder {
            merchant_address: merchant.clone(),
            amount: 1000,
            token: token.address.clone(),
            nonce: i,
            expiration: env.ledger().timestamp() as u32 + 3600,
            order_id: String::from_str(&env, &format!("ORDER1_{:03}", i)),
            fee_amount: 50,
        };

        let signature = soroban_sdk::BytesN::from_array(&env, &[0u8; 64]);
        let public_key = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);

        client.process_payment_with_signature(&payer, &order, &signature, &public_key);
    }

    // Process payments with second token
    for i in 4..=5 {
        let order = PaymentOrder {
            merchant_address: merchant.clone(),
            amount: 2000,
            token: token2.address.clone(),
            nonce: i,
            expiration: env.ledger().timestamp() as u32 + 3600,
            order_id: String::from_str(&env, &format!("ORDER2_{:03}", i)),
            fee_amount: 50,
        };

        let signature = soroban_sdk::BytesN::from_array(&env, &[0u8; 64]);
        let public_key = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);

        client.process_payment_with_signature(&payer, &order, &signature, &public_key);
    }

    // Query payments by first token
    let token1_payments = client.get_payments_by_token(&token.address, &10, &0);
    assert_eq!(token1_payments.len(), 3);

    // Query payments by second token
    let token2_payments = client.get_payments_by_token(&token2.address, &10, &0);
    assert_eq!(token2_payments.len(), 2);
}

#[test]
fn test_archive_old_payments() {
    let (env, client, admin, merchant, payer, _fee_collector, token) = setup_test_environment();

    let current_time = env.ledger().timestamp();

    // Process a payment
    let order = PaymentOrder {
        merchant_address: merchant.clone(),
        amount: 1000,
        token: token.address.clone(),
        nonce: 1,
        expiration: current_time as u32 + 3600,
        order_id: String::from_str(&env, "ORDER001"),
        fee_amount: 50,
    };

    let signature = soroban_sdk::BytesN::from_array(&env, &[0u8; 64]);
    let public_key = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);

    client.process_payment_with_signature(&payer, &order, &signature, &public_key);

    // Archive payments older than current time + 1 day
    let cutoff_time = current_time + 86400;
    
    // Set ledger time to future
    env.ledger().set(LedgerInfo {
        timestamp: cutoff_time + 100,
        protocol_version: 20,
        sequence_number: 100,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 1,
        min_persistent_entry_ttl: 1,
        max_entry_ttl: 3110400,
    });

    // Archive
    client.archive_old_payments(&admin, &cutoff_time);
    
    // Payment should still be accessible
    let payment = client.get_payment_by_order_id(&String::from_str(&env, "ORDER001"));
    assert_eq!(payment.amount, 1000);
}

#[test]
#[should_panic(expected = "InvalidAmount")]
fn test_invalid_pagination_limit() {
    let (_env, client, _admin, merchant, _payer, _fee_collector, _token) = setup_test_environment();

    // Should fail with limit > 100
    client.get_merchant_payment_history(&merchant, &101, &0);
}

#[test]
#[should_panic(expected = "InvalidAmount")]
fn test_invalid_pagination_zero_limit() {
    let (_env, client, _admin, merchant, _payer, _fee_collector, _token) = setup_test_environment();

    // Should fail with limit = 0
    client.get_merchant_payment_history(&merchant, &0, &0);
}