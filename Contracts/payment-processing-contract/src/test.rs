#![cfg(test)]

use soroban_sdk::{
    testutils::Address as _,
    Address, Env, String, BytesN,
    token,
};
use crate::{
    PaymentProcessingContract, PaymentProcessingContractClient,
    types::{PaymentOrder, PaymentStatus, PaymentRecordQuery, QueryFilter},
};

fn create_token_contract<'a>(e: &'a Env, admin: &Address) -> (Address, token::Client<'a>, token::StellarAssetClient<'a>) {
    let token_id = e.register_stellar_asset_contract_v2(admin.clone());
    let token = token_id.address();
    let token_client = token::Client::new(e, &token);
    let token_admin_client = token::StellarAssetClient::new(e, &token);
    (token, token_client, token_admin_client)
}

fn create_payment_order(
    env: &Env,
    merchant: &Address,
    amount: i128,
    token: &Address,
    expiration: u64,
) -> PaymentOrder {
    PaymentOrder {
        merchant_address: merchant.clone(),
        amount,
        token: token.clone(),
        nonce: env.ledger().timestamp(),
        expiration,
        order_id: String::from_str(&env, "TEST_ORDER_1"),
    }
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
fn test_successful_payment_with_signature() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup merchant with keys
    let merchant = Address::generate(&env);
    
    // Use any 32-byte array for public key
    let merchant_public = BytesN::from_array(&env, &[1u8; 32]);

    // Setup token and order with fixed values for deterministic testing
    let admin = Address::generate(&env);
    let (token, token_client, token_admin) = create_token_contract(&env, &admin);
    let payer = Address::generate(&env);
    let amount = 100_i128;
    
    // Register merchant and add token support
    env.mock_all_auths();
    client.register_merchant(&merchant);
    env.mock_all_auths();
    client.add_supported_token(&merchant, &token);
    
    // Create payment order with fixed values
    let order = PaymentOrder {
        merchant_address: merchant.clone(),
        amount,
        token: token.clone(),
        nonce: 12345u64,
        expiration: env.ledger().timestamp() + 1000,
        order_id: String::from_str(&env, "TEST_ORDER_1"),
    };
    
    // Use any 64-byte array for signature
    let signature = BytesN::from_array(&env, &[2u8; 64]);

    // Setup token balances
    token_admin.mint(&payer, &amount);
    
    // Mock all auths for the payment
    env.mock_all_auths();
    
    // Process payment
    let payment_id = client.process_payment_with_signature(
        &payer,
        &order,
        &signature,
        &merchant_public
    );
    
    // Verify balances
    assert_eq!(token_client.balance(&merchant), amount);
    assert_eq!(token_client.balance(&payer), 0);
    
    // Verify payment record was created
    let payment_record = client.get_payment_record(&payment_id);
    assert_eq!(payment_record.payer, payer);
    assert_eq!(payment_record.merchant, merchant);
    assert_eq!(payment_record.amount, amount);
    assert_eq!(payment_record.status, PaymentStatus::Completed);
    assert!(payment_record.completed_at.is_some());
    
    // Verify payment appears in merchant's history
    let merchant_payments = client.get_merchant_payments(&merchant);
    assert_eq!(merchant_payments.len(), 1);
    assert_eq!(merchant_payments.get(0).unwrap(), payment_id);
    
    // Verify payment appears in payer's history
    let payer_payments = client.get_payer_payments(&payer);
    assert_eq!(payer_payments.len(), 1);
    assert_eq!(payer_payments.get(0).unwrap(), payment_id);
}

#[test]
#[should_panic]
fn test_expired_order() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup merchant with keys
    let merchant = Address::generate(&env);
    let merchant_public = BytesN::from_array(&env, &[
        0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7,
        0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
        0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25,
        0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
    ]);
    
    // Setup token
    let admin = Address::generate(&env);
    let (token, _, _) = create_token_contract(&env, &admin);
    
    // Register merchant and add token
    env.mock_all_auths();
    client.register_merchant(&merchant);
    client.add_supported_token(&merchant, &token);
    
    // Create expired order
    let current_time = env.ledger().timestamp();
    let expired_time = current_time - 1000; // Set expiration in the past
    let order = create_payment_order(&env, &merchant, 100, &token, expired_time);
    
    // Create test signature
    let signature = BytesN::from_array(&env, &[3u8; 64]); // Test signature
    
    // Should fail due to expired order
    client.process_payment_with_signature(
        &Address::generate(&env),
        &order,
        &signature,
        &merchant_public
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
    let merchant_public = BytesN::from_array(&env, &[
        0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7,
        0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
        0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25,
        0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
    ]);
    
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
    let expiration = env.ledger().timestamp() + 1000;
    let order = create_payment_order(&env, &merchant, amount, &token, expiration);
    
    // Create test signature
    let signature = BytesN::from_array(&env, &[3u8; 64]); // Test signature
    
    // Setup token balances
    token_admin.mint(&payer, &(amount * 2));
    
    // First payment should succeed
    env.mock_all_auths();
    client.process_payment_with_signature(
        &payer,
        &order.clone(),
        &signature,
        &merchant_public
    );
    
    // Second payment with same nonce should fail
    client.process_payment_with_signature(
        &payer,
        &order,
        &signature,
        &merchant_public
    );
}

