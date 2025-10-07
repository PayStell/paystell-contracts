#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger}, 
    Address, Env, String, BytesN, Vec, Symbol,
    token,
};
use crate::{
    PaymentProcessingContract, PaymentProcessingContractClient,
    types::{PaymentOrder, PaymentStatus, BatchMerchantRegistration, BatchTokenAddition, BatchPayment, GasEstimate, NonceTracker, MerchantCategory, ProfileUpdateData, RefundRequest, RefundStatus},
    error::PaymentError,
    storage::Storage,
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

fn register_test_merchant(
    client: &PaymentProcessingContractClient,
    env: &Env,
    merchant: &Address,
) {
    client.register_merchant(
        merchant,
        &String::from_str(&env, "Test Merchant"),
        &String::from_str(&env, "A test merchant for unit tests"),
        &String::from_str(&env, "test@merchant.com"),
        &MerchantCategory::Retail,
    );
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
#[should_panic] // AdminNotFound
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
    register_test_merchant(&client, &env, &merchant);
    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    let auth = auths.first().unwrap();
    assert_eq!(auth.0, merchant);
    
    // Verify profile was saved correctly
    let profile = client.get_merchant_profile(&merchant);
    assert_eq!(profile.wallet_address, merchant);
    assert_eq!(profile.active, true);
    assert_eq!(profile.name, String::from_str(&env, "Test Merchant"));
    assert_eq!(profile.category, MerchantCategory::Retail);
    // Verify default transaction limit is set
    assert!(profile.max_transaction_limit > 0);
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
    register_test_merchant(&client, &env, &merchant);
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
    register_test_merchant(&client, &env, &merchant);
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
fn test_successful_refund_flow() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup accounts and token
    let merchant = Address::generate(&env);
    let payer = Address::generate(&env);
    let admin = Address::generate(&env);
    let (token, token_client, token_admin) = create_token_contract(&env, &admin);

    // Register merchant and add token
    env.mock_all_auths();
    client.register_merchant(&merchant);
    env.mock_all_auths();
    client.add_supported_token(&merchant, &token);

    // Mint tokens to payer and pay
    let amount = 200_i128;
    token_admin.mint(&payer, &amount);
    let order = PaymentOrder {
        merchant_address: merchant.clone(),
        amount,
        token: token.clone(),
        nonce: 98765u64,
        expiration: env.ledger().timestamp() + 1000,
        order_id: String::from_str(&env, "ORDER_1"),
    };
    let signature = BytesN::from_array(&env, &[7u8; 64]);
    let merchant_public = BytesN::from_array(&env, &[5u8; 32]);

    env.mock_all_auths();
    client.process_payment_with_signature(&payer, &order, &signature, &merchant_public);

    // Verify payment balances
    assert_eq!(token_client.balance(&merchant), amount);
    assert_eq!(token_client.balance(&payer), 0);

    // Initiate refund of 50
    let refund_amount = 50_i128;
    let refund_id = String::from_str(&env, "REFUND_1");
    env.mock_all_auths();
    client.initiate_refund(&merchant, &refund_id, &order.order_id, &refund_amount, &String::from_str(&env, "Customer request"));

    // Approve refund by merchant
    env.mock_all_auths();
    client.approve_refund(&merchant, &refund_id);

    // Execute refund (merchant must authorize)
    env.mock_all_auths();
    client.execute_refund(&refund_id);

    // Check balances after refund
    assert_eq!(token_client.balance(&merchant), amount - refund_amount);
    assert_eq!(token_client.balance(&payer), refund_amount);

    // Check status
    let status = client.get_refund_status(&refund_id);
    // Completed variant index may be compared via pattern; here we just ensure call succeeds
    let _ = status; // presence indicates no panic
}

#[test]
#[should_panic]
fn test_over_refund_prevented() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup
    let merchant = Address::generate(&env);
    let payer = Address::generate(&env);
    let admin = Address::generate(&env);
    let (token, _token_client, token_admin) = create_token_contract(&env, &admin);

    env.mock_all_auths();
    client.register_merchant(&merchant);
    env.mock_all_auths();
    client.add_supported_token(&merchant, &token);

    let amount = 100_i128;
    token_admin.mint(&payer, &amount);
    let order = PaymentOrder {
        merchant_address: merchant.clone(),
        amount,
        token: token.clone(),
        nonce: 22222u64,
        expiration: env.ledger().timestamp() + 1000,
        order_id: String::from_str(&env, "ORDER_2"),
    };
    let signature = BytesN::from_array(&env, &[9u8; 64]);
    let merchant_public = BytesN::from_array(&env, &[6u8; 32]);

    env.mock_all_auths();
    client.process_payment_with_signature(&payer, &order, &signature, &merchant_public);

    // Attempt to initiate refund more than amount
    let refund_amount = 150_i128;
    let refund_id = String::from_str(&env, "REFUND_2");
    env.mock_all_auths();
    client.initiate_refund(&payer, &refund_id, &order.order_id, &refund_amount, &String::from_str(&env, "Over refund"));
}

#[test]
fn test_admin_can_approve_and_execute_refund() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    let payer = Address::generate(&env);
    let admin = Address::generate(&env);
    let (token, token_client, token_admin) = create_token_contract(&env, &admin);

    // Setup merchant and token
    env.mock_all_auths();
    client.register_merchant(&merchant);
    env.mock_all_auths();
    client.add_supported_token(&merchant, &token);

    // Set admin
    env.mock_all_auths();
    client.set_admin(&admin);

    // Mint tokens and make payment
    let amount = 120_i128;
    token_admin.mint(&payer, &amount);
    let order = PaymentOrder {
        merchant_address: merchant.clone(),
        amount,
        token: token.clone(),
        nonce: 33333u64,
        expiration: env.ledger().timestamp() + 1000,
        order_id: String::from_str(&env, "ORDER_3"),
    };
    let signature = BytesN::from_array(&env, &[10u8; 64]);
    let merchant_public = BytesN::from_array(&env, &[7u8; 32]);

    env.mock_all_auths();
    client.process_payment_with_signature(&payer, &order, &signature, &merchant_public);

    // Payer initiates refund, admin approves
    let refund_id = String::from_str(&env, "REFUND_3");
    let refund_amount = 20_i128;
    env.mock_all_auths();
    client.initiate_refund(&payer, &refund_id, &order.order_id, &refund_amount, &String::from_str(&env, "Dispute"));

    // Approve by admin
    env.mock_all_auths();
    client.approve_refund(&admin, &refund_id);

    // Execute (requires merchant auth for token transfer)
    env.mock_all_auths();
    client.execute_refund(&refund_id);

    // Balances
    assert_eq!(token_client.balance(&merchant), amount - refund_amount);
    assert_eq!(token_client.balance(&payer), refund_amount);
}

