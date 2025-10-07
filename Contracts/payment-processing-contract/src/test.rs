#![cfg(test)]

use soroban_sdk::{
    testutils::Address as _,
    Address, Env, String, BytesN, Vec,
    token,
};
use crate::{
    PaymentProcessingContract, PaymentProcessingContractClient,
    types::{PaymentOrder, PaymentStatus},
    error::PaymentError,
    storage::Storage,
};
    testutils::{Address as _, Ledger}, token, Address, BytesN, Env, String, Vec
};
use crate::{
    storage::Storage, types::PaymentOrder, PaymentProcessingContract,
    PaymentProcessingContractClient,
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
        amount: payment_amount,
        token: token.clone(),
        nonce: 12345u64,
        expiration: env.ledger().timestamp() + 1000,
        order_id: String::from_str(&env, "TEST_ORDER_1"),
        fee_amount: 0, // Initial fee amount, will be calculated during processing
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
        amount,
        token: token.clone(),
        nonce: 12345u64,
        expiration: env.ledger().timestamp() + 1000,
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
    let expired_time = current_time - 1000; // Set expiration in the past
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
    let expiration = env.ledger().timestamp() + 1000;
    let order = create_payment_order(&env, &merchant, amount, &token, expiration);

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
    let expiration = env.ledger().timestamp() + 1000;
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
    let token = Address::generate(&env);
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
    let token = Address::generate(&env);
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
    let token = Address::generate(&env);
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
    let token = Address::generate(&env);
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
    let token = Address::generate(&env);
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
    let token = Address::generate(&env);
    env.mock_all_auths();

    client.set_admin(&admin);
    client.set_pause_admin(&admin, &pause_admin);
    client.register_merchant(&merchant);
    
    client.pause_for_duration(&pause_admin, &100);
    let is_paused = client.is_paused();
    assert_eq!(is_paused, true);

    env.ledger().set_timestamp(80);

    let is_pau_

    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);
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

