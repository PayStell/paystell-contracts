// test.rs
#![cfg(test)]
use soroban_sdk::{Env, Address, Vec, Bytes, symbol_short, Val, IntoVal, TryFromVal, testutils::{Address as _}, Symbol, Map};
use crate::{UpgradeableProxyContract, UpgradeableProxyContractClient};

// A dummy implementation contract to test forwarding.
use soroban_sdk::{contract, contractimpl};

#[contract]
pub struct DummyImpl;

#[contractimpl]
impl DummyImpl {
    pub fn version(_env: Env) -> u32 { 1 }
    pub fn ping(_env: Env, x: u32) -> u32 { x + 7 }
    pub fn schema_version(_env: Env) -> u32 { 1 }
    pub fn proxy_compatible(_env: Env, _current: Address) -> bool { true }
    pub fn validate_migration(_env: Env, _state_hash: Bytes) -> bool { true }
    pub fn migrate(_env: Env) { /* no-op */ }
    pub fn rollback_migration(_env: Env) { /* no-op */ }
    pub fn rollback_compatible(_env: Env, _current: Address) -> bool { true }
}

#[test]
fn test_init_and_propose_execute_flow() {
    let env = Env::default();
    let proxy_id = env.register(UpgradeableProxyContract{}, ());
    let client = UpgradeableProxyContractClient::new(&env, &proxy_id);

    // admins
    let admin1 = Address::generate(&env);
    let admin2 = Address::generate(&env);
    let admins = Vec::from_array(&env, [admin1.clone(), admin2.clone()]);

    env.mock_all_auths();
    client.init(&admins, &2u32, &0u64); // threshold 2, no delay

    // register dummy impl
    let impl_addr = env.register(DummyImpl{}, ());

    // propose upgrade
    env.mock_all_auths();
    // metadata first byte 1 -> request migration
    let meta = Bytes::from_slice(&env, &[1u8, b'v', b'1']);
    let prop_id = client.propose_upgrade(&impl_addr, &meta);

    // approvals
    env.mock_all_auths(); client.approve_upgrade(&prop_id, &admin1);
    env.mock_all_auths(); client.approve_upgrade(&prop_id, &admin2);

    // execute immediately (no delay)
    env.mock_all_auths(); client.execute_upgrade(&prop_id);

    // forward call to ping
    let arg: Val = 5_u32.into_val(&env);
    let res_val = client.forward(&symbol_short!("ping"), &Vec::from_array(&env, [arg]));
    let res_u32: u32 = u32::try_from_val(&env, &res_val).unwrap();
    assert_eq!(res_u32, 12u32);
}

// Implementation without schema_version should cause validation failure
use soroban_sdk::{contract as contract2, contractimpl as contractimpl2};
#[contract2]
pub struct BadImpl;
#[contractimpl2]
impl BadImpl { pub fn alt_ping(_env: Env) -> u32 { 1 } }

#[test]
fn test_validation_missing_schema_version() {
    let env = Env::default();
    let proxy_id = env.register(UpgradeableProxyContract{}, ());
    let client = UpgradeableProxyContractClient::new(&env, &proxy_id);
    let admin1 = Address::generate(&env);
    let admin2 = Address::generate(&env);
    let admins = Vec::from_array(&env, [admin1.clone(), admin2.clone()]);
    env.mock_all_auths();
    client.init(&admins, &2u32, &0u64);
    let bad_addr = env.register(BadImpl{}, ());
    let meta = Bytes::from_slice(&env, &[0u8]);
    env.mock_all_auths();
    let pid = client.propose_upgrade(&bad_addr, &meta);
    env.mock_all_auths(); client.approve_upgrade(&pid, &admin1);
    env.mock_all_auths(); client.approve_upgrade(&pid, &admin2);
    let result = client.try_execute_upgrade(&pid);
    assert!(result.is_err());
}

#[test]
fn test_same_implementation_validation() {
    let env = Env::default();
    let proxy_id = env.register(UpgradeableProxyContract{}, ());
    let client = UpgradeableProxyContractClient::new(&env, &proxy_id);
    
    let admin1 = Address::generate(&env);
    let admin2 = Address::generate(&env);
    let admins = Vec::from_array(&env, [admin1.clone(), admin2.clone()]);
    
    env.mock_all_auths();
    client.init(&admins, &2u32, &0u64);
    
    let impl_addr = env.register(DummyImpl{}, ());
    let meta = Bytes::from_slice(&env, &[0u8]);
    
    // First upgrade - should succeed
    env.mock_all_auths();
    let prop_id = client.propose_upgrade(&impl_addr, &meta);
    env.mock_all_auths(); client.approve_upgrade(&prop_id, &admin1);
    env.mock_all_auths(); client.approve_upgrade(&prop_id, &admin2);
    env.mock_all_auths(); client.execute_upgrade(&prop_id);
    
    // Try to propose same implementation - should fail
    env.mock_all_auths();
    let result = client.try_propose_upgrade(&impl_addr, &meta);
    assert!(result.is_err());
}

