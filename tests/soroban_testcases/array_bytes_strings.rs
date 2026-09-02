// SPDX-License-Identifier: Apache-2.0

use crate::build_solidity;
use soroban_sdk::{Bytes, BytesN, IntoVal, TryFromVal, Val, Vec as SVec};

#[test]
fn string_roundtrip_and_alias() {
    let runtime = build_solidity(
        r#"
        contract c {
            function echo(string[] memory a) public pure returns (string[] memory) {
                return a;
            }

            function alias_return(string[] memory a) public pure returns (string[] memory) {
                string[] memory b = a;
                return b;
            }

            function build() public pure returns (string[] memory) {
                string[] memory out = new string[](3);
                out[0] = "hello";
                out[1] = "";
                out[2] = "soroban world";
                return out;
            }

            function first_len(string[] memory a) public pure returns (uint64) {
                return uint64(bytes(a[0]).length);
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let input: SVec<soroban_sdk::String> = soroban_sdk::vec![
        env,
        soroban_sdk::String::from_str(env, "alpha"),
        soroban_sdk::String::from_str(env, ""),
        soroban_sdk::String::from_str(env, "a longer string element"),
    ];

    let res = runtime.invoke_contract(addr, "echo", vec![input.clone().into_val(env)]);
    assert_eq!(
        SVec::<soroban_sdk::String>::try_from_val(env, &res).unwrap(),
        input
    );

    let res = runtime.invoke_contract(addr, "alias_return", vec![input.clone().into_val(env)]);
    assert_eq!(
        SVec::<soroban_sdk::String>::try_from_val(env, &res).unwrap(),
        input
    );

    let res = runtime.invoke_contract(addr, "first_len", vec![input.into_val(env)]);
    let expected: soroban_sdk::Val = 5_u64.into_val(env);
    assert!(expected.shallow_eq(&res));

    let res = runtime.invoke_contract(addr, "build", vec![]);
    let got = SVec::<soroban_sdk::String>::try_from_val(env, &res).unwrap();
    assert_eq!(
        got,
        soroban_sdk::vec![
            env,
            soroban_sdk::String::from_str(env, "hello"),
            soroban_sdk::String::from_str(env, ""),
            soroban_sdk::String::from_str(env, "soroban world"),
        ]
    );
}

#[test]
fn bytes_roundtrip() {
    let runtime = build_solidity(
        r#"
        contract c {
            function echo(bytes[] memory a) public pure returns (bytes[] memory) {
                return a;
            }

            function total_len(bytes[] memory a) public pure returns (uint64) {
                uint64 s = 0;
                for (uint32 i = 0; i < a.length; i++) {
                    s += uint64(a[i].length);
                }
                return s;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let input: SVec<Bytes> = soroban_sdk::vec![
        env,
        Bytes::from_slice(env, &[]),
        Bytes::from_slice(env, &[0x01]),
        Bytes::from_slice(env, &[0xde, 0xad, 0xbe, 0xef]),
    ];

    let res = runtime.invoke_contract(addr, "echo", vec![input.clone().into_val(env)]);
    assert_eq!(SVec::<Bytes>::try_from_val(env, &res).unwrap(), input);

    let res = runtime.invoke_contract(addr, "total_len", vec![input.into_val(env)]);
    let expected: soroban_sdk::Val = 5_u64.into_val(env);
    assert!(expected.shallow_eq(&res));
}

#[test]
fn storage_string_set_get() {
    let src = r#"
        contract c {
            string[] stored;

            function set(string[] memory a) public {
                stored = a;
            }

            function get() public view returns (string[] memory) {
                return stored;
            }

            function first_len() public view returns (uint64) {
                return uint64(bytes(stored[0]).length);
            }

            // overwrite the whole storage array from literals
            function reset() public returns (string[] memory) {
                string[] memory tmp = new string[](2);
                tmp[0] = "fresh";
                tmp[1] = "value";
                stored = tmp;
                return stored;
            }
        }
    "#;

    let runtime = build_solidity(src, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let input: SVec<soroban_sdk::String> = soroban_sdk::vec![
        env,
        soroban_sdk::String::from_str(env, "alpha"),
        soroban_sdk::String::from_str(env, ""),
        soroban_sdk::String::from_str(env, "a longer string element"),
    ];

    runtime.invoke_contract(addr, "set", vec![input.clone().into_val(env)]);

    let res = runtime.invoke_contract(addr, "get", vec![]);
    assert_eq!(
        SVec::<soroban_sdk::String>::try_from_val(env, &res).unwrap(),
        input
    );

    let expected: Val = 5_u64.into_val(env);
    let res = runtime.invoke_contract(addr, "first_len", vec![]);
    assert!(expected.shallow_eq(&res));

    let res = runtime.invoke_contract(addr, "reset", vec![]);
    let expected: SVec<soroban_sdk::String> = soroban_sdk::vec![
        env,
        soroban_sdk::String::from_str(env, "fresh"),
        soroban_sdk::String::from_str(env, "value"),
    ];
    assert_eq!(
        SVec::<soroban_sdk::String>::try_from_val(env, &res).unwrap(),
        expected
    );

    let res = runtime.invoke_contract(addr, "get", vec![]);
    assert_eq!(
        SVec::<soroban_sdk::String>::try_from_val(env, &res).unwrap(),
        expected
    );
}

#[test]
fn bytes4_roundtrip() {
    let runtime = build_solidity(
        r#"
        contract c {
            function echo(bytes4[] memory a) public pure returns (bytes4[] memory) {
                return a;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let input: SVec<BytesN<4>> = soroban_sdk::vec![
        env,
        BytesN::from_array(env, &[0x00, 0x00, 0x00, 0x00]),
        BytesN::from_array(env, &[0x01, 0x02, 0x03, 0x04]),
        BytesN::from_array(env, &[0xff, 0xff, 0xff, 0xff]),
    ];

    let res = runtime.invoke_contract(addr, "echo", vec![input.clone().into_val(env)]);
    assert_eq!(SVec::<BytesN<4>>::try_from_val(env, &res).unwrap(), input);
}
