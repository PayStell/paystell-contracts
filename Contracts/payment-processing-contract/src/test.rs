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
