#![cfg(test)]

use soroban_sdk::{
    testutils::Address as _,
    Address, Env, String, BytesN,
    token,
};
use crate::{
    PaymentProcessingContract, PaymentProcessingContractClient,
    types::{PaymentOrder, MerchantCategory, ProfileUpdateData}
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
    register_test_merchant(&client, &env, &merchant);
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
    register_test_merchant(&client, &env, &merchant);
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
    register_test_merchant(&client, &env, &merchant);
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
    register_test_merchant(&client, &env, &merchant);
    
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

// Profile Management Tests

#[test]
fn test_update_merchant_profile() {
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
    let merchant_public = BytesN::from_array(&env, &[1u8; 32]);
    
    // Setup token
    let admin = Address::generate(&env);
    let (token, _, _) = create_token_contract(&env, &admin);
    
    // Register merchant and add token
    env.mock_all_auths();
    register_test_merchant(&client, &env, &merchant);
    client.add_supported_token(&merchant, &token);
    
    // Deactivate merchant
    client.deactivate_merchant(&merchant);
    
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

    let merchant = Address::generate(&env);
    
    // Register merchant
    env.mock_all_auths();
    register_test_merchant(&client, &env, &merchant);
    
    let initial_profile = client.get_merchant_profile(&merchant);
    let initial_timestamp = initial_profile.last_activity_timestamp;
    
    // Add token (should update last activity)
    env.mock_all_auths();
    let token = Address::generate(&env);
    client.add_supported_token(&merchant, &token);
    
    let updated_profile = client.get_merchant_profile(&merchant);
    assert!(updated_profile.last_activity_timestamp >= initial_timestamp);
}