#[test]
#[should_panic]
fn test_unsupported_token() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup merchant with keys
    let merchant = Address::generate(&env);
    let merchant_public = BytesN::from_array(&env, &[
        0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7,
        0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
        0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25,
        0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
    ]);
    
    // Setup token (but don't add it as supported)
    let admin = Address::generate(&env);
    let (token, _, _) = create_token_contract(&env, &admin);
    
    // Register merchant (but don't add token support)
    env.mock_all_auths();
    client.register_merchant(&merchant);
    
    // Create order with unsupported token
    let expiration = env.ledger().timestamp() + 1000;
    let order = create_payment_order(&env, &merchant, 100, &token, expiration);
    
    // Create test signature
    let signature = BytesN::from_array(&env, &[3u8; 64]); // Test signature
    
    // Should fail due to unsupported token
    client.process_payment_with_signature(
        &Address::generate(&env),
        &order,
        &signature,
        &merchant_public
    );
}

#[test]
fn test_payment_history_query_by_merchant() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    let merchant_public = BytesN::from_array(&env, &[1u8; 32]);
    
    let admin = Address::generate(&env);
    let (token, _, token_admin) = create_token_contract(&env, &admin);
    
    let payer1 = Address::generate(&env);
    let payer2 = Address::generate(&env);
    let amount = 100_i128;
    
    // Register merchant and add token
    env.mock_all_auths();
    client.register_merchant(&merchant);
    client.add_supported_token(&merchant, &token);
    
    // Create and process first payment
    let order1 = PaymentOrder {
        merchant_address: merchant.clone(),
        amount,
        token: token.clone(),
        nonce: 1000u64,
        expiration: env.ledger().timestamp() + 1000,
        order_id: String::from_str(&env, "ORDER_1"),
    };
    
    token_admin.mint(&payer1, &amount);
    env.mock_all_auths();
    let payment_id1 = client.process_payment_with_signature(
        &payer1,
        &order1,
        &BytesN::from_array(&env, &[2u8; 64]),
        &merchant_public
    );
    
    // Create and process second payment
    let order2 = PaymentOrder {
        merchant_address: merchant.clone(),
        amount,
        token: token.clone(),
        nonce: 2000u64,
        expiration: env.ledger().timestamp() + 1000,
        order_id: String::from_str(&env, "ORDER_2"),
    };
    
    token_admin.mint(&payer2, &amount);
    env.mock_all_auths();
    let payment_id2 = client.process_payment_with_signature(
        &payer2,
        &order2,
        &BytesN::from_array(&env, &[2u8; 64]),
        &merchant_public
    );
    
    // Query payments by merchant
    let query = PaymentRecordQuery {
        filter: QueryFilter::ByMerchant(merchant.clone()),
        from_timestamp: None,
        to_timestamp: None,
    };
    
    let results = client.query_payments(&query);
    assert_eq!(results.len(), 2);
    
    // Verify both payments are in results
    let mut found_payment1 = false;
    let mut found_payment2 = false;
    for record in results.iter() {
        if record.payment_id == payment_id1 {
            found_payment1 = true;
        }
        if record.payment_id == payment_id2 {
            found_payment2 = true;
        }
    }
    assert!(found_payment1);
    assert!(found_payment2);
}