#[test]
#[should_panic]
fn test_unauthorized_cannot_approve_refund() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    let payer = Address::generate(&env);
    let admin = Address::generate(&env);
    let outsider = Address::generate(&env);
    let (token, _token_client, token_admin) = create_token_contract(&env, &admin);

    env.mock_all_auths();
    client.register_merchant(&merchant);
    env.mock_all_auths();
    client.add_supported_token(&merchant, &token);
    env.mock_all_auths();
    client.set_admin(&admin);

    let amount = 80_i128;
    token_admin.mint(&payer, &amount);
    let order = PaymentOrder {
        merchant_address: merchant.clone(),
        amount,
        token: token.clone(),
        nonce: 44444u64,
        expiration: env.ledger().timestamp() + 1000,
        order_id: String::from_str(&env, "ORDER_4"),
    };
    let signature = BytesN::from_array(&env, &[11u8; 64]);
    let merchant_public = BytesN::from_array(&env, &[8u8; 32]);

    env.mock_all_auths();
    client.process_payment_with_signature(&payer, &order, &signature, &merchant_public);

    let refund_id = String::from_str(&env, "REFUND_4");
    env.mock_all_auths();
    client.initiate_refund(&payer, &refund_id, &order.order_id, &40_i128, &String::from_str(&env, "Test"));

    // Outsider tries to approve -> should panic
    env.mock_all_auths();
    client.approve_refund(&outsider, &refund_id);
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
    register_test_merchant(&client, &env, &merchant);
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
    register_test_merchant(&client, &env, &merchant);
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
    register_test_merchant(&client, &env, &merchant);
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
        assert!(merchant_info.active);
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
    register_test_merchant(&client, &env, &merchant);

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
    let (token, _token_client, token_admin) = create_token_contract(&env, &admin);
    let payer = Address::generate(&env);

    // Register merchant and add token
    env.mock_all_auths();
    register_test_merchant(&client, &env, &merchant);
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
    register_test_merchant(&client, &env, &merchant);
    client.add_supported_token(&merchant, &token);

    // Test view functions (should not require auth)
    let merchant_info = client.get_merchant_info(&merchant);
    assert!(merchant_info.active);
    assert_eq!(merchant_info.supported_tokens.len(), 1);

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
    register_test_merchant(&client, &env, &merchant);

    // Verify merchant is active
    let merchant_info = client.get_merchant_info(&merchant);
    assert!(merchant_info.active);

    // Deactivate merchant
    env.mock_all_auths();
    client.deactivate_merchant(&merchant);

    // Verify merchant is inactive
    let merchant_info = client.get_merchant_info(&merchant);
    assert!(!merchant_info.active);

    // Activate merchant
    env.mock_all_auths();
    client.activate_merchant(&merchant);

    // Verify merchant is active again
    let merchant_info = client.get_merchant_info(&merchant);
    assert!(merchant_info.active);
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
    register_test_merchant(&client, &env, &merchant);
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
    let (token, _token_client, token_admin) = create_token_contract(&env, &admin);
    let payer = Address::generate(&env);

    // Register merchant and add token
    env.mock_all_auths();
    register_test_merchant(&client, &env, &merchant);
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

// Multi-signature payment tests

#[test]
fn test_initiate_multisig_payment_success() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup
    let admin = Address::generate(&env);
    let (token, _, _) = create_token_contract(&env, &admin);
    let recipient = Address::generate(&env);
    let signer1 = Address::generate(&env);
    let signer2 = Address::generate(&env);
    let signer3 = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());
    signers.push_back(signer2.clone());
    signers.push_back(signer3.clone());

    let amount = 1000_i128;
    let threshold = 2u32;
    let expiry = env.ledger().timestamp() + 3600; // 1 hour from now

    env.mock_all_auths();

    // Initiate payment
    let payment_id = client.initiate_multisig_payment(
        &amount,
        &token,
        &recipient,
        &signers,
        &threshold,
        &expiry,
    );

    // Verify payment was created
    let payment = client.get_multisig_payment(&payment_id);
    assert_eq!(payment.amount, amount);
    assert_eq!(payment.token, token);
    assert_eq!(payment.recipient, recipient);
    assert_eq!(payment.threshold, threshold);
    assert_eq!(payment.status, PaymentStatus::Pending);
    assert_eq!(payment.signers.len(), 3);
}

#[test]
fn test_initiate_multisig_payment_invalid_threshold() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup
    let admin = Address::generate(&env);
    let (token, _, _) = create_token_contract(&env, &admin);
    let recipient = Address::generate(&env);
    let signer1 = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1);

    let amount = 1000_i128;
    let threshold = 5u32; // Invalid: threshold > signers count
    let expiry = env.ledger().timestamp() + 3600;

    env.mock_all_auths();

    // Should fail with InvalidThreshold
    let result = client.try_initiate_multisig_payment(
        &amount,
        &token,
        &recipient,
        &signers,
        &threshold,
        &expiry,
    );

    assert_eq!(result, Err(Ok(PaymentError::InvalidThreshold)));
}

#[test]
fn test_add_signature_success() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup payment
    let admin = Address::generate(&env);
    let (token, _, _) = create_token_contract(&env, &admin);
    let recipient = Address::generate(&env);
    let signer1 = Address::generate(&env);
    let signer2 = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());
    signers.push_back(signer2.clone());

    env.mock_all_auths();

    let payment_id = client.initiate_multisig_payment(
        &1000_i128,
        &token,
        &recipient,
        &signers,
        &2u32,
        &(env.ledger().timestamp() + 3600),
    );

    // Add signature from signer1
    client.add_signature(&payment_id, &signer1);

    // Verify signature was added
    let payment = client.get_multisig_payment(&payment_id);
    assert_eq!(payment.signatures.len(), 1);
    assert_eq!(payment.signatures.get(signer1).unwrap(), true);
}

#[test]
fn test_add_signature_not_a_signer() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup payment
    let admin = Address::generate(&env);
    let (token, _, _) = create_token_contract(&env, &admin);
    let recipient = Address::generate(&env);
    let signer1 = Address::generate(&env);
    let not_signer = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1);

    env.mock_all_auths();

    let payment_id = client.initiate_multisig_payment(
        &1000_i128,
        &token,
        &recipient,
        &signers,
        &1u32,
        &(env.ledger().timestamp() + 3600),
    );

    // Try to add signature from non-signer
    let result = client.try_add_signature(&payment_id, &not_signer);
    assert_eq!(result, Err(Ok(PaymentError::NotASigner)));
}

