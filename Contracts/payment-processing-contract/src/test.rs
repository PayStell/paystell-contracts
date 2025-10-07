#![cfg(test)]

use soroban_sdk::{
    testutils::Address as _,
    Address, Env, String, BytesN, Vec, Symbol,
    token,
};
use crate::{
    PaymentProcessingContract, PaymentProcessingContractClient, 
    types::{
        PaymentOrder, BatchMerchantRegistration, BatchTokenAddition, 
        BatchPayment
    }
};

fn create_token_contract<'a>(
    e: &'a Env,
    admin: &Address,
) -> (Address, token::Client<'a>, token::StellarAssetClient<'a>) {
    let token_id = e.register_stellar_asset_contract_v2(admin.clone());
    let token = token_id.address();
    let token_client = token::Client::new(e, &token);
    let token_admin_client = token::StellarAssetClient::new(e, &token);
    (token, token_client, token_admin_client)
}

fn create_payment_order(
    env: &Env,
    merchant: &Address,
    amount: i64,
    token: &Address,
    expiration: u32,
) -> PaymentOrder {
    PaymentOrder {
        merchant_address: merchant.clone(),
        amount,
        token: token.clone(),
        nonce: env.ledger().timestamp() as u32,
        expiration,
        order_id: String::from_str(&env, "TEST_ORDER_1"),
        fee_amount: 0, // Initial fee amount, will be calculated during processing
    }
}

#[test]
fn test_fee_management() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup admin and fee collector
    let admin = Address::generate(&env);
    let fee_collector = Address::generate(&env);
    let fee_token = Address::generate(&env);

    // Set admin
    env.mock_all_auths();
    client.set_admin(&admin);

    // Set fee (5%)
    env.mock_all_auths();
    client.set_fee(&5, &fee_collector, &fee_token);

    // Get fee info and verify
    let (rate, collector, token) = client.get_fee_info();
    assert_eq!(rate, 5);
    assert_eq!(collector, fee_collector);
    assert_eq!(token, fee_token);
}

#[test]
#[should_panic] // AdminNotSet
fn test_set_fee_no_admin() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let fee_collector = Address::generate(&env);
    let fee_token = Address::generate(&env);

    // Try to set fee without setting admin first
    env.mock_all_auths();
    client.set_fee(&5, &fee_collector, &fee_token);
}

#[test]
#[should_panic]
fn test_invalid_fee_rate() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let fee_collector = Address::generate(&env);
    let fee_token = Address::generate(&env);

    // Set admin
    env.mock_all_auths();
    client.set_admin(&admin);

    // Try to set invalid fee rate (11% > 10% max)
    env.mock_all_auths();
    client.set_fee(&11, &fee_collector, &fee_token);
}

#[test]
fn test_payment_with_fees() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup merchant
    let merchant = Address::generate(&env);
    let merchant_public = BytesN::from_array(&env, &[1u8; 32]);

    // Setup admin and fee collector
    let admin = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    // Setup token
    let token_admin = Address::generate(&env);
    let (token, token_client, token_admin_client) = create_token_contract(&env, &token_admin);

    // Setup payer with balance
    let payer = Address::generate(&env);
    let payment_amount = 1000_i128;

    env.mock_all_auths();
    token_admin_client.mint(&payer, &payment_amount);

    // Register merchant and add token support
    env.mock_all_auths();
    client.register_merchant(&merchant);
    env.mock_all_auths();
    client.add_supported_token(&merchant, &token);

    // Set admin and fee (5%)
    env.mock_all_auths();
    client.set_admin(&admin);
    env.mock_all_auths();
    client.set_fee(&5, &fee_collector, &token);

    // Create payment order
    let order = PaymentOrder {
        merchant_address: merchant.clone(),
        amount: payment_amount as i64,
        token: token.clone(),
        nonce: 12345u32,
        expiration: (env.ledger().timestamp() + 1000) as u32,
        fee_amount: 0, // Initial fee amount, will be calculated during processing
        order_id: String::from_str(&env, "TEST_ORDER_1"),
    };

    // Process payment
    let signature = BytesN::from_array(&env, &[2u8; 64]);
    env.mock_all_auths();
    client.process_payment_with_signature(&payer, &order, &signature, &merchant_public);

    // Verify balances
    let expected_fee = payment_amount * 5 / 100;
    let expected_merchant_amount = payment_amount - expected_fee;

    assert_eq!(token_client.balance(&merchant), expected_merchant_amount);
    assert_eq!(token_client.balance(&fee_collector), expected_fee);
    assert_eq!(token_client.balance(&payer), 0);
}

#[test]
fn test_register_merchant() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    env.mock_all_auths();

    client.register_merchant(&merchant);

    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    let auth = auths.first().unwrap();
    assert_eq!(auth.0, merchant);
}

