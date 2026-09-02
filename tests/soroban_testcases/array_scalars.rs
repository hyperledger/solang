// SPDX-License-Identifier: Apache-2.0

use crate::build_solidity;
use soroban_sdk::{
    testutils::Address as _, Address, IntoVal, TryFromVal, Val, Vec as SVec, I256, U256,
};

#[test]
fn u64_roundtrip_and_alias() {
    let runtime = build_solidity(
        r#"
        contract c {
            function echo(uint64[] memory a) public pure returns (uint64[] memory) {
                return a;
            }

            function squares(uint64 n) public pure returns (uint64[] memory) {
                uint64[] memory out = new uint64[](n);
                for (uint64 i = 0; i < n; i++) {
                    out[i] = i * i;
                }
                return out;
            }

            function alias_return(uint64[] memory a) public pure returns (uint64[] memory) {
                uint64[] memory b = a;
                return b;
            }

            function alias_read(uint64[] memory a) public pure returns (uint64) {
                uint64[] memory b = a;
                return b[0] + b[1] + b[2];
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let input: SVec<u64> = soroban_sdk::vec![env, 10, 20, 30, 40];

    let res = runtime.invoke_contract(addr, "echo", vec![input.clone().into_val(env)]);
    let got = SVec::<u64>::try_from_val(env, &res).unwrap();
    assert_eq!(got, input);

    let res = runtime.invoke_contract(addr, "alias_return", vec![input.clone().into_val(env)]);
    let got = SVec::<u64>::try_from_val(env, &res).unwrap();
    assert_eq!(got, input);

    let expected: Val = 60_u64.into_val(env);
    let res = runtime.invoke_contract(addr, "alias_read", vec![input.into_val(env)]);
    assert!(expected.shallow_eq(&res));

    let res = runtime.invoke_contract(addr, "squares", vec![6_u64.into_val(env)]);
    let got = SVec::<u64>::try_from_val(env, &res).unwrap();
    assert_eq!(got, soroban_sdk::vec![env, 0, 1, 4, 9, 16, 25]);
}

#[test]
fn u64_large_n_no_overflow() {
    let runtime = build_solidity(
        r#"
        contract c {
            function sum(uint64[] memory a) public pure returns (uint64) {
                uint64 s = 0;
                for (uint64 i = 0; i < a.length; i++) {
                    s += a[i];
                }
                return s;
            }

            function iota(uint64 n) public pure returns (uint64[] memory) {
                uint64[] memory out = new uint64[](n);
                for (uint64 i = 0; i < n; i++) {
                    out[i] = i;
                }
                return out;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    const N: u64 = 256;
    let mut input: SVec<u64> = soroban_sdk::vec![env];
    let mut expected_sum: u64 = 0;
    for i in 0..N {
        input.push_back(i);
        expected_sum += i;
    }

    let expected: Val = expected_sum.into_val(env);
    let res = runtime.invoke_contract(addr, "sum", vec![input.into_val(env)]);
    assert!(expected.shallow_eq(&res));

    let res = runtime.invoke_contract(addr, "iota", vec![N.into_val(env)]);
    let got = SVec::<u64>::try_from_val(env, &res).unwrap();
    assert_eq!(got.len(), N as u32);
    assert_eq!(got.get(0).unwrap(), 0);
    assert_eq!(got.get(N as u32 - 1).unwrap(), N - 1);
}

#[test]
fn u32_i32_roundtrip() {
    let runtime = build_solidity(
        r#"
        contract c {
            function echo_u32(uint32[] memory a) public pure returns (uint32[] memory) {
                return a;
            }
            function echo_i32(int32[] memory a) public pure returns (int32[] memory) {
                return a;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let u: SVec<u32> = soroban_sdk::vec![env, 0, 1, 7, 4_000_000_000];
    let res = runtime.invoke_contract(addr, "echo_u32", vec![u.clone().into_val(env)]);
    assert_eq!(SVec::<u32>::try_from_val(env, &res).unwrap(), u);

    let i: SVec<i32> = soroban_sdk::vec![env, -2_000_000_000, -1, 0, 1, 2_000_000_000];
    let res = runtime.invoke_contract(addr, "echo_i32", vec![i.clone().into_val(env)]);
    assert_eq!(SVec::<i32>::try_from_val(env, &res).unwrap(), i);
}

#[test]
fn i64_roundtrip() {
    let runtime = build_solidity(
        r#"
        contract c {
            function echo(int64[] memory a) public pure returns (int64[] memory) {
                return a;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let i: SVec<i64> = soroban_sdk::vec![env, i64::MIN, -1, 0, 1, i64::MAX];
    let res = runtime.invoke_contract(addr, "echo", vec![i.clone().into_val(env)]);
    assert_eq!(SVec::<i64>::try_from_val(env, &res).unwrap(), i);
}

#[test]
fn u128_i128_roundtrip() {
    let runtime = build_solidity(
        r#"
        contract c {
            function echo_u128(uint128[] memory a) public pure returns (uint128[] memory) {
                return a;
            }
            function echo_i128(int128[] memory a) public pure returns (int128[] memory) {
                return a;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let u: SVec<u128> = soroban_sdk::vec![env, 0, 1, u128::from(u64::MAX) + 1, u128::MAX];
    let res = runtime.invoke_contract(addr, "echo_u128", vec![u.clone().into_val(env)]);
    assert_eq!(SVec::<u128>::try_from_val(env, &res).unwrap(), u);

    let i: SVec<i128> = soroban_sdk::vec![env, i128::MIN, -1, 0, 1, i128::MAX];
    let res = runtime.invoke_contract(addr, "echo_i128", vec![i.clone().into_val(env)]);
    assert_eq!(SVec::<i128>::try_from_val(env, &res).unwrap(), i);
}

#[test]
fn u256_i256_roundtrip() {
    let runtime = build_solidity(
        r#"
        contract c {
            function echo_u256(uint256[] memory a) public pure returns (uint256[] memory) {
                return a;
            }
            function echo_i256(int256[] memory a) public pure returns (int256[] memory) {
                return a;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let u: SVec<U256> = soroban_sdk::vec![
        env,
        U256::from_u32(env, 0),
        U256::from_u32(env, 1),
        U256::from_u128(env, u128::MAX),
    ];
    let res = runtime.invoke_contract(addr, "echo_u256", vec![u.clone().into_val(env)]);
    assert_eq!(SVec::<U256>::try_from_val(env, &res).unwrap(), u);

    let i: SVec<I256> = soroban_sdk::vec![
        env,
        I256::from_i128(env, i128::MIN),
        I256::from_i32(env, -1),
        I256::from_i32(env, 0),
        I256::from_i128(env, i128::MAX),
    ];
    let res = runtime.invoke_contract(addr, "echo_i256", vec![i.clone().into_val(env)]);
    assert_eq!(SVec::<I256>::try_from_val(env, &res).unwrap(), i);
}

#[test]
fn bool_roundtrip() {
    let runtime = build_solidity(
        r#"
        contract c {
            function echo(bool[] memory a) public pure returns (bool[] memory) {
                return a;
            }

            function flip(bool[] memory a) public pure returns (bool[] memory) {
                bool[] memory out = new bool[](a.length);
                for (uint32 i = 0; i < a.length; i++) {
                    out[i] = !a[i];
                }
                return out;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let b: SVec<bool> = soroban_sdk::vec![env, true, false, false, true];
    let res = runtime.invoke_contract(addr, "echo", vec![b.clone().into_val(env)]);
    assert_eq!(SVec::<bool>::try_from_val(env, &res).unwrap(), b);

    let res = runtime.invoke_contract(addr, "flip", vec![b.into_val(env)]);
    let got = SVec::<bool>::try_from_val(env, &res).unwrap();
    assert_eq!(got, soroban_sdk::vec![env, false, true, true, false]);
}

#[test]
fn storage_u64_set_get() {
    let src = r#"
        contract c {
            uint64[] stored;

            function set(uint64[] memory a) public {
                stored = a;
            }

            function get() public view returns (uint64[] memory) {
                return stored;
            }

            function element(uint32 i) public view returns (uint64) {
                return stored[i];
            }

            function len() public view returns (uint64) {
                return uint64(stored.length);
            }

            // overwrite the whole storage array with a fresh memory array
            function reset(uint64 n) public returns (uint64[] memory) {
                uint64[] memory tmp = new uint64[](n);
                for (uint64 i = 0; i < n; i++) {
                    tmp[i] = i * 2;
                }
                stored = tmp;
                return stored;
            }
        }
    "#;

    let runtime = build_solidity(src, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let input: SVec<u64> = soroban_sdk::vec![env, 10, 20, 30, 40];
    runtime.invoke_contract(addr, "set", vec![input.clone().into_val(env)]);

    // whole storage array read back
    let res = runtime.invoke_contract(addr, "get", vec![]);
    assert_eq!(SVec::<u64>::try_from_val(env, &res).unwrap(), input);

    // element read out of storage
    let expected: Val = 30_u64.into_val(env);
    let res = runtime.invoke_contract(addr, "element", vec![2_u32.into_val(env)]);
    assert!(expected.shallow_eq(&res));

    // length of storage array
    let expected: Val = 4_u64.into_val(env);
    let res = runtime.invoke_contract(addr, "len", vec![]);
    assert!(expected.shallow_eq(&res));

    // overwrite whole storage array and read the new value back
    let res = runtime.invoke_contract(addr, "reset", vec![3_u64.into_val(env)]);
    let expected: SVec<u64> = soroban_sdk::vec![env, 0, 2, 4];
    assert_eq!(SVec::<u64>::try_from_val(env, &res).unwrap(), expected);

    let res = runtime.invoke_contract(addr, "get", vec![]);
    assert_eq!(SVec::<u64>::try_from_val(env, &res).unwrap(), expected);
}

#[test]
fn address_roundtrip() {
    let runtime = build_solidity(
        r#"
        contract c {
            function echo(address[] memory a) public pure returns (address[] memory) {
                return a;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let a: SVec<Address> = soroban_sdk::vec![
        env,
        Address::generate(env),
        Address::generate(env),
        Address::generate(env),
    ];
    let res = runtime.invoke_contract(addr, "echo", vec![a.clone().into_val(env)]);
    assert_eq!(SVec::<Address>::try_from_val(env, &res).unwrap(), a);
}