#[test]
fn test_execute_multisig_payment_success() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup token and balances
    let admin = Address::generate(&env);
    let (token, token_client, token_admin) = create_token_contract(&env, &admin);
    let recipient = Address::generate(&env);
    let signer1 = Address::generate(&env);
    let signer2 = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());
    signers.push_back(signer2.clone());

    let amount = 1000_i128;

    // Mock all auths first
    env.mock_all_auths();

    // Mint tokens to signer1 (executor)
    token_admin.mint(&signer1, &amount);

    // Create payment
    let payment_id = client.initiate_multisig_payment(
        &amount,
        &token,
        &recipient,
        &signers,
        &2u32,
        &(env.ledger().timestamp() + 3600),
    );

    // Add signatures
    client.add_signature(&payment_id, &signer1);
    client.add_signature(&payment_id, &signer2);

    // Execute payment
    client.execute_multisig_payment(&payment_id, &signer1);

    // Verify payment was executed (should be removed from active payments)
    let result = client.try_get_multisig_payment(&payment_id);
    assert!(result.is_err()); // Should be removed after execution

    // Verify token transfer
    assert_eq!(token_client.balance(&recipient), amount);
    assert_eq!(token_client.balance(&signer1), 0);
}

#[test]
fn test_execute_multisig_payment_threshold_not_met() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup
    let admin = Address::generate(&env);
    let (token, _, _) = create_token_contract(&env, &admin);
    let recipient = Address::generate(&env);
    let signer1 = Address::generate(&env);
    let signer2 = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());
    signers.push_back(signer2.clone());

    env.mock_all_auths();

    let payment_id = client.initiate_multisig_payment(
        &1000_i128,
        &token,
        &recipient,
        &signers,
        &2u32,
        &(env.ledger().timestamp() + 3600),
    );

    // Add only one signature (threshold is 2)
    client.add_signature(&payment_id, &signer1);

    // Try to execute with only one signature
    let result = client.try_execute_multisig_payment(&payment_id, &signer1);
    assert_eq!(result, Err(Ok(PaymentError::ThresholdNotMet)));
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #16)")]
fn test_register_merchant_paused() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let pause_admin = Address::generate(&env);
    let merchant = Address::generate(&env);
    env.mock_all_auths();

    client.set_admin(&admin);
    client.set_pause_admin(&admin, &pause_admin);
    client.pause(&pause_admin);

    register_test_merchant(&client, &env, &merchant);
}

#[test]
fn test_contract_is_paused() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let pause_admin = Address::generate(&env);
    env.mock_all_auths();

    client.set_admin(&admin);
    client.set_pause_admin(&admin, &pause_admin);
    client.pause(&pause_admin);

    let is_paused = client.is_paused();

    assert_eq!(is_paused, true);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #16)")]
fn test_add_supported_token_paused() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let pause_admin = Address::generate(&env);
    let merchant = Address::generate(&env);
    let _token = Address::generate(&env);
    env.mock_all_auths();

    client.set_admin(&admin);

    client.set_pause_admin(&admin, &pause_admin);
    // Register merchant first
    register_test_merchant(&client, &env, &merchant);
    
    client.pause(&pause_admin);

    // Add supported token
    client.add_supported_token(&merchant, &token);
}

#[test]
fn test_pause_unpause() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let pause_admin = Address::generate(&env);
    let merchant = Address::generate(&env);
    let _token = Address::generate(&env);
    env.mock_all_auths();

    client.set_admin(&admin);

    client.set_pause_admin(&admin, &pause_admin);
    // Register merchant first
    register_test_merchant(&client, &env, &merchant);
    
    client.pause(&pause_admin);

    let is_paused = client.is_paused();
    assert_eq!(is_paused, true);

    client.unpause(&pause_admin);

    let is_paused = client.is_paused();
    assert_eq!(is_paused, false);

    // Add supported token
    client.add_supported_token(&merchant, &token);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #17)")]
fn test_double_pause() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let pause_admin = Address::generate(&env);
    let merchant = Address::generate(&env);
    let _token = Address::generate(&env);
    env.mock_all_auths();

    client.set_admin(&admin);

    client.set_pause_admin(&admin, &pause_admin);
    // Register merchant first
    register_test_merchant(&client, &env, &merchant);
    
    client.pause(&pause_admin);

    let is_paused = client.is_paused();
    assert_eq!(is_paused, true);
    
    client.pause(&pause_admin);
}


#[test]
#[should_panic(expected = "HostError: Error(Contract, #15)")]
fn test_pause_without_set_pause_admin() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let merchant = Address::generate(&env);
    env.mock_all_auths();
    register_test_merchant(&client, &env, &merchant);
    
    client.pause(&admin);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #1)")]
fn test_unauthorized() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let pause_admin = Address::generate(&env);
    let unauthorized = Address::generate(&env);
    let merchant = Address::generate(&env);
    let token = Address::generate(&env);
    env.mock_all_auths();

    client.set_admin(&admin);

    client.set_pause_admin(&admin, &pause_admin);
    register_test_merchant(&client, &env, &merchant);
    
    client.pause(&unauthorized);
}

#[test]
fn test_pause_until() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let pause_admin = Address::generate(&env);
    let merchant = Address::generate(&env);
    let _token = Address::generate(&env);
    env.mock_all_auths();

    client.set_admin(&admin);

    client.set_pause_admin(&admin, &pause_admin);
    register_test_merchant(&client, &env, &merchant);
    
    client.pause_for_duration(&pause_admin, &100);
    let is_paused = client.is_paused();
    assert_eq!(is_paused, true);
}

#[test]
fn test_pause_until_duration_passed() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup
    let admin = Address::generate(&env);
    let (token, _, token_admin) = create_token_contract(&env, &admin);
    let recipient = Address::generate(&env);
    let signer1 = Address::generate(&env);
    let signer2 = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());
    signers.push_back(signer2.clone());

    let amount = 1000_i128;

    // Mock all auths first
    env.mock_all_auths();

    token_admin.mint(&signer1, &amount);

    // Create payment requiring 2 signatures
    let payment_id = client.initiate_multisig_payment(
        &amount,
        &token,
        &recipient,
        &signers,
        &2u32,
        &(env.ledger().timestamp() + 3600),
    );

    // Add only 1 signature
    client.add_signature(&payment_id, &signer1);

    // Try to execute - should fail
    let result = client.try_execute_multisig_payment(&payment_id, &signer1);
    assert_eq!(result, Err(Ok(PaymentError::ThresholdNotMet)));
}