#[test]
fn test_add_supported_token() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    let token = Address::generate(&env);

    // Register merchant first
    env.mock_all_auths();
    client.register_merchant(&merchant);

    // Add supported token
    env.mock_all_auths();
    client.add_supported_token(&merchant, &token);
}

#[test]
#[should_panic] // MerchantNotFound
fn test_add_token_to_nonexistent_merchant() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    let token = Address::generate(&env);

    // Try to add token without registering merchant first
    env.mock_all_auths();
    client.add_supported_token(&merchant, &token);
}

#[test]
fn test_successful_payment_with_signature() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup merchant with keys
    let merchant = Address::generate(&env);
    let merchant_public = BytesN::from_array(&env, &[1u8; 32]);

    // Setup admin and fee collector
    let admin = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    // Setup token
    let token_admin = Address::generate(&env);
    let (token, token_client, token_admin_client) = create_token_contract(&env, &token_admin);
    let payer = Address::generate(&env);
    let amount = 100_i128;

    // Register merchant and add token support
    env.mock_all_auths();
    client.register_merchant(&merchant);
    env.mock_all_auths();
    client.add_supported_token(&merchant, &token);

    // Set up fee management
    env.mock_all_auths();
    client.set_admin(&admin);
    env.mock_all_auths();
    client.set_fee(&5, &fee_collector, &token); // 5% fee

    // Create payment order
    let order = PaymentOrder {
        merchant_address: merchant.clone(),
        amount: amount as i64,
        token: token.clone(),
        nonce: 12345u32,
        expiration: (env.ledger().timestamp() + 1000) as u32,
        order_id: String::from_str(&env, "TEST_ORDER_1"),
        fee_amount: 0, // Will be calculated during processing
    };

    // Setup token balances
    env.mock_all_auths();
    token_admin_client.mint(&payer, &amount);

    // Use any 64-byte array for signature
    let signature = BytesN::from_array(&env, &[2u8; 64]);

    // Mock all auths for the payment including fee collector
    env.mock_all_auths();

    // Process payment
    client.process_payment_with_signature(&payer, &order, &signature, &merchant_public);

    // Verify balances
    let expected_fee = amount * 5 / 100;
    let expected_merchant_amount = amount - expected_fee;

    assert_eq!(token_client.balance(&merchant), expected_merchant_amount);
    assert_eq!(token_client.balance(&fee_collector), expected_fee);
    assert_eq!(token_client.balance(&payer), 0);
}

#[test]
#[should_panic]
fn test_expired_order() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup merchant with keys
    let merchant = Address::generate(&env);
    let merchant_public = BytesN::from_array(
        &env,
        &[
            0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64,
            0x07, 0x3a, 0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68,
            0xf7, 0x07, 0x51, 0x1a,
        ],
    );

    // Setup token
    let admin = Address::generate(&env);
    let (token, _, _) = create_token_contract(&env, &admin);

    // Register merchant and add token
    env.mock_all_auths();
    client.register_merchant(&merchant);
    client.add_supported_token(&merchant, &token);

    // Create expired order
    let current_time = env.ledger().timestamp();
    let expired_time = (current_time - 1000) as u32; // Set expiration in the past
    let order = create_payment_order(&env, &merchant, 100, &token, expired_time);

    // Create test signature
    let signature = BytesN::from_array(&env, &[3u8; 64]); // Test signature

    // Should fail due to expired order
    client.process_payment_with_signature(
        &Address::generate(&env),
        &order,
        &signature,
        &merchant_public,
    );
}

#[test]
#[should_panic]
fn test_duplicate_nonce() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup merchant with keys
    let merchant = Address::generate(&env);
    let merchant_public = BytesN::from_array(
        &env,
        &[
            0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64,
            0x07, 0x3a, 0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68,
            0xf7, 0x07, 0x51, 0x1a,
        ],
    );

    // Setup token
    let admin = Address::generate(&env);
    let (token, _, token_admin) = create_token_contract(&env, &admin);

    // Setup payer
    let payer = Address::generate(&env);
    let amount = 100_i128;

    // Register merchant and add token
    env.mock_all_auths();
    client.register_merchant(&merchant);
    client.add_supported_token(&merchant, &token);

    // Create order
    let expiration = (env.ledger().timestamp() + 1000) as u32;
    let order = create_payment_order(&env, &merchant, amount as i64, &token, expiration);
    // Create test signature
    let signature = BytesN::from_array(&env, &[3u8; 64]); // Test signature

    // Setup token balances
    token_admin.mint(&payer, &(amount * 2));

    // First payment should succeed
    env.mock_all_auths();
    client.process_payment_with_signature(&payer, &order.clone(), &signature, &merchant_public);

    // Second payment with same nonce should fail
    client.process_payment_with_signature(&payer, &order, &signature, &merchant_public);
}