#[test]
fn test_metadata_too_large_validation() {
    let env = Env::default();
    let proxy_id = env.register(UpgradeableProxyContract{}, ());
    let client = UpgradeableProxyContractClient::new(&env, &proxy_id);
    
    let admin1 = Address::generate(&env);
    let admin2 = Address::generate(&env);
    let admins = Vec::from_array(&env, [admin1.clone(), admin2.clone()]);
    
    env.mock_all_auths();
    client.init(&admins, &2u32, &0u64);
    
    let impl_addr = env.register(DummyImpl{}, ());
    // Create metadata larger than 1KB (1024 bytes) - using a simple approach
    let large_data = [0u8; 1025]; // This creates an array of 1025 zeros
    let large_metadata = Bytes::from_slice(&env, &large_data);
    
    env.mock_all_auths();
    let result = client.try_propose_upgrade(&impl_addr, &large_metadata);
    assert!(result.is_err());
}

#[test]
fn test_rollback_functionality() {
    let env = Env::default();
    let proxy_id = env.register(UpgradeableProxyContract{}, ());
    let client = UpgradeableProxyContractClient::new(&env, &proxy_id);
    
    let admin1 = Address::generate(&env);
    let admin2 = Address::generate(&env);
    let admins = Vec::from_array(&env, [admin1.clone(), admin2.clone()]);
    
    env.mock_all_auths();
    client.init(&admins, &2u32, &0u64);
    
    // Initial state
    assert_eq!(client.get_version(), 0);
    
    let impl_addr1 = env.register(DummyImpl{}, ());
    let meta = Bytes::from_slice(&env, &[0u8]);
    
    // First upgrade
    env.mock_all_auths();
    let prop_id1 = client.propose_upgrade(&impl_addr1, &meta);
    env.mock_all_auths(); client.approve_upgrade(&prop_id1, &admin1);
    env.mock_all_auths(); client.approve_upgrade(&prop_id1, &admin2);
    env.mock_all_auths(); client.execute_upgrade(&prop_id1);
    
    assert_eq!(client.get_version(), 1);
    assert_eq!(client.get_current_implementation(), impl_addr1);
    
    // Rollback
    env.mock_all_auths();
    client.rollback();
    
    // After rollback, should be back to initial state
    assert_eq!(client.get_version(), 0);
    let result = client.try_get_current_implementation();
    assert!(result.is_err());
}

#[test]
fn test_no_rollback_available() {
    let env = Env::default();
    let proxy_id = env.register(UpgradeableProxyContract{}, ());
    let client = UpgradeableProxyContractClient::new(&env, &proxy_id);
    
    let admin1 = Address::generate(&env);
    let admin2 = Address::generate(&env);
    let admins = Vec::from_array(&env, [admin1.clone(), admin2.clone()]);
    
    env.mock_all_auths();
    client.init(&admins, &2u32, &0u64);
    
    // Try to rollback without any upgrades - should fail
    env.mock_all_auths();
    let result = client.try_rollback();
    assert!(result.is_err());
}

// Additional tests for new features
#[contract]
pub struct IncompatImpl;

#[contractimpl]
impl IncompatImpl {
    pub fn schema_version(_env: Env) -> u32 { 1 }
    pub fn proxy_compatible(_env: Env, _current: Address) -> bool { false }
}

#[test]
fn test_compatibility_check_failure() {
    let env = Env::default();
    let proxy_id = env.register(UpgradeableProxyContract{}, ());
    let client = UpgradeableProxyContractClient::new(&env, &proxy_id);
    let admin1 = Address::generate(&env);
    let admin2 = Address::generate(&env);
    let admins = Vec::from_array(&env, [admin1.clone(), admin2.clone()]);
    env.mock_all_auths();
    client.init(&admins, &2u32, &0u64);
    let incompat_addr = env.register(IncompatImpl{}, ());
    let meta = Bytes::from_slice(&env, &[0u8]);
    env.mock_all_auths();
    let pid = client.propose_upgrade(&incompat_addr, &meta);
    env.mock_all_auths(); client.approve_upgrade(&pid, &admin1);
    env.mock_all_auths(); client.approve_upgrade(&pid, &admin2);
    let result = client.try_execute_upgrade(&pid);
    assert!(result.is_err());
}