#[test]
fn test_cancel_multisig_payment_success() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Acá continuarán los tests multisig que siguen...
}

// --- Tests de pausa (de la rama main) ---

#[test]
fn test_pause_for_duration_success() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    env.ledger().set_timestamp(10);
    
    let admin = Address::generate(&env);
    let pause_admin = Address::generate(&env);
    let merchant = Address::generate(&env);
    let _token = Address::generate(&env);
    env.mock_all_auths();

    client.set_admin(&admin);
    client.set_pause_admin(&admin, &pause_admin);
    register_test_merchant(&client, &env, &merchant);
    
    client.pause_for_duration(&pause_admin, &100);
    let is_paused = client.is_paused();
    assert_eq!(is_paused, true);

    env.ledger().set_timestamp(150); // After pause time, so paused should be false

    let is_paused = client.is_paused();
    assert_eq!(is_paused, false);

    client.add_supported_token(&merchant, &token);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #16)")]
fn test_pause_until_duration_not_passed() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    env.ledger().set_timestamp(10);
    
    let admin = Address::generate(&env);
    let pause_admin = Address::generate(&env);
    let merchant = Address::generate(&env);
    let _token = Address::generate(&env);
    env.mock_all_auths();

    client.set_admin(&admin);
    client.set_pause_admin(&admin, &pause_admin);
    register_test_merchant(&client, &env, &merchant);
    
    client.pause_for_duration(&pause_admin, &100);
    let is_paused = client.is_paused();
    assert_eq!(is_paused, true);

    env.ledger().set_timestamp(80);

    let is_paused = client.is_paused();
    assert_eq!(is_paused, false);
}

// --- Test: cancel_multisig_payment ---
#[test]
fn test_cancel_multisig_payment() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup
    let admin = Address::generate(&env);
    let (token, _, _) = create_token_contract(&env, &admin);
    let recipient = Address::generate(&env);
    let signer1 = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());

    env.mock_all_auths();

    let payment_id = client.initiate_multisig_payment(
        &1000_i128,
        &token,
        &recipient,
        &signers,
        &1u32,
        &(env.ledger().timestamp() + 3600),
    );

    // Cancel payment
    let reason = String::from_str(&env, "Test cancellation");
    client.cancel_multisig_payment(&payment_id, &signer1, &reason);

    // Verify payment was cancelled and removed
    let result = client.try_get_multisig_payment(&payment_id);
    assert!(result.is_err()); // Should be removed after cancellation
}

// --- Test: batch_execute_payments ---
#[test]
fn test_batch_execute_payments() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup
    let admin = Address::generate(&env);
    let (token, token_client, token_admin) = create_token_contract(&env, &admin);
    let recipient = Address::generate(&env);
    let signer1 = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());

    let amount = 500_i128;

    // Mock all auths first
    env.mock_all_auths();

    token_admin.mint(&signer1, &(amount * 3)); // Enough for 3 payments

    // Create multiple payments
    let mut payment_ids = Vec::new(&env);
    for _i in 0..3 {
        let payment_id = client.initiate_multisig_payment(
            &amount,
            &token,
            &recipient,
            &signers,
            &1u32,
            &(env.ledger().timestamp() + 3600),
        );

        // Add signature
        client.add_signature(&payment_id, &signer1);
        payment_ids.push_back(payment_id);
    }

    // Batch execute
    let executed = client.batch_execute_payments(&payment_ids, &signer1);

    // Verify all payments were executed
    assert_eq!(executed.len(), 3);

    // Verify total amount transferred
    assert_eq!(token_client.balance(&recipient), amount * 3);
}

// --- Test: payment_history_retrieval ---
#[test]
fn test_payment_history_retrieval() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup
    let admin = Address::generate(&env);
    let (token, _token_client, token_admin) = create_token_contract(&env, &admin);
    let recipient = Address::generate(&env);
    let signer1 = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());

    let amount = 1000_i128;

    // Mock all auths first
    env.mock_all_auths();

    token_admin.mint(&signer1, &amount);

    // Create and execute a payment
    let payment_id = client.initiate_multisig_payment(
        &amount,
        &token,
        &recipient,
        &signers,
        &1u32,
        &(env.ledger().timestamp() + 3600),
    );

    client.add_signature(&payment_id, &signer1);
    client.execute_multisig_payment(&payment_id, &signer1);

    // Test payment history retrieval using storage directly
    let payment_record = env.as_contract(&contract_id, || {
        let storage = Storage::new(&env);
        storage.get_payment_record(payment_id)
    });

    assert!(payment_record.is_some());
    let record = payment_record.unwrap();
    assert_eq!(record.payment_id, payment_id);
    assert_eq!(record.amount, amount);
    assert_eq!(record.recipient, recipient);
    assert_eq!(record.status, PaymentStatus::Executed);
}

// --- Tests de pausa (rama main) ---
#[test]
fn test_pause_for_duration_success() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    env.ledger().set_timestamp(10);
    
    let admin = Address::generate(&env);
    let pause_admin = Address::generate(&env);
    env.mock_all_auths();

    client.set_admin(&admin);
    client.set_pause_admin(&admin, &pause_admin);
    
    client.pause_for_duration(&pause_admin, &100);
    let is_paused = client.is_paused();
    assert_eq!(is_paused, true);

    env.ledger().set_timestamp(60);

    client.pause_for_duration(&pause_admin, &100);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #17)")]
fn test_pause_pause_until_already_paused() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    env.ledger().set_timestamp(10);
    
    let admin = Address::generate(&env);
    let pause_admin = Address::generate(&env);
    env.mock_all_auths();

    client.set_admin(&admin);
    client.set_pause_admin(&admin, &pause_admin);

    client.pause_for_duration(&pause_admin, &100);
    let is_paused = client.is_paused();
    assert_eq!(is_paused, true);

    env.ledger().set_timestamp(60);

    client.pause(&pause_admin);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #17)")]
fn test_pause_until_pause_already_paused() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    env.ledger().set_timestamp(10);
    
    let admin = Address::generate(&env);
    let pause_admin = Address::generate(&env);
    env.mock_all_auths();

    client.set_admin(&admin);

    client.set_pause_admin(&admin, &pause_admin);

    client.pause_for_duration(&pause_admin, &100);
    let is_paused = client.is_paused();
    assert_eq!(is_paused, true);

    env.ledger().set_timestamp(60);

    client.pause(&pause_admin);
}

