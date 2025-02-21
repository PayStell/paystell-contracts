#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _},
    Address, Env, String, token,
};
use crate::{PaymentProcessingContract, PaymentProcessingContractClient};

#[test]
fn test_register_merchant() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let merchant = Address::generate(&env);
    
    // Mock merchant authorization
    env.mock_all_auths();
    
    // Test merchant registration
    client.register_merchant(&merchant);
    
    // Verify authorization
    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    let auth = auths.first().unwrap();
    assert_eq!(auth.0, merchant);
}

#[test]
fn test_process_payment() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    // Setup test token contract
    let token_admin = Address::generate(&env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token = token_contract.address();
    let token_client = token::Client::new(&env, &token);
    let token_admin_client = token::StellarAssetClient::new(&env, &token);

    let merchant = Address::generate(&env);
    let payer = Address::generate(&env);
    let amount = 100_i128;
    let description = String::from_str(&env, "Test payment");
    
    // Mock all authorizations
    env.mock_all_auths();
    
    // Register merchant and add token
    client.register_merchant(&merchant);
    client.add_supported_token(&merchant, &token);
    
    // Create payment link
    let link_id = client.create_payment_link(
        &merchant,
        &amount,
        &token,
        &description
    );
    
    // Setup token balances - mint to payer
    token_client.mock_all_auths();
    token_admin_client.mint(&payer, &amount);
    
    // Approve spending
    token_client.approve(&payer, &contract_id, &amount, &200);
    
    // Process payment
    client.process_payment(&link_id, &payer);
    
    // Verify token transfer
    let payer_balance = token_client.balance(&payer);
    let merchant_balance = token_client.balance(&merchant);
    assert_eq!(merchant_balance, amount);
    assert_eq!(payer_balance, 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_duplicate_payment() {
    let env = Env::default();
    let contract_id = env.register(PaymentProcessingContract {}, ());
    let client = PaymentProcessingContractClient::new(&env, &contract_id);

    let token_admin = Address::generate(&env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token = token_contract.address();
    let token_client = token::Client::new(&env, &token);
    let token_admin_client = token::StellarAssetClient::new(&env, &token);

    let merchant = Address::generate(&env);
    let payer = Address::generate(&env);
    let amount = 100_i128;
    let description = String::from_str(&env, "Test payment");
    
    // Mock all authorizations
    env.mock_all_auths();
    
    // Setup
    client.register_merchant(&merchant);
    client.add_supported_token(&merchant, &token);
    let link_id = client.create_payment_link(&merchant, &amount, &token, &description);
    
    // Setup token balance - mint to payer
    token_client.mock_all_auths();
    token_admin_client.mint(&payer, &(amount * 2));
    
    // Approve spending
    token_client.approve(&payer, &contract_id, &(amount * 2), &200);
    
    // First payment should succeed
    client.process_payment(&link_id, &payer);
    
    // Verify first payment was successful
    let merchant_balance = token_client.balance(&merchant);
    assert_eq!(merchant_balance, amount);
    
    // Second payment should fail with PaymentAlreadyProcessed    
    client.process_payment(&link_id, &payer);
}
