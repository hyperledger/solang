// SPDX-License-Identifier: Apache-2.0

use crate::build_solidity;
use soroban_sdk::{IntoVal, String as SString, TryFromVal, Vec as SVec};

const CONTRACT: &str = r#"
    contract hello_world {
        function hello(string memory to) public pure returns (string[] memory) {
            string[] memory result = new string[](2);
            result[0] = "Hello";
            result[1] = to;
            return result;
        }
    }
"#;

#[test]
fn example_hello_world_greets() {
    let runtime = build_solidity(CONTRACT, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let to = SString::from_str(env, "Soroban");
    let res = runtime.invoke_contract(addr, "hello", vec![to.into_val(env)]);
    let got = SVec::<SString>::try_from_val(env, &res).unwrap();
    assert_eq!(
        got,
        soroban_sdk::vec![
            env,
            SString::from_str(env, "Hello"),
            SString::from_str(env, "Soroban"),
        ]
    );
}

#[test]
fn example_hello_world_echoes_empty_name() {
    let runtime = build_solidity(CONTRACT, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let to = SString::from_str(env, "");
    let res = runtime.invoke_contract(addr, "hello", vec![to.into_val(env)]);
    let got = SVec::<SString>::try_from_val(env, &res).unwrap();
    assert_eq!(
        got,
        soroban_sdk::vec![
            env,
            SString::from_str(env, "Hello"),
            SString::from_str(env, ""),
        ]
    );
}