// Profile Management Tests

#[test]
fn test_update_merchant_profile() {
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
    let (token, _token_client, token_admin) = create_token_contract(&env, &admin);
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
    
    // Register merchant
    env.mock_all_auths();
    register_test_merchant(&client, &env, &merchant);
    
    // Update profile
    let update_data = ProfileUpdateData {
        update_name: true,
        name: String::from_str(&env, "Updated Merchant"),
        update_description: true,
        description: String::from_str(&env, "Updated description"),
        update_contact_info: true,
        contact_info: String::from_str(&env, "updated@merchant.com"),
        update_category: true,
        category: MerchantCategory::ECommerce,
    };
    
    env.mock_all_auths();
    client.update_merchant_profile(&merchant, &update_data);
    
    // Verify profile was updated
    let profile = client.get_merchant_profile(&merchant);
    assert_eq!(profile.name, String::from_str(&env, "Updated Merchant"));
    assert_eq!(profile.description, String::from_str(&env, "Updated description"));
    assert_eq!(profile.contact_info, String::from_str(&env, "updated@merchant.com"));
    assert_eq!(profile.category, MerchantCategory::ECommerce);
}

#[test]
fn test_update_merchant_profile_partial() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    
    // Register merchant
    env.mock_all_auths();
    register_test_merchant(&client, &env, &merchant);
    
    // Update only name
    let update_data = ProfileUpdateData {
        update_name: true,
        name: String::from_str(&env, "New Name"),
        update_description: false,
        description: String::from_str(&env, ""),
        update_contact_info: false,
        contact_info: String::from_str(&env, ""),
        update_category: false,
        category: MerchantCategory::Retail,
    };
    
    env.mock_all_auths();
    client.update_merchant_profile(&merchant, &update_data);
    
    // Verify only name was updated
    let profile = client.get_merchant_profile(&merchant);
    assert_eq!(profile.name, String::from_str(&env, "New Name"));
    assert_eq!(profile.description, String::from_str(&env, "A test merchant for unit tests"));
    assert_eq!(profile.contact_info, String::from_str(&env, "test@merchant.com"));
    assert_eq!(profile.category, MerchantCategory::Retail);
}

#[test]
fn test_set_merchant_limits() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    
    // Register merchant
    env.mock_all_auths();
    register_test_merchant(&client, &env, &merchant);
    
    // Set new limits
    let new_limit = 5_000_000_i128;
    env.mock_all_auths();
    client.set_merchant_limits(&merchant, &new_limit);
    
    // Verify limits were updated
    let profile = client.get_merchant_profile(&merchant);
    assert_eq!(profile.max_transaction_limit, new_limit);
}

#[test]
fn test_deactivate_merchant() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    
    // Register merchant
    env.mock_all_auths();
    register_test_merchant(&client, &env, &merchant);
    
    // Verify merchant is active
    let profile = client.get_merchant_profile(&merchant);
    assert_eq!(profile.active, true);
    
    // Deactivate merchant
    env.mock_all_auths();
    client.deactivate_merchant(&merchant);
    
    // Verify merchant is inactive
    let profile = client.get_merchant_profile(&merchant);
    assert_eq!(profile.active, false);
}

#[test]
#[should_panic]
fn test_payment_with_inactive_merchant() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    let admin = Address::generate(&env);
    let (token, _token_client, token_admin) = create_token_contract(&env, &admin);
    let payer = Address::generate(&env);

    // Register merchant and add token
    env.mock_all_auths();
    register_test_merchant(&client, &env, &merchant);
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

// Multi-signature payment tests

#[test]
fn test_initiate_multisig_payment_success() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup
    let admin = Address::generate(&env);
    let (token, _, _) = create_token_contract(&env, &admin);
    let recipient = Address::generate(&env);
    let signer1 = Address::generate(&env);
    let signer2 = Address::generate(&env);
    let signer3 = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());
    signers.push_back(signer2.clone());
    signers.push_back(signer3.clone());

    let amount = 1000_i128;
    let threshold = 2u32;
    let expiry = env.ledger().timestamp() + 3600; // 1 hour from now

    env.mock_all_auths();

    // Initiate payment
    let payment_id = client.initiate_multisig_payment(
        &amount,
        &token,
        &recipient,
        &signers,
        &threshold,
        &expiry,
    );

    // Verify payment was created
    let payment = client.get_multisig_payment(&payment_id);
    assert_eq!(payment.amount, amount);
    assert_eq!(payment.token, token);
    assert_eq!(payment.recipient, recipient);
    assert_eq!(payment.threshold, threshold);
    assert_eq!(payment.status, PaymentStatus::Pending);
    assert_eq!(payment.signers.len(), 3);
}
    
    // Try to process payment - should fail
    let order = create_payment_order(&env, &merchant, 100, &token, env.ledger().timestamp() + 1000);
    let signature = BytesN::from_array(&env, &[2u8; 64]);
    
    client.process_payment_with_signature(
        &Address::generate(&env),
        &order,
        &signature,
        &merchant_public
    );
}

#[test]
#[should_panic]
fn test_transaction_limit_exceeded() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    let merchant_public = BytesN::from_array(&env, &[1u8; 32]);
    
    // Setup token
    let admin = Address::generate(&env);
    let (token, _, _) = create_token_contract(&env, &admin);
    
    // Register merchant with limit of 1,000,000
    env.mock_all_auths();
    register_test_merchant(&client, &env, &merchant);
    client.add_supported_token(&merchant, &token);
    
    // Try to process payment exceeding limit - should fail
    let amount = 2_000_000_i128; // Exceeds the 1,000,000 limit
    let order = create_payment_order(&env, &merchant, amount, &token, env.ledger().timestamp() + 1000);
    let signature = BytesN::from_array(&env, &[2u8; 64]);
    
    client.process_payment_with_signature(
        &Address::generate(&env),
        &order,
        &signature,
        &merchant_public
    );
}

#[test]
#[should_panic]
fn test_invalid_name_too_long() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    env.mock_all_auths();
    
    // Create a name that's too long (over 100 characters)
    let long_name = String::from_str(&env, "A".repeat(101).as_str());
    
    client.register_merchant(
        &merchant,
        &long_name,
        &String::from_str(&env, "Description"),
        &String::from_str(&env, "contact@test.com"),
        &MerchantCategory::Retail,
    );
}