#[test]
#[should_panic]
fn test_unsupported_token() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup merchant with keys
    let merchant = Address::generate(&env);
    let merchant_public = BytesN::from_array(
        &env,
        &[
            0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64,
            0x07, 0x3a, 0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68,
            0xf7, 0x07, 0x51, 0x1a,
        ],
    );

    // Setup token (but don't add it as supported)
    let admin = Address::generate(&env);
    let (token, _, _) = create_token_contract(&env, &admin);

    // Register merchant (but don't add token support)
    env.mock_all_auths();
    client.register_merchant(&merchant);

    // Create order with unsupported token
    let expiration = (env.ledger().timestamp() + 1000) as u32;
    let order = create_payment_order(&env, &merchant, 100, &token, expiration);

    // Create test signature
    let signature = BytesN::from_array(&env, &[3u8; 64]); // Test signature

    // Should fail due to unsupported token
    client.process_payment_with_signature(
        &Address::generate(&env),
        &order,
        &signature,
        &merchant_public,
    );
}

// Performance and Gas Optimization Tests

#[test]
fn test_batch_register_merchants() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Create multiple merchants
    let merchants = Vec::from_array(&env, [
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    ]);

    let batch = BatchMerchantRegistration {
        merchants: merchants.clone(),
    };

    env.mock_all_auths();
    client.batch_register_merchants(&batch);

    // Verify all merchants were registered
    for merchant in merchants.iter() {
        let merchant_info = client.get_merchant_info(&merchant);
        assert!(merchant_info.is_active());
    }
}

#[test]
fn test_batch_add_tokens() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    let tokens = Vec::from_array(&env, [
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    ]);

    // Register merchant first
    env.mock_all_auths();
    client.register_merchant(&merchant);

    let batch = BatchTokenAddition {
        merchant: merchant.clone(),
        tokens: tokens.clone(),
    };

    env.mock_all_auths();
    client.batch_add_tokens(&batch);

    // Verify all tokens were added
    for token in tokens.iter() {
        assert!(client.is_token_supported(&merchant, &token));
    }

    // Verify token count
    assert_eq!(client.get_merchant_token_count(&merchant), 3);
}

#[test]
fn test_batch_process_payments() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup merchant and token
    let merchant = Address::generate(&env);
    let admin = Address::generate(&env);
    let (token, token_client, token_admin) = create_token_contract(&env, &admin);
    let payer = Address::generate(&env);

    // Register merchant and add token
    env.mock_all_auths();
    client.register_merchant(&merchant);
    client.add_supported_token(&merchant, &token);

    // Create multiple payment orders
    let orders = Vec::from_array(&env, [
        PaymentOrder {
            merchant_address: merchant.clone(),
            amount: 100,
            token: token.clone(),
            nonce: 1,
            expiration: (env.ledger().timestamp() + 1000) as u32,
            order_id: String::from_str(&env, "ORDER_1"),
            fee_amount: 0,
        },
        PaymentOrder {
            merchant_address: merchant.clone(),
            amount: 200,
            token: token.clone(),
            nonce: 2,
            expiration: (env.ledger().timestamp() + 1000) as u32,
            order_id: String::from_str(&env, "ORDER_2"),
            fee_amount: 0,
        },
        PaymentOrder {
            merchant_address: merchant.clone(),
            amount: 300,
            token: token.clone(),
            nonce: 3,
            expiration: (env.ledger().timestamp() + 1000) as u32,
            order_id: String::from_str(&env, "ORDER_3"),
            fee_amount: 0,
        },
    ]);

    let signatures = Vec::from_array(&env, [
        BytesN::from_array(&env, &[4u8; 64]),
        BytesN::from_array(&env, &[5u8; 64]),
        BytesN::from_array(&env, &[6u8; 64]),
    ]);
    let merchant_public = BytesN::from_array(&env, &[7u8; 32]);

    let batch = BatchPayment {
        payer: payer.clone(),
        orders: orders.clone(),
        signatures,
        merchant_public_key: merchant_public,
    };

    // Setup token balances
    token_admin.mint(&payer, &600);

    env.mock_all_auths();
    client.batch_process_payments(&batch);

    // Verify balances
    assert_eq!(token_client.balance(&merchant), 600);
    assert_eq!(token_client.balance(&payer), 0);

    // Verify nonces were marked as used
    for order in orders.iter() {
        let tracker = client.get_nonce_tracker(&merchant);
        assert!(tracker.is_some());
        assert!(tracker.unwrap().is_nonce_used(order.nonce));
    }
}

