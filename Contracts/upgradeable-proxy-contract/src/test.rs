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
    let meta = Bytes::from_slice(&env, b"dummy v1");
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