#[test]
#[should_panic]
fn test_invalid_description_too_long() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    env.mock_all_auths();
    
    // Create a description that's too long (over 500 characters)
    let long_description = String::from_str(&env, "A".repeat(501).as_str());
    
    client.register_merchant(
        &merchant,
        &String::from_str(&env, "Test Merchant"),
        &long_description,
        &String::from_str(&env, "contact@test.com"),
        &MerchantCategory::Retail,
    );
}

#[test]
#[should_panic]
fn test_invalid_transaction_limit() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    
    // Register merchant
    env.mock_all_auths();
    register_test_merchant(&client, &env, &merchant);
    
    // Try to set negative limit - should fail
    env.mock_all_auths();
    client.set_merchant_limits(&merchant, &-100_i128);
}

#[test]
#[should_panic]
fn test_merchant_already_exists() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    
    // Register merchant first time
    env.mock_all_auths();
    register_test_merchant(&client, &env, &merchant);
    
    // Try to register same merchant again - should fail
    env.mock_all_auths();
    register_test_merchant(&client, &env, &merchant);
}

#[test]
#[should_panic]
fn test_update_inactive_merchant_profile() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    
    // Register and deactivate merchant
    env.mock_all_auths();
    register_test_merchant(&client, &env, &merchant);
    client.deactivate_merchant(&merchant);
    
    // Try to update profile - should fail
    let update_data = ProfileUpdateData {
        update_name: true,
        name: String::from_str(&env, "New Name"),
        update_description: false,
        description: String::from_str(&env, ""),
        update_contact_info: false,
        contact_info: String::from_str(&env, ""),
        update_category: false,
        category: MerchantCategory::Retail,
    };
    
    client.update_merchant_profile(&merchant, &update_data);
}

#[test]
#[should_panic]
fn test_set_limits_inactive_merchant() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    
    // Register and deactivate merchant
    env.mock_all_auths();
    register_test_merchant(&client, &env, &merchant);
    client.deactivate_merchant(&merchant);
    
    // Try to set limits - should fail
    client.set_merchant_limits(&merchant, &5_000_000_i128);
}

#[test]
fn test_merchant_categories() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Test different categories
    let categories = [
        MerchantCategory::Retail,
        MerchantCategory::ECommerce,
        MerchantCategory::Hospitality,
        MerchantCategory::Professional,
        MerchantCategory::Entertainment,
        MerchantCategory::Other,
    ];
    
    for category in categories.iter() {
        let merchant = Address::generate(&env);
        env.mock_all_auths();
        
        client.register_merchant(
            &merchant,
            &String::from_str(&env, "Test Merchant"),
            &String::from_str(&env, "Description"),
            &String::from_str(&env, "contact@test.com"),
            category,
        );
        
        let profile = client.get_merchant_profile(&merchant);
        assert_eq!(profile.category, *category);
    }
}

#[test]
fn test_last_activity_timestamp_updates() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup
    let admin = Address::generate(&env);
    let (token, _, _) = create_token_contract(&env, &admin);
    let recipient = Address::generate(&env);
    let signer1 = Address::generate(&env);
    let signer2 = Address::generate(&env);
    let signer3 = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());
    signers.push_back(signer2.clone());
    signers.push_back(signer3.clone());

    let amount = 1000_i128;
    let threshold = 2u32;
    let expiry = env.ledger().timestamp() + 3600; // 1 hour from now

    env.mock_all_auths();

    // Initiate payment
    let payment_id = client.initiate_multisig_payment(
        &amount,
        &token,
        &recipient,
        &signers,
        &threshold,
        &expiry,
    );

    // Verify payment was created
    let payment = client.get_multisig_payment(&payment_id);
    assert_eq!(payment.amount, amount);
    assert_eq!(payment.token, token);
    assert_eq!(payment.recipient, recipient);
    assert_eq!(payment.threshold, threshold);
    assert_eq!(payment.status, PaymentStatus::Pending);
    assert_eq!(payment.signers.len(), 3);
}

#[test]
fn test_initiate_multisig_payment_invalid_threshold() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup
    let admin = Address::generate(&env);
    let (token, _, _) = create_token_contract(&env, &admin);
    let recipient = Address::generate(&env);
    let signer1 = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1);

    let amount = 1000_i128;
    let threshold = 5u32; // Invalid: threshold > signers count
    let expiry = env.ledger().timestamp() + 3600;

    env.mock_all_auths();

    // Should fail with InvalidThreshold
    let result = client.try_initiate_multisig_payment(
        &amount,
        &token,
        &recipient,
        &signers,
        &threshold,
        &expiry,
    );

    assert_eq!(result, Err(Ok(PaymentError::InvalidThreshold)));
}

#[test]
fn test_add_signature_success() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup payment
    let admin = Address::generate(&env);
    let (token, _, _) = create_token_contract(&env, &admin);
    let recipient = Address::generate(&env);
    let signer1 = Address::generate(&env);
    let signer2 = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());
    signers.push_back(signer2.clone());

    env.mock_all_auths();

    let payment_id = client.initiate_multisig_payment(
        &1000_i128,
        &token,
        &recipient,
        &signers,
        &2u32,
        &(env.ledger().timestamp() + 3600),
    );

    // Add signature from signer1
    client.add_signature(&payment_id, &signer1);

    // Verify signature was added
    let payment = client.get_multisig_payment(&payment_id);
    assert_eq!(payment.signatures.len(), 1);
    assert_eq!(payment.signatures.get(signer1).unwrap(), true);
}

#[test]
fn test_add_signature_not_a_signer() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup payment
    let admin = Address::generate(&env);
    let (token, _, _) = create_token_contract(&env, &admin);
    let recipient = Address::generate(&env);
    let signer1 = Address::generate(&env);
    let not_signer = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1);

    env.mock_all_auths();

    let payment_id = client.initiate_multisig_payment(
        &1000_i128,
        &token,
        &recipient,
        &signers,
        &1u32,
        &(env.ledger().timestamp() + 3600),
    );

    // Try to add signature from non-signer
    let result = client.try_add_signature(&payment_id, &not_signer);
    assert_eq!(result, Err(Ok(PaymentError::NotASigner)));
}

