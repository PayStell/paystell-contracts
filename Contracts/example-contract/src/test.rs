#![cfg(test)]

use soroban_sdk::{symbol_short, vec, Env};
use crate::{HelloContract, HelloContractClient};

#[test]
fn test_hello() {
    let env = Env::default();
    let contract_id = env.register(HelloContract {}, ());
    let client = HelloContractClient::new(&env, &contract_id);

    let name = symbol_short!("Dev");
    let expected = vec![&env, symbol_short!("Hello"), symbol_short!("Dev")];
    let actual = client.hello(&name);
    assert_eq!(actual, expected);
}