#[test]
fn test_payment_history_query_by_payer() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant1 = Address::generate(&env);
    let merchant2 = Address::generate(&env);
    let merchant_public = BytesN::from_array(&env, &[1u8; 32]);
    
    let admin = Address::generate(&env);
    let (token, _, token_admin) = create_token_contract(&env, &admin);
    
    let payer = Address::generate(&env);
    let amount = 100_i128;
    
    // Register merchants and add token
    env.mock_all_auths();
    client.register_merchant(&merchant1);
    client.add_supported_token(&merchant1, &token);
    client.register_merchant(&merchant2);
    client.add_supported_token(&merchant2, &token);
    
    // Create and process payment to merchant1
    let order1 = PaymentOrder {
        merchant_address: merchant1.clone(),
        amount,
        token: token.clone(),
        nonce: 3000u64,
        expiration: env.ledger().timestamp() + 1000,
        order_id: String::from_str(&env, "ORDER_3"),
    };
    
    token_admin.mint(&payer, &(amount * 2));
    env.mock_all_auths();
    let payment_id1 = client.process_payment_with_signature(
        &payer,
        &order1,
        &BytesN::from_array(&env, &[2u8; 64]),
        &merchant_public
    );
    
    // Create and process payment to merchant2
    let order2 = PaymentOrder {
        merchant_address: merchant2.clone(),
        amount,
        token: token.clone(),
        nonce: 4000u64,
        expiration: env.ledger().timestamp() + 1000,
        order_id: String::from_str(&env, "ORDER_4"),
    };
    
    env.mock_all_auths();
    let payment_id2 = client.process_payment_with_signature(
        &payer,
        &order2,
        &BytesN::from_array(&env, &[2u8; 64]),
        &merchant_public
    );
    
    // Query payments by payer
    let query = PaymentRecordQuery {
        filter: QueryFilter::ByPayer(payer.clone()),
        from_timestamp: None,
        to_timestamp: None,
    };
    
    let results = client.query_payments(&query);
    assert_eq!(results.len(), 2);
    
    // Verify both payments are in results
    let mut found_payment1 = false;
    let mut found_payment2 = false;
    for record in results.iter() {
        if record.payment_id == payment_id1 {
            found_payment1 = true;
        }
        if record.payment_id == payment_id2 {
            found_payment2 = true;
        }
    }
    assert!(found_payment1);
    assert!(found_payment2);
}

#[test]
fn test_payment_validation() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    let merchant_public = BytesN::from_array(&env, &[1u8; 32]);
    
    let admin = Address::generate(&env);
    let (token, _, token_admin) = create_token_contract(&env, &admin);
    let payer = Address::generate(&env);
    let amount = 100_i128;
    
    // Register merchant and add token
    env.mock_all_auths();
    client.register_merchant(&merchant);
    client.add_supported_token(&merchant, &token);
    
    // Create and process payment
    let order = PaymentOrder {
        merchant_address: merchant.clone(),
        amount,
        token: token.clone(),
        nonce: 5000u64,
        expiration: env.ledger().timestamp() + 1000,
        order_id: String::from_str(&env, "ORDER_5"),
    };
    
    token_admin.mint(&payer, &amount);
    env.mock_all_auths();
    let payment_id = client.process_payment_with_signature(
        &payer,
        &order,
        &BytesN::from_array(&env, &[2u8; 64]),
        &merchant_public
    );
    
    // Validate payment record
    let is_valid = client.validate_payment(&payment_id);
    assert!(is_valid);
}

#[test]
fn test_payment_reconciliation() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    let merchant_public = BytesN::from_array(&env, &[1u8; 32]);
    
    let admin = Address::generate(&env);
    let (token, _, token_admin) = create_token_contract(&env, &admin);
    let payer = Address::generate(&env);
    let amount = 100_i128;
    
    // Register merchant and add token
    env.mock_all_auths();
    client.register_merchant(&merchant);
    client.add_supported_token(&merchant, &token);
    
    // Create and process payment
    let order = PaymentOrder {
        merchant_address: merchant.clone(),
        amount,
        token: token.clone(),
        nonce: 6000u64,
        expiration: env.ledger().timestamp() + 1000,
        order_id: String::from_str(&env, "ORDER_6"),
    };
    
    token_admin.mint(&payer, &amount);
    env.mock_all_auths();
    let payment_id = client.process_payment_with_signature(
        &payer,
        &order,
        &BytesN::from_array(&env, &[2u8; 64]),
        &merchant_public
    );
    
    // Create vector with payment ID for reconciliation
    let mut payment_ids = soroban_sdk::Vec::new(&env);
    payment_ids.push_back(payment_id);
    
    // Reconcile payments
    let fixed_count = client.reconcile_payments(&payment_ids);
    
    // Should be 0 since payment is already consistent
    assert_eq!(fixed_count, 0);
}