#[test]
fn test_gas_estimation() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    let token = Address::generate(&env);
    let order = PaymentOrder {
        merchant_address: merchant.clone(),
        amount: 100,
        token: token.clone(),
        nonce: 1,
        expiration: (env.ledger().timestamp() + 1000) as u32,
        order_id: String::from_str(&env, "TEST_ORDER"),
        fee_amount: 0,
    };

    // Test payment gas estimation
    let estimate = client.estimate_gas_for_payment(&order);
    assert!(estimate.total_estimated > 0);
    assert!(estimate.base_gas > 0);
    assert!(estimate.per_item_gas > 0);

    // Test batch operation gas estimation
    let batch_estimate = client.estimate_gas_for_batch_operation(
        &Symbol::new(&env, "proc_pay"),
        &3
    );
    assert!(batch_estimate.total_estimated > estimate.total_estimated);
}

#[test]
fn test_view_functions() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    let token = Address::generate(&env);

    // Register merchant and add token
    env.mock_all_auths();
    client.register_merchant(&merchant);
    client.add_supported_token(&merchant, &token);

    // Test view functions (should not require auth)
    let merchant_info = client.get_merchant_info(&merchant);
    assert!(merchant_info.is_active());
    assert_eq!(merchant_info.token_count, 1);

    let token_count = client.get_merchant_token_count(&merchant);
    assert_eq!(token_count, 1);

    let is_supported = client.is_token_supported(&merchant, &token);
    assert!(is_supported);

    let tracker = client.get_nonce_tracker(&merchant);
    assert!(tracker.is_none()); // No nonces used yet
}

#[test]
fn test_merchant_activation_deactivation() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);

    // Register merchant
    env.mock_all_auths();
    client.register_merchant(&merchant);

    // Verify merchant is active
    let merchant_info = client.get_merchant_info(&merchant);
    assert!(merchant_info.is_active());

    // Deactivate merchant
    env.mock_all_auths();
    client.deactivate_merchant(&merchant);

    // Verify merchant is inactive
    let merchant_info = client.get_merchant_info(&merchant);
    assert!(!merchant_info.is_active());

    // Activate merchant
    env.mock_all_auths();
    client.activate_merchant(&merchant);

    // Verify merchant is active again
    let merchant_info = client.get_merchant_info(&merchant);
    assert!(merchant_info.is_active());
}

#[test]
fn test_token_removal() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    let token1 = Address::generate(&env);
    let token2 = Address::generate(&env);

    // Register merchant and add tokens
    env.mock_all_auths();
    client.register_merchant(&merchant);
    client.add_supported_token(&merchant, &token1);
    client.add_supported_token(&merchant, &token2);

    // Verify both tokens are supported
    assert_eq!(client.get_merchant_token_count(&merchant), 2);
    assert!(client.is_token_supported(&merchant, &token1));
    assert!(client.is_token_supported(&merchant, &token2));

    // Remove one token
    env.mock_all_auths();
    client.remove_supported_token(&merchant, &token1);

    // Verify only one token remains
    assert_eq!(client.get_merchant_token_count(&merchant), 1);
    assert!(!client.is_token_supported(&merchant, &token1));
    assert!(client.is_token_supported(&merchant, &token2));
}

#[test]
fn test_nonce_bitmap_optimization() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    let admin = Address::generate(&env);
    let (token, token_client, token_admin) = create_token_contract(&env, &admin);
    let payer = Address::generate(&env);

    // Register merchant and add token
    env.mock_all_auths();
    client.register_merchant(&merchant);
    client.add_supported_token(&merchant, &token);

    // Set up fee management
    env.mock_all_auths();
    client.set_admin(&admin);
    env.mock_all_auths();
    client.set_fee(&0, &admin, &token); // 0% fee for this test

    // Setup token balance
    token_admin.mint(&payer, &1000);

    let signature = BytesN::from_array(&env, &[6u8; 64]);
    let merchant_public = BytesN::from_array(&env, &[7u8; 32]);

    // Process multiple payments with different nonces
    for i in 1..=10 {
        let order = PaymentOrder {
            merchant_address: merchant.clone(),
            amount: 100,
            token: token.clone(),
            nonce: i,
            expiration: (env.ledger().timestamp() + 1000) as u32,
            order_id: String::from_str(&env, "ORDER_TEST"),
            fee_amount: 0,
        };

        env.mock_all_auths();
        client.process_payment_with_signature(
            &payer,
            &order,
            &signature,
            &merchant_public
        );
    }

    // Verify nonce tracker shows all nonces as used
    let tracker = client.get_nonce_tracker(&merchant);
    assert!(tracker.is_some());
    let tracker = tracker.unwrap();
    
    for i in 1..=10 {
        assert!(tracker.is_nonce_used(i));
    }
    
    // Verify nonce 11 is not used
    assert!(!tracker.is_nonce_used(11));
}
