#![cfg(test)]

use soroban_sdk::{
    testutils::Address as _,
    Address, Env, String, BytesN,
    token,
};

use crate::{PaymentProcessingContract, PaymentProcessingContractClient, types::PaymentOrder};

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
    client.process_payment_with_signature(
        &payer,
        &order,
        &signature,
        &merchant_public
    );
    
    // Verify balances
    assert_eq!(token_client.balance(&merchant), amount);
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
