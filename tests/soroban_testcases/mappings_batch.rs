// SPDX-License-Identifier: Apache-2.0

use crate::build_solidity;
use soroban_sdk::{
    contracttype, testutils::Address as _, Address, Bytes, BytesN, Env, FromVal, IntoVal,
    String as SorobanString, TryFromVal, Val, Vec as SorobanVec, I256, U256,
};
use std::collections::HashMap;

fn model(key: &str, val: &str) -> String {
    format!(
        r#"
        contract mapping_model {{
            mapping({key} => {val}) m;

            function set({key} k, {val} v) public {{ m[k] = v; }}
            function get({key} k) public view returns ({val}) {{ return m[k]; }}
        }}
        "#
    )
}

fn model_mem(key: &str, key_ref: bool, val: &str, val_ref: bool) -> String {
    let km = if key_ref { "memory" } else { "" };
    let vm = if val_ref { "memory" } else { "" };
    format!(
        r#"
        contract mapping_model {{
            mapping({key} => {val}) m;

            function set({key} {km} k, {val} {vm} v) public {{ m[k] = v; }}
            function get({key} {km} k) public view returns ({val} {vm}) {{ return m[k]; }}
        }}
        "#
    )
}

#[test]
fn map_key_u32_val_u64() {
    let runtime = build_solidity(&model("uint32", "uint64"), |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let mut oracle: HashMap<u32, u64> = HashMap::new();

    for i in 0u32..40 {
        let k = i.wrapping_mul(2_654_435_761);
        let v = i as u64 * 1_000 + 7;
        runtime.invoke_contract(addr, "set", vec![k.into_val(env), v.into_val(env)]);
        oracle.insert(k, v);
    }

    let keys: Vec<u32> = oracle.keys().copied().collect();
    for (n, k) in keys.iter().enumerate().filter(|(n, _)| n % 3 == 0) {
        let v = 9_000_000 + n as u64;
        runtime.invoke_contract(addr, "set", vec![k.into_val(env), v.into_val(env)]);
        oracle.insert(*k, v);
    }

    for (k, v) in &oracle {
        let got = runtime.invoke_contract(addr, "get", vec![(*k).into_val(env)]);
        let exp: Val = (*v).into_val(env);
        assert!(exp.shallow_eq(&got), "m[{k}] should be {v}");
    }

    for k in [1u32, 3, 5_000_000, u32::MAX] {
        if oracle.contains_key(&k) {
            continue;
        }
        let got = runtime.invoke_contract(addr, "get", vec![k.into_val(env)]);
        let zero: Val = 0u64.into_val(env);
        assert!(zero.shallow_eq(&got), "never-set m[{k}] should be 0");
    }
}

#[test]
fn map_key_u64_val_u64() {
    let runtime = build_solidity(&model("uint64", "uint64"), |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let mut oracle: HashMap<u64, u64> = HashMap::new();

    for i in 0u64..40 {
        let k = i.wrapping_mul(11_400_714_819_323_198_485);
        let v = i * 1_000 + 7;
        runtime.invoke_contract(addr, "set", vec![k.into_val(env), v.into_val(env)]);
        oracle.insert(k, v);
    }

    let keys: Vec<u64> = oracle.keys().copied().collect();
    for (n, k) in keys.iter().enumerate().filter(|(n, _)| n % 4 == 0) {
        let v = 9_000_000_000 + n as u64;
        runtime.invoke_contract(addr, "set", vec![k.into_val(env), v.into_val(env)]);
        oracle.insert(*k, v);
    }

    for (k, v) in &oracle {
        let got = runtime.invoke_contract(addr, "get", vec![(*k).into_val(env)]);
        let exp: Val = (*v).into_val(env);
        assert!(exp.shallow_eq(&got), "m[{k}] should be {v}");
    }

    for k in [1u64, 3, 5_000_000_000, u64::MAX] {
        if oracle.contains_key(&k) {
            continue;
        }
        let got = runtime.invoke_contract(addr, "get", vec![k.into_val(env)]);
        let zero: Val = 0u64.into_val(env);
        assert!(zero.shallow_eq(&got), "never-set m[{k}] should be 0");
    }
}

#[test]
fn map_key_u128_val_u64() {
    let runtime = build_solidity(&model("uint128", "uint64"), |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let mut oracle: HashMap<u128, u64> = HashMap::new();

    for i in 0u128..40 {
        let k = if i % 2 == 0 {
            i * 3 + 1
        } else {
            2u128.pow(100) + i * 7
        };
        let v = i as u64 * 1_000 + 7;
        runtime.invoke_contract(addr, "set", vec![k.into_val(env), v.into_val(env)]);
        oracle.insert(k, v);
    }

    let keys: Vec<u128> = oracle.keys().copied().collect();
    for (n, k) in keys.iter().enumerate().filter(|(n, _)| n % 3 == 0) {
        let v = 8_000_000 + n as u64;
        runtime.invoke_contract(addr, "set", vec![k.into_val(env), v.into_val(env)]);
        oracle.insert(*k, v);
    }

    for (k, v) in &oracle {
        let got = runtime.invoke_contract(addr, "get", vec![(*k).into_val(env)]);
        let exp: Val = (*v).into_val(env);
        assert!(exp.shallow_eq(&got), "m[{k}] should be {v}");
    }

    for k in [2u128, 5, 2u128.pow(120), u128::MAX] {
        if oracle.contains_key(&k) {
            continue;
        }
        let got = runtime.invoke_contract(addr, "get", vec![k.into_val(env)]);
        let zero: Val = 0u64.into_val(env);
        assert!(zero.shallow_eq(&got), "never-set m[{k}] should be 0");
    }
}

#[test]
fn map_key_u256_val_u64() {
    let runtime = build_solidity(&model("uint256", "uint64"), |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let mut oracle: HashMap<u128, u64> = HashMap::new();

    for i in 0u128..40 {
        let seed = if i % 2 == 0 {
            i * 3 + 1
        } else {
            2u128.pow(110) + i * 7
        };
        let v = i as u64 * 1_000 + 7;
        let k = U256::from_u128(env, seed);
        runtime.invoke_contract(addr, "set", vec![k.into_val(env), v.into_val(env)]);
        oracle.insert(seed, v);
    }

    let seeds: Vec<u128> = oracle.keys().copied().collect();
    for (n, seed) in seeds.iter().enumerate().filter(|(n, _)| n % 3 == 0) {
        let v = 8_000_000 + n as u64;
        let k = U256::from_u128(env, *seed);
        runtime.invoke_contract(addr, "set", vec![k.into_val(env), v.into_val(env)]);
        oracle.insert(*seed, v);
    }

    for (seed, v) in &oracle {
        let k = U256::from_u128(env, *seed);
        let got = runtime.invoke_contract(addr, "get", vec![k.into_val(env)]);
        let exp: Val = (*v).into_val(env);
        assert!(exp.shallow_eq(&got), "m[{seed}] should be {v}");
    }

    for seed in [2u128, 5, 2u128.pow(120), u128::MAX] {
        if oracle.contains_key(&seed) {
            continue;
        }
        let k = U256::from_u128(env, seed);
        let got = runtime.invoke_contract(addr, "get", vec![k.into_val(env)]);
        let zero: Val = 0u64.into_val(env);
        assert!(zero.shallow_eq(&got), "never-set m[{seed}] should be 0");
    }
}

fn gen_addresses(env: &Env, n: usize) -> Vec<Address> {
    (0..n).map(|_| Address::generate(env)).collect()
}

#[test]
fn map_key_address_val_u32() {
    let runtime = build_solidity(&model("address", "uint32"), |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let addrs = gen_addresses(env, 24);
    let mut oracle: Vec<u32> = vec![0; addrs.len()];

    for (i, a) in addrs.iter().enumerate() {
        let v = i as u32 * 100 + 1;
        runtime.invoke_contract(addr, "set", vec![a.into_val(env), v.into_val(env)]);
        oracle[i] = v;
    }

    for i in (0..addrs.len()).step_by(4) {
        let v = 7_000_000 + i as u32;
        runtime.invoke_contract(addr, "set", vec![addrs[i].into_val(env), v.into_val(env)]);
        oracle[i] = v;
    }

    for (i, a) in addrs.iter().enumerate() {
        let got = runtime.invoke_contract(addr, "get", vec![a.into_val(env)]);
        let exp: Val = oracle[i].into_val(env);
        assert!(exp.shallow_eq(&got), "m[addr {i}] should be {}", oracle[i]);
    }

    let ghost = Address::generate(env);
    let got = runtime.invoke_contract(addr, "get", vec![ghost.into_val(env)]);
    let zero: Val = 0u32.into_val(env);
    assert!(zero.shallow_eq(&got), "never-set address should be 0");
}

#[test]
fn map_key_address_val_u64() {
    let runtime = build_solidity(&model("address", "uint64"), |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let addrs = gen_addresses(env, 24);
    let mut oracle: Vec<u64> = vec![0; addrs.len()];

    for (i, a) in addrs.iter().enumerate() {
        let v = i as u64 * 1_000_000_000 + 1;
        runtime.invoke_contract(addr, "set", vec![a.into_val(env), v.into_val(env)]);
        oracle[i] = v;
    }

    for i in (0..addrs.len()).step_by(3) {
        let v = 42_000_000_000 + i as u64;
        runtime.invoke_contract(addr, "set", vec![addrs[i].into_val(env), v.into_val(env)]);
        oracle[i] = v;
    }

    for (i, a) in addrs.iter().enumerate() {
        let got = runtime.invoke_contract(addr, "get", vec![a.into_val(env)]);
        let exp: Val = oracle[i].into_val(env);
        assert!(exp.shallow_eq(&got), "m[addr {i}] should be {}", oracle[i]);
    }

    let ghost = Address::generate(env);
    let got = runtime.invoke_contract(addr, "get", vec![ghost.into_val(env)]);
    let zero: Val = 0u64.into_val(env);
    assert!(zero.shallow_eq(&got), "never-set address should be 0");
}

#[test]
fn map_key_address_val_u128() {
    let runtime = build_solidity(&model("address", "uint128"), |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let addrs = gen_addresses(env, 24);
    let mut oracle: Vec<u128> = vec![0; addrs.len()];

    for (i, a) in addrs.iter().enumerate() {
        let v = if i % 2 == 0 {
            i as u128 * 1_000 + 1
        } else {
            2u128.pow(100) + i as u128
        };
        runtime.invoke_contract(addr, "set", vec![a.into_val(env), v.into_val(env)]);
        oracle[i] = v;
    }

    for i in (0..addrs.len()).step_by(3) {
        let v = 2u128.pow(120) + i as u128;
        runtime.invoke_contract(addr, "set", vec![addrs[i].into_val(env), v.into_val(env)]);
        oracle[i] = v;
    }

    for (i, a) in addrs.iter().enumerate() {
        let got = runtime.invoke_contract(addr, "get", vec![a.into_val(env)]);
        let got = u128::try_from_val(env, &got).expect("decode u128");
        assert_eq!(got, oracle[i], "m[addr {i}] mismatch");
    }

    let ghost = Address::generate(env);
    let got = runtime.invoke_contract(addr, "get", vec![ghost.into_val(env)]);
    assert_eq!(
        u128::try_from_val(env, &got).expect("decode u128"),
        0,
        "never-set address should be 0"
    );
}

#[test]
fn map_key_address_val_u256() {
    let runtime = build_solidity(&model("address", "uint256"), |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let addrs = gen_addresses(env, 24);
    let mut oracle: Vec<u128> = vec![0; addrs.len()];

    for (i, a) in addrs.iter().enumerate() {
        let seed = if i % 2 == 0 {
            i as u128 * 1_000 + 1
        } else {
            2u128.pow(110) + i as u128
        };
        let v = U256::from_u128(env, seed);
        runtime.invoke_contract(addr, "set", vec![a.into_val(env), v.into_val(env)]);
        oracle[i] = seed;
    }

    for i in (0..addrs.len()).step_by(3) {
        let seed = 2u128.pow(120) + i as u128;
        let v = U256::from_u128(env, seed);
        runtime.invoke_contract(addr, "set", vec![addrs[i].into_val(env), v.into_val(env)]);
        oracle[i] = seed;
    }

    for (i, a) in addrs.iter().enumerate() {
        let got = runtime.invoke_contract(addr, "get", vec![a.into_val(env)]);
        let exp = U256::from_u128(env, oracle[i]);
        assert!(U256::from_val(env, &got) == exp, "m[addr {i}] mismatch");
    }

    let ghost = Address::generate(env);
    let got = runtime.invoke_contract(addr, "get", vec![ghost.into_val(env)]);
    assert!(
        U256::from_val(env, &got) == U256::from_u128(env, 0),
        "never-set address should be 0"
    );
}

#[test]
fn map_key_i32_val_u64() {
    let runtime = build_solidity(&model("int32", "uint64"), |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let mut oracle: HashMap<i32, u64> = HashMap::new();

    for i in 0i32..40 {
        let k = i - 20;
        let v = (i as u64) * 1_000 + 7;
        runtime.invoke_contract(addr, "set", vec![k.into_val(env), v.into_val(env)]);
        oracle.insert(k, v);
    }

    let keys: Vec<i32> = oracle.keys().copied().collect();
    for (n, k) in keys.iter().enumerate().filter(|(n, _)| n % 3 == 0) {
        let v = 9_000_000 + n as u64;
        runtime.invoke_contract(addr, "set", vec![k.into_val(env), v.into_val(env)]);
        oracle.insert(*k, v);
    }

    for (k, v) in &oracle {
        let got = runtime.invoke_contract(addr, "get", vec![(*k).into_val(env)]);
        let exp: Val = (*v).into_val(env);
        assert!(exp.shallow_eq(&got), "m[{k}] should be {v}");
    }

    for k in [100i32, -100, i32::MIN, i32::MAX] {
        if oracle.contains_key(&k) {
            continue;
        }
        let got = runtime.invoke_contract(addr, "get", vec![k.into_val(env)]);
        let zero: Val = 0u64.into_val(env);
        assert!(zero.shallow_eq(&got), "never-set m[{k}] should be 0");
    }
}

#[test]
fn map_key_i64_val_u64() {
    let runtime = build_solidity(&model("int64", "uint64"), |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let mut oracle: HashMap<i64, u64> = HashMap::new();

    for i in 0i64..40 {
        let k = if i % 2 == 0 {
            i - 20
        } else {
            -(1_000_000_000_000) + i
        };
        let v = (i as u64) * 1_000 + 7;
        runtime.invoke_contract(addr, "set", vec![k.into_val(env), v.into_val(env)]);
        oracle.insert(k, v);
    }

    let keys: Vec<i64> = oracle.keys().copied().collect();
    for (n, k) in keys.iter().enumerate().filter(|(n, _)| n % 4 == 0) {
        let v = 9_000_000_000 + n as u64;
        runtime.invoke_contract(addr, "set", vec![k.into_val(env), v.into_val(env)]);
        oracle.insert(*k, v);
    }

    for (k, v) in &oracle {
        let got = runtime.invoke_contract(addr, "get", vec![(*k).into_val(env)]);
        let exp: Val = (*v).into_val(env);
        assert!(exp.shallow_eq(&got), "m[{k}] should be {v}");
    }

    for k in [7i64, -7, i64::MIN, i64::MAX] {
        if oracle.contains_key(&k) {
            continue;
        }
        let got = runtime.invoke_contract(addr, "get", vec![k.into_val(env)]);
        let zero: Val = 0u64.into_val(env);
        assert!(zero.shallow_eq(&got), "never-set m[{k}] should be 0");
    }
}

#[test]
fn map_key_i128_val_u64() {
    let runtime = build_solidity(&model("int128", "uint64"), |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let mut oracle: HashMap<i128, u64> = HashMap::new();

    for i in 0i128..40 {
        let k = if i % 2 == 0 {
            i - 20
        } else {
            -(2i128.pow(100)) + i
        };
        let v = (i as u64) * 1_000 + 7;
        runtime.invoke_contract(addr, "set", vec![k.into_val(env), v.into_val(env)]);
        oracle.insert(k, v);
    }

    let keys: Vec<i128> = oracle.keys().copied().collect();
    for (n, k) in keys.iter().enumerate().filter(|(n, _)| n % 3 == 0) {
        let v = 8_000_000 + n as u64;
        runtime.invoke_contract(addr, "set", vec![k.into_val(env), v.into_val(env)]);
        oracle.insert(*k, v);
    }

    for (k, v) in &oracle {
        let got = runtime.invoke_contract(addr, "get", vec![(*k).into_val(env)]);
        let exp: Val = (*v).into_val(env);
        assert!(exp.shallow_eq(&got), "m[{k}] should be {v}");
    }

    for k in [5i128, -5, 2i128.pow(120), -(2i128.pow(120))] {
        if oracle.contains_key(&k) {
            continue;
        }
        let got = runtime.invoke_contract(addr, "get", vec![k.into_val(env)]);
        let zero: Val = 0u64.into_val(env);
        assert!(zero.shallow_eq(&got), "never-set m[{k}] should be 0");
    }
}

#[test]
fn map_key_i256_val_u64() {
    let runtime = build_solidity(&model("int256", "uint64"), |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let mut oracle: HashMap<i128, u64> = HashMap::new();

    for i in 0i128..40 {
        let seed = if i % 2 == 0 {
            i - 20
        } else {
            -(2i128.pow(110)) + i
        };
        let v = (i as u64) * 1_000 + 7;
        let k = I256::from_i128(env, seed);
        runtime.invoke_contract(addr, "set", vec![k.into_val(env), v.into_val(env)]);
        oracle.insert(seed, v);
    }

    let seeds: Vec<i128> = oracle.keys().copied().collect();
    for (n, seed) in seeds.iter().enumerate().filter(|(n, _)| n % 3 == 0) {
        let v = 8_000_000 + n as u64;
        let k = I256::from_i128(env, *seed);
        runtime.invoke_contract(addr, "set", vec![k.into_val(env), v.into_val(env)]);
        oracle.insert(*seed, v);
    }

    for (seed, v) in &oracle {
        let k = I256::from_i128(env, *seed);
        let got = runtime.invoke_contract(addr, "get", vec![k.into_val(env)]);
        let exp: Val = (*v).into_val(env);
        assert!(exp.shallow_eq(&got), "m[{seed}] should be {v}");
    }

    for seed in [5i128, -5, 2i128.pow(120), -(2i128.pow(120))] {
        if oracle.contains_key(&seed) {
            continue;
        }
        let k = I256::from_i128(env, seed);
        let got = runtime.invoke_contract(addr, "get", vec![k.into_val(env)]);
        let zero: Val = 0u64.into_val(env);
        assert!(zero.shallow_eq(&got), "never-set m[{seed}] should be 0");
    }
}

#[test]
fn map_key_address_val_i32() {
    let runtime = build_solidity(&model("address", "int32"), |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let addrs = gen_addresses(env, 24);
    let mut oracle: Vec<i32> = vec![0; addrs.len()];

    for (i, a) in addrs.iter().enumerate() {
        let v = (i as i32 * 100 + 1) * if i % 2 == 0 { 1 } else { -1 };
        runtime.invoke_contract(addr, "set", vec![a.into_val(env), v.into_val(env)]);
        oracle[i] = v;
    }

    for i in (0..addrs.len()).step_by(4) {
        let v = -7_000_000 - i as i32;
        runtime.invoke_contract(addr, "set", vec![addrs[i].into_val(env), v.into_val(env)]);
        oracle[i] = v;
    }

    for (i, a) in addrs.iter().enumerate() {
        let got = runtime.invoke_contract(addr, "get", vec![a.into_val(env)]);
        let exp: Val = oracle[i].into_val(env);
        assert!(exp.shallow_eq(&got), "m[addr {i}] should be {}", oracle[i]);
    }

    let ghost = Address::generate(env);
    let got = runtime.invoke_contract(addr, "get", vec![ghost.into_val(env)]);
    let zero: Val = 0i32.into_val(env);
    assert!(zero.shallow_eq(&got), "never-set address should be 0");
}

#[test]
fn map_key_address_val_i64() {
    let runtime = build_solidity(&model("address", "int64"), |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let addrs = gen_addresses(env, 24);
    let mut oracle: Vec<i64> = vec![0; addrs.len()];

    for (i, a) in addrs.iter().enumerate() {
        let v = (i as i64 * 1_000_000_000 + 1) * if i % 2 == 0 { 1 } else { -1 };
        runtime.invoke_contract(addr, "set", vec![a.into_val(env), v.into_val(env)]);
        oracle[i] = v;
    }

    for i in (0..addrs.len()).step_by(3) {
        let v = -42_000_000_000 - i as i64;
        runtime.invoke_contract(addr, "set", vec![addrs[i].into_val(env), v.into_val(env)]);
        oracle[i] = v;
    }

    for (i, a) in addrs.iter().enumerate() {
        let got = runtime.invoke_contract(addr, "get", vec![a.into_val(env)]);
        let exp: Val = oracle[i].into_val(env);
        assert!(exp.shallow_eq(&got), "m[addr {i}] should be {}", oracle[i]);
    }

    let ghost = Address::generate(env);
    let got = runtime.invoke_contract(addr, "get", vec![ghost.into_val(env)]);
    let zero: Val = 0i64.into_val(env);
    assert!(zero.shallow_eq(&got), "never-set address should be 0");
}

#[test]
fn map_key_address_val_i128() {
    let runtime = build_solidity(&model("address", "int128"), |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let addrs = gen_addresses(env, 24);
    let mut oracle: Vec<i128> = vec![0; addrs.len()];

    for (i, a) in addrs.iter().enumerate() {
        let v = if i % 2 == 0 {
            i as i128 * 1_000 + 1
        } else {
            -(2i128.pow(100)) - i as i128
        };
        runtime.invoke_contract(addr, "set", vec![a.into_val(env), v.into_val(env)]);
        oracle[i] = v;
    }

    for i in (0..addrs.len()).step_by(3) {
        let v = 2i128.pow(120) - i as i128;
        runtime.invoke_contract(addr, "set", vec![addrs[i].into_val(env), v.into_val(env)]);
        oracle[i] = v;
    }

    for (i, a) in addrs.iter().enumerate() {
        let got = runtime.invoke_contract(addr, "get", vec![a.into_val(env)]);
        let got = i128::try_from_val(env, &got).expect("decode i128");
        assert_eq!(got, oracle[i], "m[addr {i}] mismatch");
    }

    let ghost = Address::generate(env);
    let got = runtime.invoke_contract(addr, "get", vec![ghost.into_val(env)]);
    assert_eq!(
        i128::try_from_val(env, &got).expect("decode i128"),
        0,
        "never-set address should be 0"
    );
}

#[test]
fn map_key_address_val_i256() {
    let runtime = build_solidity(&model("address", "int256"), |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let addrs = gen_addresses(env, 24);
    let mut oracle: Vec<i128> = vec![0; addrs.len()];
    for (i, a) in addrs.iter().enumerate() {
        let seed = if i % 2 == 0 {
            i as i128 * 1_000 + 1
        } else {
            -(2i128.pow(110)) - i as i128
        };
        let v = I256::from_i128(env, seed);
        runtime.invoke_contract(addr, "set", vec![a.into_val(env), v.into_val(env)]);
        oracle[i] = seed;
    }

    for i in (0..addrs.len()).step_by(3) {
        let seed = -(2i128.pow(120)) - i as i128;
        let v = I256::from_i128(env, seed);
        runtime.invoke_contract(addr, "set", vec![addrs[i].into_val(env), v.into_val(env)]);
        oracle[i] = seed;
    }

    for (i, a) in addrs.iter().enumerate() {
        let got = runtime.invoke_contract(addr, "get", vec![a.into_val(env)]);
        let exp = I256::from_i128(env, oracle[i]);
        assert!(I256::from_val(env, &got) == exp, "m[addr {i}] mismatch");
    }

    let ghost = Address::generate(env);
    let got = runtime.invoke_contract(addr, "get", vec![ghost.into_val(env)]);
    assert!(
        I256::from_val(env, &got) == I256::from_i128(env, 0),
        "never-set address should be 0"
    );
}

#[test]
fn map_delete_scalar() {
    let src = r#"
        contract c {
            mapping(uint64 => uint64) m;
            function set(uint64 k, uint64 v) public { m[k] = v; }
            function get(uint64 k) public view returns (uint64) { return m[k]; }
            function del(uint64 k) public { delete m[k]; }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let mut oracle: HashMap<u64, u64> = HashMap::new();
    for i in 0u64..20 {
        let (k, v) = (i * 3 + 1, i * 100 + 7);
        runtime.invoke_contract(addr, "set", vec![k.into_val(env), v.into_val(env)]);
        oracle.insert(k, v);
    }

    let keys: Vec<u64> = oracle.keys().copied().collect();
    for (n, k) in keys.iter().enumerate().filter(|(n, _)| n % 2 == 0) {
        runtime.invoke_contract(addr, "del", vec![k.into_val(env)]);
        oracle.remove(k);
        let _ = n;
    }

    for k in &keys {
        let got = runtime.invoke_contract(addr, "get", vec![k.into_val(env)]);
        let exp: Val = oracle.get(k).copied().unwrap_or(0).into_val(env);
        assert!(exp.shallow_eq(&got), "m[{k}] after delete");
    }

    let revive = keys[0];
    runtime.invoke_contract(
        addr,
        "set",
        vec![revive.into_val(env), 999u64.into_val(env)],
    );
    let got = runtime.invoke_contract(addr, "get", vec![revive.into_val(env)]);
    let exp: Val = 999u64.into_val(env);
    assert!(exp.shallow_eq(&got), "re-set after delete");
}

#[test]
fn map_delete_nested() {
    let src = r#"
        contract c {
            mapping(uint64 => mapping(uint64 => uint64)) m;
            function set(uint64 a, uint64 b, uint64 v) public { m[a][b] = v; }
            function get(uint64 a, uint64 b) public view returns (uint64) { return m[a][b]; }
            function del(uint64 a, uint64 b) public { delete m[a][b]; }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let set = |a: u64, b: u64, v: u64| {
        runtime.invoke_contract(
            addr,
            "set",
            vec![a.into_val(env), b.into_val(env), v.into_val(env)],
        );
    };
    let get = |a: u64, b: u64| {
        runtime.invoke_contract(addr, "get", vec![a.into_val(env), b.into_val(env)])
    };

    set(1, 10, 100);
    set(1, 20, 200);
    set(2, 10, 300);

    runtime.invoke_contract(addr, "del", vec![1u64.into_val(env), 10u64.into_val(env)]);

    let zero: Val = 0u64.into_val(env);
    let e200: Val = 200u64.into_val(env);
    let e300: Val = 300u64.into_val(env);
    assert!(zero.shallow_eq(&get(1, 10)), "deleted leaf -> 0");
    assert!(e200.shallow_eq(&get(1, 20)), "sibling leaf intact");
    assert!(e300.shallow_eq(&get(2, 10)), "other outer key intact");
}

#[test]
fn map_delete_absent_key() {
    let src = r#"
        contract c {
            mapping(uint64 => uint64) m;
            mapping(uint64 => mapping(uint64 => uint64)) n;
            function set(uint64 k, uint64 v) public { m[k] = v; }
            function get(uint64 k) public view returns (uint64) { return m[k]; }
            function del(uint64 k) public { delete m[k]; }
            function del_nested(uint64 a, uint64 b) public { delete n[a][b]; }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let zero: Val = 0u64.into_val(env);

    runtime.invoke_contract(addr, "del", vec![7u64.into_val(env)]);
    let got = runtime.invoke_contract(addr, "get", vec![7u64.into_val(env)]);
    assert!(zero.shallow_eq(&got), "absent key still 0 after delete");

    runtime.invoke_contract(addr, "set", vec![7u64.into_val(env), 42u64.into_val(env)]);
    runtime.invoke_contract(addr, "del", vec![8u64.into_val(env)]);
    let got = runtime.invoke_contract(addr, "get", vec![7u64.into_val(env)]);
    let e42: Val = 42u64.into_val(env);
    assert!(e42.shallow_eq(&got), "neighbour intact after absent delete");

    runtime.invoke_contract(
        addr,
        "del_nested",
        vec![99u64.into_val(env), 1u64.into_val(env)],
    );
}

#[test]
fn map_nested_uint_keys() {
    let src = r#"
        contract c {
            mapping(uint64 => mapping(uint64 => uint64)) m;
            function set(uint64 a, uint64 b, uint64 v) public { m[a][b] = v; }
            function get(uint64 a, uint64 b) public view returns (uint64) { return m[a][b]; }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let mut oracle: HashMap<(u64, u64), u64> = HashMap::new();
    for a in 0u64..6 {
        for b in 0u64..6 {
            let v = a * 1000 + b + 1;
            runtime.invoke_contract(
                addr,
                "set",
                vec![a.into_val(env), b.into_val(env), v.into_val(env)],
            );
            oracle.insert((a, b), v);
        }
    }

    for (&(a, b), &v) in &oracle {
        let got = runtime.invoke_contract(addr, "get", vec![a.into_val(env), b.into_val(env)]);
        let exp: Val = v.into_val(env);
        assert!(exp.shallow_eq(&got), "m[{a}][{b}] should be {v}");
    }

    let zero: Val = 0u64.into_val(env);
    let got = runtime.invoke_contract(addr, "get", vec![1u64.into_val(env), 99u64.into_val(env)]);
    assert!(zero.shallow_eq(&got), "absent inner key -> 0");
    let got = runtime.invoke_contract(addr, "get", vec![99u64.into_val(env), 1u64.into_val(env)]);
    assert!(zero.shallow_eq(&got), "absent outer key -> 0");
}

#[test]
fn map_nested_three_levels() {
    let src = r#"
        contract c {
            mapping(address => mapping(address => mapping(uint64 => uint64))) m;
            function set(address a, address b, uint64 c, uint64 v) public { m[a][b][c] = v; }
            function get(address a, address b, uint64 c) public view returns (uint64) { return m[a][b][c]; }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let a = Address::generate(env);
    let b = Address::generate(env);
    let other = Address::generate(env);

    runtime.invoke_contract(
        addr,
        "set",
        vec![
            a.into_val(env),
            b.into_val(env),
            7u64.into_val(env),
            4242u64.into_val(env),
        ],
    );

    let e4242: Val = 4242u64.into_val(env);
    let got = runtime.invoke_contract(
        addr,
        "get",
        vec![a.into_val(env), b.into_val(env), 7u64.into_val(env)],
    );
    assert!(e4242.shallow_eq(&got), "3-level round-trip");

    let zero: Val = 0u64.into_val(env);
    let got = runtime.invoke_contract(
        addr,
        "get",
        vec![a.into_val(env), other.into_val(env), 7u64.into_val(env)],
    );
    assert!(zero.shallow_eq(&got), "absent middle level -> 0");
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct MapS {
    pub a: u64,
    pub b: i32,
    pub c: bool,
}

#[test]
fn map_value_struct() {
    let src = r#"
        contract c {
            struct S { uint64 a; int32 b; bool c; }
            mapping(address => S) m;
            function set(address k, S memory v) public { m[k] = v; }
            function get(address k) public view returns (S memory) { return m[k]; }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let k1 = Address::generate(env);
    let k2 = Address::generate(env);
    let s1 = MapS {
        a: 1_000_000_000_000,
        b: -5,
        c: true,
    };
    let s2 = MapS {
        a: 42,
        b: 7,
        c: false,
    };

    runtime.invoke_contract(
        addr,
        "set",
        vec![k1.into_val(env), s1.clone().into_val(env)],
    );
    runtime.invoke_contract(
        addr,
        "set",
        vec![k2.into_val(env), s2.clone().into_val(env)],
    );

    let g1 = runtime.invoke_contract(addr, "get", vec![k1.into_val(env)]);
    let g2 = runtime.invoke_contract(addr, "get", vec![k2.into_val(env)]);
    assert_eq!(MapS::from_val(env, &g1), s1, "struct value k1");
    assert_eq!(MapS::from_val(env, &g2), s2, "struct value k2");

    let ghost = Address::generate(env);
    let gg = runtime.invoke_contract(addr, "get", vec![ghost.into_val(env)]);
    assert_eq!(
        MapS::from_val(env, &gg),
        MapS {
            a: 0,
            b: 0,
            c: false
        },
        "default struct value"
    );
}

#[test]
fn map_value_dynarray() {
    let src = r#"
        contract c {
            mapping(address => uint64[]) m;
            function set(address k, uint64[] memory v) public { m[k] = v; }
            function get(address k) public view returns (uint64[] memory) { return m[k]; }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let k1 = Address::generate(env);
    let k2 = Address::generate(env);
    let v1 = SorobanVec::from_array(env, [10u64, 20, 30]);
    let v2 = SorobanVec::from_array(env, [7u64]);

    runtime.invoke_contract(
        addr,
        "set",
        vec![k1.into_val(env), v1.clone().into_val(env)],
    );
    runtime.invoke_contract(
        addr,
        "set",
        vec![k2.into_val(env), v2.clone().into_val(env)],
    );

    let g1 = runtime.invoke_contract(addr, "get", vec![k1.into_val(env)]);
    let g2 = runtime.invoke_contract(addr, "get", vec![k2.into_val(env)]);
    assert!(
        SorobanVec::<u64>::from_val(env, &g1) == v1,
        "array value k1"
    );
    assert!(
        SorobanVec::<u64>::from_val(env, &g2) == v2,
        "array value k2"
    );

    let ghost = Address::generate(env);
    let gg = runtime.invoke_contract(addr, "get", vec![ghost.into_val(env)]);
    assert!(
        SorobanVec::<u64>::from_val(env, &gg) == SorobanVec::<u64>::new(env),
        "default array value is empty"
    );
}

#[test]
fn map_in_struct_member() {
    let src = r#"
        contract c {
            struct Box { uint64 tag; mapping(uint64 => uint64) inner; }
            Box b;
            function setTag(uint64 t) public { b.tag = t; }
            function getTag() public view returns (uint64) { return b.tag; }
            function set(uint64 k, uint64 v) public { b.inner[k] = v; }
            function get(uint64 k) public view returns (uint64) { return b.inner[k]; }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    runtime.invoke_contract(addr, "setTag", vec![55u64.into_val(env)]);
    for i in 0u64..8 {
        runtime.invoke_contract(
            addr,
            "set",
            vec![(i * 2).into_val(env), (i * 100 + 1).into_val(env)],
        );
    }

    for i in 0u64..8 {
        let got = runtime.invoke_contract(addr, "get", vec![(i * 2).into_val(env)]);
        let exp: Val = (i * 100 + 1).into_val(env);
        assert!(exp.shallow_eq(&got), "b.inner[{}] mismatch", i * 2);
    }

    let tag = runtime.invoke_contract(addr, "getTag", vec![]);
    let e55: Val = 55u64.into_val(env);
    assert!(
        e55.shallow_eq(&tag),
        "b.tag intact alongside mapping member"
    );
    let zero: Val = 0u64.into_val(env);
    let got = runtime.invoke_contract(addr, "get", vec![999u64.into_val(env)]);
    assert!(zero.shallow_eq(&got), "absent inner key -> 0");
}

#[test]
fn map_key_bool_val_u64() {
    let runtime = build_solidity(&model("bool", "uint64"), |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let get = |k: bool| runtime.invoke_contract(addr, "get", vec![k.into_val(env)]);
    let zero: Val = 0u64.into_val(env);
    assert!(zero.shallow_eq(&get(true)), "unset true -> 0");
    assert!(zero.shallow_eq(&get(false)), "unset false -> 0");

    runtime.invoke_contract(addr, "set", vec![true.into_val(env), 111u64.into_val(env)]);
    runtime.invoke_contract(addr, "set", vec![false.into_val(env), 222u64.into_val(env)]);
    let e111: Val = 111u64.into_val(env);
    let e222: Val = 222u64.into_val(env);
    assert!(e111.shallow_eq(&get(true)), "m[true]");
    assert!(e222.shallow_eq(&get(false)), "m[false]");

    runtime.invoke_contract(addr, "set", vec![true.into_val(env), 333u64.into_val(env)]);
    let e333: Val = 333u64.into_val(env);
    assert!(e333.shallow_eq(&get(true)), "m[true] overwritten");
    assert!(e222.shallow_eq(&get(false)), "m[false] intact");
}

#[test]
fn map_key_address_val_bool() {
    let runtime = build_solidity(&model("address", "bool"), |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let addrs = gen_addresses(env, 16);
    let mut oracle: Vec<bool> = vec![false; addrs.len()];
    for (i, a) in addrs.iter().enumerate() {
        let v = i % 3 == 0;
        runtime.invoke_contract(addr, "set", vec![a.into_val(env), v.into_val(env)]);
        oracle[i] = v;
    }
    for i in (0..addrs.len()).step_by(2) {
        let v = i % 2 == 0;
        runtime.invoke_contract(addr, "set", vec![addrs[i].into_val(env), v.into_val(env)]);
        oracle[i] = v;
    }
    for (i, a) in addrs.iter().enumerate() {
        let got = runtime.invoke_contract(addr, "get", vec![a.into_val(env)]);
        let exp: Val = oracle[i].into_val(env);
        assert!(exp.shallow_eq(&got), "m[addr {i}] should be {}", oracle[i]);
    }
    let ghost = Address::generate(env);
    let got = runtime.invoke_contract(addr, "get", vec![ghost.into_val(env)]);
    let f: Val = false.into_val(env);
    assert!(f.shallow_eq(&got), "never-set bool -> false");
}

#[test]
fn map_key_address_val_address() {
    let runtime = build_solidity(&model("address", "address"), |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let keys = gen_addresses(env, 12);
    let mut vals = gen_addresses(env, 12);
    for (i, k) in keys.iter().enumerate() {
        runtime.invoke_contract(addr, "set", vec![k.into_val(env), vals[i].into_val(env)]);
    }
    for i in (0..keys.len()).step_by(2) {
        let nv = Address::generate(env);
        runtime.invoke_contract(addr, "set", vec![keys[i].into_val(env), nv.into_val(env)]);
        vals[i] = nv;
    }
    for (i, k) in keys.iter().enumerate() {
        let got = runtime.invoke_contract(addr, "get", vec![k.into_val(env)]);
        assert!(Address::from_val(env, &got) == vals[i], "m[addr {i}] value");
    }
}

fn bytesn_arr<const N: usize>(seed: u8) -> [u8; N] {
    let mut a = [0u8; N];
    a[0] = seed;
    if N > 1 {
        a[N - 1] = seed.wrapping_mul(7).wrapping_add(1);
    }
    a
}

fn bytesn_key_test<const N: usize>(sol_ty: &str) {
    let runtime = build_solidity(&model(sol_ty, "uint64"), |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let mut oracle: HashMap<[u8; N], u64> = HashMap::new();
    for i in 0u8..16 {
        let arr = bytesn_arr::<N>(i);
        let v = i as u64 * 100 + 7;
        let k = BytesN::from_array(env, &arr);
        runtime.invoke_contract(addr, "set", vec![k.into_val(env), v.into_val(env)]);
        oracle.insert(arr, v);
    }
    let arrs: Vec<[u8; N]> = oracle.keys().copied().collect();
    for (n, arr) in arrs.iter().enumerate().filter(|(n, _)| n % 3 == 0) {
        let v = 900_000 + n as u64;
        let k = BytesN::from_array(env, arr);
        runtime.invoke_contract(addr, "set", vec![k.into_val(env), v.into_val(env)]);
        oracle.insert(*arr, v);
    }
    for (arr, v) in &oracle {
        let k = BytesN::from_array(env, arr);
        let got = runtime.invoke_contract(addr, "get", vec![k.into_val(env)]);
        let exp: Val = (*v).into_val(env);
        assert!(exp.shallow_eq(&got), "{sol_ty} key round-trip");
    }
    let ghost = BytesN::from_array(env, &bytesn_arr::<N>(200));
    let got = runtime.invoke_contract(addr, "get", vec![ghost.into_val(env)]);
    let zero: Val = 0u64.into_val(env);
    assert!(zero.shallow_eq(&got), "never-set {sol_ty} key -> 0");
}

fn bytesn_val_test<const N: usize>(sol_ty: &str) {
    let runtime = build_solidity(&model("address", sol_ty), |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let addrs = gen_addresses(env, 16);
    let mut oracle: Vec<[u8; N]> = vec![[0u8; N]; addrs.len()];
    for (i, a) in addrs.iter().enumerate() {
        let arr = bytesn_arr::<N>(i as u8 + 1);
        let v = BytesN::from_array(env, &arr);
        runtime.invoke_contract(addr, "set", vec![a.into_val(env), v.into_val(env)]);
        oracle[i] = arr;
    }
    for i in (0..addrs.len()).step_by(3) {
        let arr = bytesn_arr::<N>(i as u8 + 100);
        let v = BytesN::from_array(env, &arr);
        runtime.invoke_contract(addr, "set", vec![addrs[i].into_val(env), v.into_val(env)]);
        oracle[i] = arr;
    }
    for (i, a) in addrs.iter().enumerate() {
        let got = runtime.invoke_contract(addr, "get", vec![a.into_val(env)]);
        assert!(
            BytesN::<N>::from_val(env, &got) == BytesN::from_array(env, &oracle[i]),
            "{sol_ty} value at addr {i}"
        );
    }
    let ghost = Address::generate(env);
    let got = runtime.invoke_contract(addr, "get", vec![ghost.into_val(env)]);
    assert!(
        BytesN::<N>::from_val(env, &got) == BytesN::from_array(env, &[0u8; N]),
        "never-set {sol_ty} -> zero"
    );
}

#[test]
fn map_key_bytes1_val_u64() {
    bytesn_key_test::<1>("bytes1");
}
#[test]
fn map_key_bytes4_val_u64() {
    bytesn_key_test::<4>("bytes4");
}
#[test]
fn map_key_bytes16_val_u64() {
    bytesn_key_test::<16>("bytes16");
}
#[test]
fn map_key_bytes32_val_u64() {
    bytesn_key_test::<32>("bytes32");
}

#[test]
fn map_key_address_val_bytes1() {
    bytesn_val_test::<1>("bytes1");
}
#[test]
fn map_key_address_val_bytes4() {
    bytesn_val_test::<4>("bytes4");
}
#[test]
fn map_key_address_val_bytes16() {
    bytesn_val_test::<16>("bytes16");
}
#[test]
fn map_key_address_val_bytes32() {
    bytesn_val_test::<32>("bytes32");
}

#[test]
fn map_key_bytes_val_u64() {
    let runtime = build_solidity(&model_mem("bytes", true, "uint64", false), |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let raw: [&[u8]; 5] = [
        &[0xAA, 0xBB],
        &[],
        &[1, 2, 3, 4, 5],
        &[0xFF],
        &[0x10, 0x20, 0x30],
    ];
    let mut oracle: Vec<(Bytes, u64)> = Vec::new();
    for (i, b) in raw.iter().enumerate() {
        let k = Bytes::from_slice(env, b);
        let v = i as u64 * 1000 + 7;
        runtime.invoke_contract(addr, "set", vec![k.clone().into_val(env), v.into_val(env)]);
        oracle.push((k, v));
    }
    runtime.invoke_contract(
        addr,
        "set",
        vec![oracle[0].0.clone().into_val(env), 55555u64.into_val(env)],
    );
    oracle[0].1 = 55555;

    for (k, v) in &oracle {
        let got = runtime.invoke_contract(addr, "get", vec![k.clone().into_val(env)]);
        let exp: Val = (*v).into_val(env);
        assert!(exp.shallow_eq(&got), "bytes key round-trip");
    }
    let ghost = Bytes::from_slice(env, &[0x99, 0x88, 0x77]);
    let got = runtime.invoke_contract(addr, "get", vec![ghost.into_val(env)]);
    let zero: Val = 0u64.into_val(env);
    assert!(zero.shallow_eq(&got), "never-set bytes key -> 0");
}

#[test]
fn map_key_string_val_u64() {
    let runtime = build_solidity(&model_mem("string", true, "uint64", false), |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let strs = ["hello", "", "solang world", "x", "soroban"];
    let mut oracle: Vec<(SorobanString, u64)> = Vec::new();
    for (i, s) in strs.iter().enumerate() {
        let k = SorobanString::from_str(env, s);
        let v = i as u64 * 1000 + 7;
        runtime.invoke_contract(addr, "set", vec![k.clone().into_val(env), v.into_val(env)]);
        oracle.push((k, v));
    }
    runtime.invoke_contract(
        addr,
        "set",
        vec![oracle[2].0.clone().into_val(env), 42424u64.into_val(env)],
    );
    oracle[2].1 = 42424;

    for (k, v) in &oracle {
        let got = runtime.invoke_contract(addr, "get", vec![k.clone().into_val(env)]);
        let exp: Val = (*v).into_val(env);
        assert!(exp.shallow_eq(&got), "string key round-trip");
    }
    let ghost = SorobanString::from_str(env, "never-set");
    let got = runtime.invoke_contract(addr, "get", vec![ghost.into_val(env)]);
    let zero: Val = 0u64.into_val(env);
    assert!(zero.shallow_eq(&got), "never-set string key -> 0");
}

#[test]
fn map_key_address_val_bytes() {
    let runtime = build_solidity(&model_mem("address", false, "bytes", true), |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let addrs = gen_addresses(env, 6);
    let raw: [&[u8]; 6] = [
        &[0xAA, 0xBB],
        &[],
        &[1, 2, 3, 4, 5, 6],
        &[0xFF],
        &[0x10, 0x20],
        &[0xDE, 0xAD, 0xBE, 0xEF],
    ];
    let mut oracle: Vec<Bytes> = Vec::new();
    for (i, a) in addrs.iter().enumerate() {
        let v = Bytes::from_slice(env, raw[i]);
        runtime.invoke_contract(addr, "set", vec![a.into_val(env), v.clone().into_val(env)]);
        oracle.push(v);
    }
    let nv = Bytes::from_slice(env, &[0x01, 0x02, 0x03]);
    runtime.invoke_contract(
        addr,
        "set",
        vec![addrs[0].into_val(env), nv.clone().into_val(env)],
    );
    oracle[0] = nv;

    for (i, a) in addrs.iter().enumerate() {
        let got = runtime.invoke_contract(addr, "get", vec![a.into_val(env)]);
        assert!(
            Bytes::from_val(env, &got) == oracle[i],
            "bytes value at addr {i}"
        );
    }
    let ghost = Address::generate(env);
    let got = runtime.invoke_contract(addr, "get", vec![ghost.into_val(env)]);
    assert!(
        Bytes::from_val(env, &got) == Bytes::new(env),
        "never-set bytes value -> empty"
    );
}

#[test]
fn map_key_address_val_string() {
    let runtime = build_solidity(&model_mem("address", false, "string", true), |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let addrs = gen_addresses(env, 6);
    let strs = ["hello", "", "solang world", "x", "soroban", "last"];
    let mut oracle: Vec<SorobanString> = Vec::new();
    for (i, a) in addrs.iter().enumerate() {
        let v = SorobanString::from_str(env, strs[i]);
        runtime.invoke_contract(addr, "set", vec![a.into_val(env), v.clone().into_val(env)]);
        oracle.push(v);
    }
    let nv = SorobanString::from_str(env, "replaced");
    runtime.invoke_contract(
        addr,
        "set",
        vec![addrs[1].into_val(env), nv.clone().into_val(env)],
    );
    oracle[1] = nv;

    for (i, a) in addrs.iter().enumerate() {
        let got = runtime.invoke_contract(addr, "get", vec![a.into_val(env)]);
        assert!(
            SorobanString::from_val(env, &got) == oracle[i],
            "string value at addr {i}"
        );
    }
    let ghost = Address::generate(env);
    let got = runtime.invoke_contract(addr, "get", vec![ghost.into_val(env)]);
    assert!(
        SorobanString::from_val(env, &got) == SorobanString::from_str(env, ""),
        "never-set string value -> empty"
    );
}

#[test]
fn map_key_enum_val_u64() {
    let src = r#"
        contract c {
            enum E { A, B, C, D }
            mapping(E => uint64) m;
            function set(E k, uint64 v) public { m[k] = v; }
            function get(E k) public view returns (uint64) { return m[k]; }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    for variant in 0u32..3 {
        let v = variant as u64 * 100 + 7;
        runtime.invoke_contract(addr, "set", vec![variant.into_val(env), v.into_val(env)]);
    }
    for variant in 0u32..3 {
        let got = runtime.invoke_contract(addr, "get", vec![variant.into_val(env)]);
        let exp: Val = (variant as u64 * 100 + 7).into_val(env);
        assert!(exp.shallow_eq(&got), "m[E({variant})]");
    }
    runtime.invoke_contract(addr, "set", vec![1u32.into_val(env), 9999u64.into_val(env)]);
    let got = runtime.invoke_contract(addr, "get", vec![1u32.into_val(env)]);
    let e9999: Val = 9999u64.into_val(env);
    assert!(e9999.shallow_eq(&got), "m[E(1)] overwritten");
    let got = runtime.invoke_contract(addr, "get", vec![3u32.into_val(env)]);
    let zero: Val = 0u64.into_val(env);
    assert!(zero.shallow_eq(&got), "unset m[E(3)] -> 0");
}

#[test]
fn map_key_address_val_enum() {
    let src = r#"
        contract c {
            enum E { A, B, C, D }
            mapping(address => E) m;
            function set(address k, E v) public { m[k] = v; }
            function get(address k) public view returns (E) { return m[k]; }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let addrs = gen_addresses(env, 12);
    let mut oracle: Vec<u32> = vec![0; addrs.len()];
    for (i, a) in addrs.iter().enumerate() {
        let v = (i % 4) as u32;
        runtime.invoke_contract(addr, "set", vec![a.into_val(env), v.into_val(env)]);
        oracle[i] = v;
    }
    for (i, a) in addrs.iter().enumerate() {
        let got = runtime.invoke_contract(addr, "get", vec![a.into_val(env)]);
        let exp: Val = oracle[i].into_val(env);
        assert!(exp.shallow_eq(&got), "enum value at addr {i}");
    }
    let ghost = Address::generate(env);
    let got = runtime.invoke_contract(addr, "get", vec![ghost.into_val(env)]);
    let zero: Val = 0u32.into_val(env);
    assert!(zero.shallow_eq(&got), "never-set enum -> 0");
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct MapPoint {
    pub x: i64,
    pub y: i64,
}

#[test]
fn map_value_struct_point_members() {
    let src = r#"
        contract c {
            struct Point { int64 x; int64 y; }
            mapping(address => Point) m;

            function set(address k, Point memory v) public { m[k] = v; }
            function get(address k) public view returns (Point memory) { return m[k]; }
            function setX(address k, int64 x) public { m[k].x = x; }
            function setY(address k, int64 y) public { m[k].y = y; }
            function getX(address k) public view returns (int64) { return m[k].x; }
            function getY(address k) public view returns (int64) { return m[k].y; }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let k1 = Address::generate(env);
    let k2 = Address::generate(env);

    let get = |k: &Address| {
        MapPoint::from_val(
            env,
            &runtime.invoke_contract(addr, "get", vec![k.into_val(env)]),
        )
    };
    let get_x = |k: &Address| runtime.invoke_contract(addr, "getX", vec![k.into_val(env)]);
    let get_y = |k: &Address| runtime.invoke_contract(addr, "getY", vec![k.into_val(env)]);

    let p1 = MapPoint { x: 10, y: -20 };
    let p2 = MapPoint { x: -3, y: 4 };
    runtime.invoke_contract(
        addr,
        "set",
        vec![k1.into_val(env), p1.clone().into_val(env)],
    );
    runtime.invoke_contract(
        addr,
        "set",
        vec![k2.into_val(env), p2.clone().into_val(env)],
    );
    assert_eq!(get(&k1), p1, "whole-struct get k1");
    assert_eq!(get(&k2), p2, "whole-struct get k2");

    let e10: Val = 10i64.into_val(env);
    let em20: Val = (-20i64).into_val(env);
    assert!(e10.shallow_eq(&get_x(&k1)), "k1.x read");
    assert!(em20.shallow_eq(&get_y(&k1)), "k1.y read");

    runtime.invoke_contract(addr, "setX", vec![k1.into_val(env), 999i64.into_val(env)]);
    runtime.invoke_contract(
        addr,
        "setY",
        vec![k1.into_val(env), (-999i64).into_val(env)],
    );

    let e999: Val = 999i64.into_val(env);
    let em999: Val = (-999i64).into_val(env);
    assert!(e999.shallow_eq(&get_x(&k1)), "k1.x after member write");
    assert!(em999.shallow_eq(&get_y(&k1)), "k1.y after member write");

    assert_eq!(
        get(&k1),
        MapPoint { x: 999, y: -999 },
        "k1 whole-struct after member writes"
    );
    assert_eq!(get(&k2), p2, "k2 unaffected by k1 member writes");

    let k3 = Address::generate(env);
    runtime.invoke_contract(addr, "setY", vec![k3.into_val(env), 7i64.into_val(env)]);
    assert_eq!(
        get(&k3),
        MapPoint { x: 0, y: 7 },
        "member write on fresh key"
    );

    let ghost = Address::generate(env);
    assert_eq!(
        get(&ghost),
        MapPoint { x: 0, y: 0 },
        "never-set struct is zeroed"
    );
    let zero: Val = 0i64.into_val(env);
    assert!(zero.shallow_eq(&get_x(&ghost)), "never-set member -> 0");
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct MapDoc {
    pub name: SorobanString,
    pub data: Bytes,
    pub tag: BytesN<4>,
    pub id: u32,
}

#[test]
fn map_value_struct_ref() {
    let src = r#"
        contract c {
            struct D { string name; bytes data; bytes4 tag; uint32 id; }
            mapping(address => D) m;
            function set(address k, D memory v) public { m[k] = v; }
            function get(address k) public view returns (D memory) { return m[k]; }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let mk = |name: &str, data: &[u8], tag: [u8; 4], id: u32| MapDoc {
        name: SorobanString::from_str(env, name),
        data: Bytes::from_slice(env, data),
        tag: BytesN::from_array(env, &tag),
        id,
    };

    let k1 = Address::generate(env);
    let k2 = Address::generate(env);
    let d1 = mk("hello", &[0xAA, 0xBB], [1, 2, 3, 4], 7);
    let d2 = mk("", &[], [0, 0, 0, 0], 0);

    runtime.invoke_contract(
        addr,
        "set",
        vec![k1.into_val(env), d1.clone().into_val(env)],
    );
    runtime.invoke_contract(
        addr,
        "set",
        vec![k2.into_val(env), d2.clone().into_val(env)],
    );
    assert_eq!(
        MapDoc::from_val(
            env,
            &runtime.invoke_contract(addr, "get", vec![k1.into_val(env)])
        ),
        d1
    );
    assert_eq!(
        MapDoc::from_val(
            env,
            &runtime.invoke_contract(addr, "get", vec![k2.into_val(env)])
        ),
        d2
    );

    let d1b = mk("replaced", &[0x99], [0xDE, 0xAD, 0xBE, 0xEF], 123);
    runtime.invoke_contract(
        addr,
        "set",
        vec![k1.into_val(env), d1b.clone().into_val(env)],
    );
    assert_eq!(
        MapDoc::from_val(
            env,
            &runtime.invoke_contract(addr, "get", vec![k1.into_val(env)])
        ),
        d1b
    );

    let ghost = Address::generate(env);
    assert_eq!(
        MapDoc::from_val(
            env,
            &runtime.invoke_contract(addr, "get", vec![ghost.into_val(env)])
        ),
        mk("", &[], [0, 0, 0, 0], 0),
        "never-set ref struct is zeroed"
    );
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct MapInner {
    pub x: i64,
    pub y: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct MapOuter {
    pub inner: MapInner,
    pub tag: u32,
}

#[test]
fn map_value_struct_nested() {
    let src = r#"
        contract c {
            struct Inner { int64 x; uint64 y; }
            struct Outer { Inner inner; uint32 tag; }
            mapping(address => Outer) m;
            function set(address k, Outer memory v) public { m[k] = v; }
            function get(address k) public view returns (Outer memory) { return m[k]; }
            function getX(address k) public view returns (int64) { return m[k].inner.x; }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let k1 = Address::generate(env);
    let k2 = Address::generate(env);
    let o1 = MapOuter {
        inner: MapInner { x: -7, y: 8 },
        tag: 100,
    };
    let o2 = MapOuter {
        inner: MapInner {
            x: 123456789,
            y: u64::MAX,
        },
        tag: 0,
    };

    runtime.invoke_contract(
        addr,
        "set",
        vec![k1.into_val(env), o1.clone().into_val(env)],
    );
    runtime.invoke_contract(
        addr,
        "set",
        vec![k2.into_val(env), o2.clone().into_val(env)],
    );
    assert_eq!(
        MapOuter::from_val(
            env,
            &runtime.invoke_contract(addr, "get", vec![k1.into_val(env)])
        ),
        o1
    );
    assert_eq!(
        MapOuter::from_val(
            env,
            &runtime.invoke_contract(addr, "get", vec![k2.into_val(env)])
        ),
        o2
    );

    let ex: Val = (-7i64).into_val(env);
    assert!(
        ex.shallow_eq(&runtime.invoke_contract(addr, "getX", vec![k1.into_val(env)])),
        "m[k1].inner.x"
    );

    let ghost = Address::generate(env);
    assert_eq!(
        MapOuter::from_val(
            env,
            &runtime.invoke_contract(addr, "get", vec![ghost.into_val(env)])
        ),
        MapOuter {
            inner: MapInner { x: 0, y: 0 },
            tag: 0
        },
        "never-set nested struct is zeroed"
    );
}

#[test]
fn map_value_fixed_array() {
    let src = r#"
        contract c {
            mapping(address => uint64[3]) m;
            function set(address k, uint64[3] memory v) public { m[k] = v; }
            function get(address k) public view returns (uint64[3] memory) { return m[k]; }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let addrs = gen_addresses(env, 8);
    let mut oracle: Vec<[u64; 3]> = vec![[0; 3]; addrs.len()];
    for (i, a) in addrs.iter().enumerate() {
        let arr = [i as u64 * 10 + 1, i as u64 * 10 + 2, i as u64 * 10 + 3];
        let v = SorobanVec::from_array(env, arr);
        runtime.invoke_contract(addr, "set", vec![a.into_val(env), v.into_val(env)]);
        oracle[i] = arr;
    }
    for i in (0..addrs.len()).step_by(3) {
        let arr = [9_000 + i as u64, 8_000, 7_000];
        let v = SorobanVec::from_array(env, arr);
        runtime.invoke_contract(addr, "set", vec![addrs[i].into_val(env), v.into_val(env)]);
        oracle[i] = arr;
    }
    for (i, a) in addrs.iter().enumerate() {
        let got = runtime.invoke_contract(addr, "get", vec![a.into_val(env)]);
        let exp = SorobanVec::from_array(env, oracle[i]);
        assert!(
            SorobanVec::<u64>::from_val(env, &got) == exp,
            "fixed array at addr {i}"
        );
    }
    let ghost = Address::generate(env);
    let got = runtime.invoke_contract(addr, "get", vec![ghost.into_val(env)]);
    assert!(
        SorobanVec::<u64>::from_val(env, &got) == SorobanVec::from_array(env, [0u64, 0, 0]),
        "never-set fixed array is zeroed"
    );
}

#[test]
fn map_value_string_array() {
    let src = r#"
        contract c {
            mapping(address => string[]) m;
            function set(address k, string[] memory v) public { m[k] = v; }
            function get(address k) public view returns (string[] memory) { return m[k]; }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let k1 = Address::generate(env);
    let k2 = Address::generate(env);

    let mut v1 = SorobanVec::<SorobanString>::new(env);
    for s in ["alpha", "", "gamma"] {
        v1.push_back(SorobanString::from_str(env, s));
    }
    let mut v2 = SorobanVec::<SorobanString>::new(env);
    v2.push_back(SorobanString::from_str(env, "solo"));

    runtime.invoke_contract(
        addr,
        "set",
        vec![k1.into_val(env), v1.clone().into_val(env)],
    );
    runtime.invoke_contract(
        addr,
        "set",
        vec![k2.into_val(env), v2.clone().into_val(env)],
    );
    assert!(
        SorobanVec::<SorobanString>::from_val(
            env,
            &runtime.invoke_contract(addr, "get", vec![k1.into_val(env)])
        ) == v1,
        "string[] value k1"
    );
    assert!(
        SorobanVec::<SorobanString>::from_val(
            env,
            &runtime.invoke_contract(addr, "get", vec![k2.into_val(env)])
        ) == v2,
        "string[] value k2"
    );

    let ghost = Address::generate(env);
    let got = runtime.invoke_contract(addr, "get", vec![ghost.into_val(env)]);
    assert!(
        SorobanVec::<SorobanString>::from_val(env, &got) == SorobanVec::<SorobanString>::new(env),
        "never-set string[] is empty"
    );
}

#[test]
fn map_cross_tx_persistence() {
    let src = r#"
        contract c {
            mapping(address => uint64) m;
            function set(address k, uint64 v) public { m[k] = v; }
            function get(address k) public view returns (uint64) { return m[k]; }
            function inc(address k) public { m[k] = m[k] + 1; }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let a = Address::generate(env);
    let b = Address::generate(env);

    runtime.invoke_contract(addr, "set", vec![a.into_val(env), 100u64.into_val(env)]);
    let e100: Val = 100u64.into_val(env);
    assert!(
        e100.shallow_eq(&runtime.invoke_contract(addr, "get", vec![a.into_val(env)])),
        "a persists to tx2"
    );

    runtime.invoke_contract(addr, "set", vec![b.into_val(env), 200u64.into_val(env)]);
    assert!(
        e100.shallow_eq(&runtime.invoke_contract(addr, "get", vec![a.into_val(env)])),
        "a unaffected by b write"
    );

    runtime.invoke_contract(addr, "inc", vec![a.into_val(env)]);
    runtime.invoke_contract(addr, "inc", vec![a.into_val(env)]);
    let e102: Val = 102u64.into_val(env);
    assert!(
        e102.shallow_eq(&runtime.invoke_contract(addr, "get", vec![a.into_val(env)])),
        "a incremented across txs"
    );

    let e200: Val = 200u64.into_val(env);
    assert!(
        e200.shallow_eq(&runtime.invoke_contract(addr, "get", vec![b.into_val(env)])),
        "b persists independently"
    );
}

#[test]
fn delete_array_element() {
    let src = r#"
        contract c {
            uint64[] a;
            function push(uint64 v) public { a.push(v); }
            function get(uint64 i) public view returns (uint64) { return a[i]; }
            function len() public view returns (uint64) { return uint64(a.length); }
            function del(uint64 i) public { delete a[i]; }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    for v in [10u64, 20, 30, 40] {
        runtime.invoke_contract(addr, "push", vec![v.into_val(env)]);
    }

    runtime.invoke_contract(addr, "del", vec![1u64.into_val(env)]);

    let get = |i: u64| runtime.invoke_contract(addr, "get", vec![i.into_val(env)]);
    let z: Val = 0u64.into_val(env);
    let e10: Val = 10u64.into_val(env);
    let e30: Val = 30u64.into_val(env);
    let e40: Val = 40u64.into_val(env);
    assert!(e10.shallow_eq(&get(0)), "a[0] intact");
    assert!(z.shallow_eq(&get(1)), "a[1] reset to default");
    assert!(e30.shallow_eq(&get(2)), "a[2] intact");
    assert!(e40.shallow_eq(&get(3)), "a[3] intact");

    let four: Val = 4u64.into_val(env);
    assert!(
        four.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])),
        "length unchanged by element delete"
    );
}

#[test]
fn delete_struct_field() {
    let src = r#"
        contract c {
            struct S { uint64 x; uint64 y; }
            S s;
            function set(uint64 x, uint64 y) public { s.x = x; s.y = y; }
            function getx() public view returns (uint64) { return s.x; }
            function gety() public view returns (uint64) { return s.y; }
            function delx() public { delete s.x; }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    runtime.invoke_contract(addr, "set", vec![7u64.into_val(env), 9u64.into_val(env)]);
    runtime.invoke_contract(addr, "delx", vec![]);

    let z: Val = 0u64.into_val(env);
    let e9: Val = 9u64.into_val(env);
    assert!(
        z.shallow_eq(&runtime.invoke_contract(addr, "getx", vec![])),
        "s.x reset to default"
    );
    assert!(
        e9.shallow_eq(&runtime.invoke_contract(addr, "gety", vec![])),
        "s.y intact"
    );
}

#[test]
fn delete_mapping_value_struct_field() {
    let src = r#"
        contract c {
            struct S { uint64 x; uint64 y; }
            mapping(uint64 => S) m;
            function set(uint64 k, uint64 x, uint64 y) public { m[k].x = x; m[k].y = y; }
            function getx(uint64 k) public view returns (uint64) { return m[k].x; }
            function gety(uint64 k) public view returns (uint64) { return m[k].y; }
            function delx(uint64 k) public { delete m[k].x; }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let env = &runtime.env;
    let addr = runtime.contracts.last().unwrap();

    let set = |k: u64, x: u64, y: u64| {
        runtime.invoke_contract(
            addr,
            "set",
            vec![k.into_val(env), x.into_val(env), y.into_val(env)],
        );
    };
    set(1, 11, 12);
    set(2, 21, 22);

    runtime.invoke_contract(addr, "delx", vec![1u64.into_val(env)]);

    let getx = |k: u64| runtime.invoke_contract(addr, "getx", vec![k.into_val(env)]);
    let gety = |k: u64| runtime.invoke_contract(addr, "gety", vec![k.into_val(env)]);
    let z: Val = 0u64.into_val(env);
    let e12: Val = 12u64.into_val(env);
    let e21: Val = 21u64.into_val(env);
    let e22: Val = 22u64.into_val(env);
    assert!(z.shallow_eq(&getx(1)), "m[1].x reset to default");
    assert!(e12.shallow_eq(&gety(1)), "m[1].y intact");
    assert!(e21.shallow_eq(&getx(2)), "m[2].x intact");
    assert!(e22.shallow_eq(&gety(2)), "m[2].y intact");
}
