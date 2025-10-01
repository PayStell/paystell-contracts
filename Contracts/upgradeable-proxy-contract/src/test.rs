#![cfg(test)]
use soroban_sdk::{Env, Address, Vec, Bytes, symbol_short, Val, IntoVal, TryFromVal, testutils::{Address as _}};
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
    pub fn migrate(_env: Env) { /* no-op */ }
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

// Implementation without schema_version should cause validation failure (panic)
use soroban_sdk::{contract as contract2, contractimpl as contractimpl2};
#[contract2]
pub struct BadImpl;
#[contractimpl2]
impl BadImpl { pub fn alt_ping(_env: Env) -> u32 { 1 } }

#[test]
#[should_panic]
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
    env.mock_all_auths(); client.execute_upgrade(&pid); // should panic due to validation
}