#[test]
fn test_execute_multisig_payment_success() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup token and balances
    let admin = Address::generate(&env);
    let (token, token_client, token_admin) = create_token_contract(&env, &admin);
    let recipient = Address::generate(&env);
    let signer1 = Address::generate(&env);
    let signer2 = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());
    signers.push_back(signer2.clone());

    let amount = 1000_i128;

    // Mock all auths first
    env.mock_all_auths();

    // Mint tokens to signer1 (executor)
    token_admin.mint(&signer1, &amount);

    // Create payment
    let payment_id = client.initiate_multisig_payment(
        &amount,
        &token,
        &recipient,
        &signers,
        &2u32,
        &(env.ledger().timestamp() + 3600),
    );

    // Add signatures
    client.add_signature(&payment_id, &signer1);
    client.add_signature(&payment_id, &signer2);

    // Execute payment
    client.execute_multisig_payment(&payment_id, &signer1);

    // Verify payment was executed (should be removed from active payments)
    let result = client.try_get_multisig_payment(&payment_id);
    assert!(result.is_err()); // Should be removed after execution

    // Verify token transfer
    assert_eq!(token_client.balance(&recipient), amount);
    assert_eq!(token_client.balance(&signer1), 0);
}

#[test]
fn test_execute_multisig_payment_threshold_not_met() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup
    let admin = Address::generate(&env);
    let (token, _, _) = create_token_contract(&env, &admin);
    let recipient = Address::generate(&env);
    let signer1 = Address::generate(&env);
    let signer2 = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());
    signers.push_back(signer2.clone());

    env.mock_all_auths();

    let payment_id = client.initiate_multisig_payment(
        &1000_i128,
        &token,
        &recipient,
        &signers,
        &2u32,
        &(env.ledger().timestamp() + 3600),
    );

    // Add only one signature (threshold is 2)
    client.add_signature(&payment_id, &signer1);

    // Try to execute with only one signature
    let result = client.try_execute_multisig_payment(&payment_id, &signer1);
    assert_eq!(result, Err(Ok(PaymentError::ThresholdNotMet)));
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #9)")]
fn test_register_merchant_paused() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let pause_admin = Address::generate(&env);
    let merchant = Address::generate(&env);
    env.mock_all_auths();

    client.set_admin(&admin);
    client.set_pause_admin(&admin, &pause_admin);
    client.pause(&pause_admin);

    client.register_merchant(&merchant);
}

#[test]
fn test_contract_is_paused() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let pause_admin = Address::generate(&env);
    env.mock_all_auths();

    client.set_admin(&admin);
    client.set_pause_admin(&admin, &pause_admin);
    client.pause(&pause_admin);

    let is_paused = client.is_paused();

    assert_eq!(is_paused, true);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #9)")]
fn test_add_supported_token_paused() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let pause_admin = Address::generate(&env);
    let merchant = Address::generate(&env);
    let _token = Address::generate(&env);
    env.mock_all_auths();

    client.set_admin(&admin);

    client.set_pause_admin(&admin, &pause_admin);
    // Register merchant first
    client.register_merchant(&merchant);
    
    client.pause(&pause_admin);

    // Add supported token
    client.add_supported_token(&merchant, &token);
}

#[test]
fn test_pause_unpause() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let pause_admin = Address::generate(&env);
    let merchant = Address::generate(&env);
    let _token = Address::generate(&env);
    env.mock_all_auths();

    client.set_admin(&admin);

    client.set_pause_admin(&admin, &pause_admin);
    // Register merchant first
    client.register_merchant(&merchant);
    
    client.pause(&pause_admin);

    let is_paused = client.is_paused();
    assert_eq!(is_paused, true);

    client.unpause(&pause_admin);

    let is_paused = client.is_paused();
    assert_eq!(is_paused, false);

    // Add supported token
    client.add_supported_token(&merchant, &token);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #10)")]
fn test_double_pause() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let pause_admin = Address::generate(&env);
    let merchant = Address::generate(&env);
    let _token = Address::generate(&env);
    env.mock_all_auths();

    client.set_admin(&admin);

    client.set_pause_admin(&admin, &pause_admin);
    // Register merchant first
    client.register_merchant(&merchant);
    
    client.pause(&pause_admin);

    let is_paused = client.is_paused();
    assert_eq!(is_paused, true);
    
    client.pause(&pause_admin);
}


#[test]
#[should_panic(expected = "HostError: Error(Contract, #8)")]
fn test_pause_without_set_pause_admin() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let merchant = Address::generate(&env);
    env.mock_all_auths();
    client.register_merchant(&merchant);
    
    client.pause(&admin);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #1)")]
fn test_unauthorized() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let pause_admin = Address::generate(&env);
    let unauthorized = Address::generate(&env);
    let merchant = Address::generate(&env);
    let token = Address::generate(&env);
    env.mock_all_auths();

    client.set_admin(&admin);

    client.set_pause_admin(&admin, &pause_admin);
    client.register_merchant(&merchant);
    
    client.pause(&unauthorized);
}

#[test]
fn test_pause_until() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    let pause_admin = Address::generate(&env);
    let merchant = Address::generate(&env);
    let _token = Address::generate(&env);
    env.mock_all_auths();

    client.set_admin(&admin);

    client.set_pause_admin(&admin, &pause_admin);
    client.register_merchant(&merchant);
    
    client.pause_for_duration(&pause_admin, &100);
    let is_paused = client.is_paused();
    assert_eq!(is_paused, true);
}

#[test]
fn test_pause_until_duration_passed() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup
    let admin = Address::generate(&env);
    let (token, _, token_admin) = create_token_contract(&env, &admin);
    let recipient = Address::generate(&env);
    let signer1 = Address::generate(&env);
    let signer2 = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());
    signers.push_back(signer2.clone());

    let amount = 1000_i128;

    // Mock all auths first
    env.mock_all_auths();

    token_admin.mint(&signer1, &amount);

    // Create payment requiring 2 signatures
    let payment_id = client.initiate_multisig_payment(
        &amount,
        &token,
        &recipient,
        &signers,
        &2u32,
        &(env.ledger().timestamp() + 3600),
    );

    // Add only 1 signature
    client.add_signature(&payment_id, &signer1);

    // Try to execute - should fail
    let result = client.try_execute_multisig_payment(&payment_id, &signer1);
    assert_eq!(result, Err(Ok(PaymentError::ThresholdNotMet)));
}

#[test]
fn test_cancel_multisig_payment_success() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Acá continuarán los tests multisig que siguen...
}

// --- Tests de pausa (de la rama main) ---