#[contract]
pub struct BadMigrateImpl;

#[contractimpl]
impl BadMigrateImpl {
    pub fn schema_version(_env: Env) -> u32 { 1 }
    pub fn proxy_compatible(_env: Env, _current: Address) -> bool { true }
    pub fn validate_migration(_env: Env, _state_hash: Bytes) -> bool { true }
    pub fn migrate(_env: Env) { panic!("Migration failed"); } // Simulate failure
    pub fn rollback_migration(_env: Env) { /* no-op */ }
}

#[test]
fn test_migration_failure_rollback() {
    let env = Env::default();
    let proxy_id = env.register(UpgradeableProxyContract{}, ());
    let client = UpgradeableProxyContractClient::new(&env, &proxy_id);
    let admin1 = Address::generate(&env);
    let admin2 = Address::generate(&env);
    let admins = Vec::from_array(&env, [admin1.clone(), admin2.clone()]);
    env.mock_all_auths();
    client.init(&admins, &2u32, &0u64);
    let bad_migrate_addr = env.register(BadMigrateImpl{}, ());
    let meta = Bytes::from_slice(&env, &[1u8]); // Flag for migration
    env.mock_all_auths();
    let pid = client.propose_upgrade(&bad_migrate_addr, &meta);
    env.mock_all_auths(); client.approve_upgrade(&pid, &admin1);
    env.mock_all_auths(); client.approve_upgrade(&pid, &admin2);
    let result = client.try_execute_upgrade(&pid);
    assert!(result.is_err());
}

#[test]
fn test_upgrade_stats() {
    let env = Env::default();
    let proxy_id = env.register(UpgradeableProxyContract{}, ());
    let client = UpgradeableProxyContractClient::new(&env, &proxy_id);
    let admin1 = Address::generate(&env);
    let admin2 = Address::generate(&env);
    let admins = Vec::from_array(&env, [admin1.clone(), admin2.clone()]);
    env.mock_all_auths();
    client.init(&admins, &2u32, &0u64);
    let impl_addr = env.register(DummyImpl{}, ());
    let meta = Bytes::from_slice(&env, &[0u8]);
    env.mock_all_auths();
    let prop_id = client.propose_upgrade(&impl_addr, &meta);
    env.mock_all_auths(); client.approve_upgrade(&prop_id, &admin1);
    env.mock_all_auths(); client.approve_upgrade(&prop_id, &admin2);
    env.mock_all_auths(); client.execute_upgrade(&prop_id);
    let stats: Map<Symbol, Val> = client.get_upgrade_stats();
    let total_upgrades: u32 = u32::try_from_val(&env, &stats.get(Symbol::new(&env, "total_upgrades")).unwrap()).unwrap();
    assert_eq!(total_upgrades, 1);
}

#[test]
fn test_upgrade_checklist() {
    let env = Env::default();
    let proxy_id = env.register(UpgradeableProxyContract{}, ());
    let client = UpgradeableProxyContractClient::new(&env, &proxy_id);
    let admin1 = Address::generate(&env);
    let admin2 = Address::generate(&env);
    let admins = Vec::from_array(&env, [admin1.clone(), admin2.clone()]);
    env.mock_all_auths();
    client.init(&admins, &2u32, &0u64);
    let impl_addr = env.register(DummyImpl{}, ());
    let meta = Bytes::from_slice(&env, &[0u8]);
    env.mock_all_auths();
    let prop_id = client.propose_upgrade(&impl_addr, &meta);
    env.mock_all_auths(); client.approve_upgrade(&prop_id, &admin1);
    env.mock_all_auths(); client.approve_upgrade(&prop_id, &admin2);
    let checklist: Vec<Symbol> = client.get_upgrade_checklist(&prop_id);
    assert!(checklist.contains(&Symbol::new(&env, "threshold_met")));
    env.mock_all_auths(); client.execute_upgrade(&prop_id);
    let updated_checklist: Vec<Symbol> = client.get_upgrade_checklist(&prop_id);
    assert!(updated_checklist.contains(&Symbol::new(&env, "executed")));
}