#[test]
fn test_pause_for_duration_success() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    env.ledger().set_timestamp(10);
    
    let admin = Address::generate(&env);
    let pause_admin = Address::generate(&env);
    let merchant = Address::generate(&env);
    let _token = Address::generate(&env);
    env.mock_all_auths();

    client.set_admin(&admin);
    client.set_pause_admin(&admin, &pause_admin);
    client.register_merchant(&merchant);
    
    client.pause_for_duration(&pause_admin, &100);
    let is_paused = client.is_paused();
    assert_eq!(is_paused, true);

    env.ledger().set_timestamp(150); // After pause time, so paused should be false

    let is_paused = client.is_paused();
    assert_eq!(is_paused, false);

    client.add_supported_token(&merchant, &token);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #9)")]
fn test_pause_until_duration_not_passed() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    env.ledger().set_timestamp(10);
    
    let admin = Address::generate(&env);
    let pause_admin = Address::generate(&env);
    let merchant = Address::generate(&env);
    let _token = Address::generate(&env);
    env.mock_all_auths();

    client.set_admin(&admin);
    client.set_pause_admin(&admin, &pause_admin);
    client.register_merchant(&merchant);
    
    client.pause_for_duration(&pause_admin, &100);
    let is_paused = client.is_paused();
    assert_eq!(is_paused, true);

    env.ledger().set_timestamp(80);

    let is_paused = client.is_paused();
    assert_eq!(is_paused, false);
}

// --- Test: cancel_multisig_payment ---
#[test]
fn test_cancel_multisig_payment() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup
    let admin = Address::generate(&env);
    let (token, _, _) = create_token_contract(&env, &admin);
    let recipient = Address::generate(&env);
    let signer1 = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());

    env.mock_all_auths();

    let payment_id = client.initiate_multisig_payment(
        &1000_i128,
        &token,
        &recipient,
        &signers,
        &1u32,
        &(env.ledger().timestamp() + 3600),
    );

    // Cancel payment
    let reason = String::from_str(&env, "Test cancellation");
    client.cancel_multisig_payment(&payment_id, &signer1, &reason);

    // Verify payment was cancelled and removed
    let result = client.try_get_multisig_payment(&payment_id);
    assert!(result.is_err()); // Should be removed after cancellation
}

// --- Test: batch_execute_payments ---
#[test]
fn test_batch_execute_payments() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup
    let admin = Address::generate(&env);
    let (token, token_client, token_admin) = create_token_contract(&env, &admin);
    let recipient = Address::generate(&env);
    let signer1 = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());

    let amount = 500_i128;

    // Mock all auths first
    env.mock_all_auths();

    token_admin.mint(&signer1, &(amount * 3)); // Enough for 3 payments

    // Create multiple payments
    let mut payment_ids = Vec::new(&env);
    for _i in 0..3 {
        let payment_id = client.initiate_multisig_payment(
            &amount,
            &token,
            &recipient,
            &signers,
            &1u32,
            &(env.ledger().timestamp() + 3600),
        );

        // Add signature
        client.add_signature(&payment_id, &signer1);
        payment_ids.push_back(payment_id);
    }

    // Batch execute
    let executed = client.batch_execute_payments(&payment_ids, &signer1);

    // Verify all payments were executed
    assert_eq!(executed.len(), 3);

    // Verify total amount transferred
    assert_eq!(token_client.balance(&recipient), amount * 3);
}

// --- Test: payment_history_retrieval ---
#[test]
fn test_payment_history_retrieval() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup
    let admin = Address::generate(&env);
    let (token, _token_client, token_admin) = create_token_contract(&env, &admin);
    let recipient = Address::generate(&env);
    let signer1 = Address::generate(&env);

    let mut signers = Vec::new(&env);
    signers.push_back(signer1.clone());

    let amount = 1000_i128;

    // Mock all auths first
    env.mock_all_auths();

    token_admin.mint(&signer1, &amount);

    // Create and execute a payment
    let payment_id = client.initiate_multisig_payment(
        &amount,
        &token,
        &recipient,
        &signers,
        &1u32,
        &(env.ledger().timestamp() + 3600),
    );

    client.add_signature(&payment_id, &signer1);
    client.execute_multisig_payment(&payment_id, &signer1);

    // Test payment history retrieval using storage directly
    let payment_record = env.as_contract(&contract_id, || {
        let storage = Storage::new(&env);
        storage.get_payment_record(payment_id)
    });

    assert!(payment_record.is_some());
    let record = payment_record.unwrap();
    assert_eq!(record.payment_id, payment_id);
    assert_eq!(record.amount, amount);
    assert_eq!(record.recipient, recipient);
    assert_eq!(record.status, PaymentStatus::Executed);
}

// --- Tests de pausa (rama main) ---
#[test]
fn test_pause_for_duration_success() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    env.ledger().set_timestamp(10);
    
    let admin = Address::generate(&env);
    let pause_admin = Address::generate(&env);
    env.mock_all_auths();

    client.set_admin(&admin);
    client.set_pause_admin(&admin, &pause_admin);
    
    client.pause_for_duration(&pause_admin, &100);
    let is_paused = client.is_paused();
    assert_eq!(is_paused, true);

    env.ledger().set_timestamp(60);

    client.pause_for_duration(&pause_admin, &100);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #10)")]
fn test_pause_pause_until_already_paused() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    env.ledger().set_timestamp(10);
    
    let admin = Address::generate(&env);
    let pause_admin = Address::generate(&env);
    env.mock_all_auths();

    client.set_admin(&admin);
    client.set_pause_admin(&admin, &pause_admin);

    client.pause_for_duration(&pause_admin, &100);
    let is_paused = client.is_paused();
    assert_eq!(is_paused, true);

    env.ledger().set_timestamp(60);

    client.pause(&pause_admin);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #10)")]
fn test_pause_until_pause_already_paused() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    env.ledger().set_timestamp(10);
    
    let admin = Address::generate(&env);
    let pause_admin = Address::generate(&env);
    env.mock_all_auths();

    client.set_admin(&admin);

    client.set_pause_admin(&admin, &pause_admin);

    client.pause_for_duration(&pause_admin, &100);
    let is_paused = client.is_paused();
    assert_eq!(is_paused, true);

    env.ledger().set_timestamp(60);

    client.pause(&pause_admin);
    
    let initial_profile = client.get_merchant_profile(&merchant);
    let initial_timestamp = initial_profile.last_activity_timestamp;
    
    // Add token (should update last activity)
    env.mock_all_auths();
    let token = Address::generate(&env);
    client.add_supported_token(&merchant, &token);
    
    let updated_profile = client.get_merchant_profile(&merchant);
    assert!(updated_profile.last_activity_timestamp >= initial_timestamp);
}
