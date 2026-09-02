// SPDX-License-Identifier: Apache-2.0

use crate::build_solidity;
use soroban_sdk::{Bytes, BytesN, FromVal, IntoVal, Val};

fn bytes_eq(env: &soroban_sdk::Env, result: &Val, expected: &[u8]) -> bool {
    Bytes::from_val(env, result) == Bytes::from_slice(env, expected)
}

#[test]
fn struct_bytes_field_push_subscript_length() {
    let src = r#"
        contract c {
            struct S { bytes data; uint64 z; }
            S s;
            function push_b(bytes1 v) public { s.data.push(v); }
            function pop_b() public { s.data.pop(); }
            function set_at(uint32 i, bytes1 v) public { s.data[i] = v; }
            function get_at(uint32 i) public view returns (bytes1) { return s.data[i]; }
            function blen() public view returns (uint32) { return uint32(s.data.length); }
            function get_all() public view returns (bytes memory) { return s.data; }
            function set_z(uint64 v) public { s.z = v; }
            function get_z() public view returns (uint64) { return s.z; }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let push = |b: u8| {
        runtime.invoke_contract(
            addr,
            "push_b",
            vec![BytesN::from_array(env, &[b]).into_val(env)],
        );
    };

    runtime.invoke_contract(addr, "set_z", vec![777_u64.into_val(env)]);

    for b in [0xAAu8, 0xBB, 0xCC] {
        push(b);
    }

    let three: Val = 3_u32.into_val(env);
    assert!(three.shallow_eq(&runtime.invoke_contract(addr, "blen", vec![])));
    assert!(bytes_eq(
        env,
        &runtime.invoke_contract(addr, "get_all", vec![]),
        &[0xAA, 0xBB, 0xCC]
    ));

    for (i, &b) in [0xAAu8, 0xBB, 0xCC].iter().enumerate() {
        let got = runtime.invoke_contract(addr, "get_at", vec![(i as u32).into_val(env)]);
        assert!(bytes_eq(env, &got, &[b]), "s.data[{i}] should be {b:#04x}");
    }

    runtime.invoke_contract(
        addr,
        "set_at",
        vec![
            1_u32.into_val(env),
            BytesN::from_array(env, &[0x99]).into_val(env),
        ],
    );
    assert!(bytes_eq(
        env,
        &runtime.invoke_contract(addr, "get_all", vec![]),
        &[0xAA, 0x99, 0xCC]
    ));

    runtime.invoke_contract(addr, "pop_b", vec![]);
    let two: Val = 2_u32.into_val(env);
    assert!(two.shallow_eq(&runtime.invoke_contract(addr, "blen", vec![])));
    assert!(bytes_eq(
        env,
        &runtime.invoke_contract(addr, "get_all", vec![]),
        &[0xAA, 0x99]
    ));

    let z: Val = 777_u64.into_val(env);
    assert!(
        z.shallow_eq(&runtime.invoke_contract(addr, "get_z", vec![])),
        "sibling scalar isolated"
    );
}

#[test]
fn struct_bytes_field_whole_rw() {
    let src = r#"
        contract c {
            struct S { bytes data; uint64 z; }
            S s;
            function set_data(bytes memory d) public { s.data = d; }
            function get_data() public view returns (bytes memory) { return s.data; }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    runtime.invoke_contract(
        addr,
        "set_data",
        vec![Bytes::from_array(env, &[0x01, 0x02, 0x03, 0x04]).into_val(env)],
    );
    assert!(bytes_eq(
        env,
        &runtime.invoke_contract(addr, "get_data", vec![]),
        &[0x01, 0x02, 0x03, 0x04]
    ));

    runtime.invoke_contract(
        addr,
        "set_data",
        vec![Bytes::from_array(env, &[0xFF]).into_val(env)],
    );
    assert!(bytes_eq(
        env,
        &runtime.invoke_contract(addr, "get_data", vec![]),
        &[0xFF]
    ));
}

#[test]
fn array_of_bytes_nested_push_subscript() {
    let src = r#"
        contract c {
            bytes[] arr;
            function add_row() public { arr.push(); }
            function push_into(uint32 i, bytes1 v) public { arr[i].push(v); }
            function set_at(uint32 i, uint32 j, bytes1 v) public { arr[i][j] = v; }
            function get_at(uint32 i, uint32 j) public view returns (bytes1) { return arr[i][j]; }
            function row(uint32 i) public view returns (bytes memory) { return arr[i]; }
            function outer_len() public view returns (uint32) { return uint32(arr.length); }
            function inner_len(uint32 i) public view returns (uint32) { return uint32(arr[i].length); }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    runtime.invoke_contract(addr, "add_row", vec![]);
    runtime.invoke_contract(addr, "add_row", vec![]);

    let push_into = |i: u32, b: u8| {
        runtime.invoke_contract(
            addr,
            "push_into",
            vec![i.into_val(env), BytesN::from_array(env, &[b]).into_val(env)],
        );
    };
    push_into(0, 0x10);
    push_into(0, 0x11);
    push_into(1, 0x20);

    let two: Val = 2_u32.into_val(env);
    assert!(two.shallow_eq(&runtime.invoke_contract(addr, "outer_len", vec![])));
    assert!(two.shallow_eq(&runtime.invoke_contract(addr, "inner_len", vec![0_u32.into_val(env)])));
    let one: Val = 1_u32.into_val(env);
    assert!(one.shallow_eq(&runtime.invoke_contract(addr, "inner_len", vec![1_u32.into_val(env)])));

    assert!(bytes_eq(
        env,
        &runtime.invoke_contract(addr, "row", vec![0_u32.into_val(env)]),
        &[0x10, 0x11]
    ));
    assert!(bytes_eq(
        env,
        &runtime.invoke_contract(addr, "row", vec![1_u32.into_val(env)]),
        &[0x20]
    ));

    runtime.invoke_contract(
        addr,
        "set_at",
        vec![
            0_u32.into_val(env),
            1_u32.into_val(env),
            BytesN::from_array(env, &[0xEE]).into_val(env),
        ],
    );
    let got = runtime.invoke_contract(
        addr,
        "get_at",
        vec![0_u32.into_val(env), 1_u32.into_val(env)],
    );
    assert!(bytes_eq(env, &got, &[0xEE]));
    assert!(bytes_eq(
        env,
        &runtime.invoke_contract(addr, "row", vec![1_u32.into_val(env)]),
        &[0x20]
    ));
}

#[test]
fn struct_string_field_whole_rw_length() {
    let src = r#"
        contract c {
            struct S { string name; uint64 z; }
            S s;
            function set_name(string memory n) public { s.name = n; }
            function get_name() public view returns (string memory) { return s.name; }
            function nlen() public view returns (uint32) { return uint32(bytes(s.name).length); }
            function set_z(uint64 v) public { s.z = v; }
            function get_z() public view returns (uint64) { return s.z; }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    runtime.invoke_contract(addr, "set_z", vec![42_u64.into_val(env)]);
    runtime.invoke_contract(
        addr,
        "set_name",
        vec![soroban_sdk::String::from_str(env, "hello").into_val(env)],
    );

    let got = runtime.invoke_contract(addr, "get_name", vec![]);
    assert_eq!(
        soroban_sdk::String::from_val(env, &got),
        soroban_sdk::String::from_str(env, "hello"),
    );

    let five: Val = 5_u32.into_val(env);
    assert!(five.shallow_eq(&runtime.invoke_contract(addr, "nlen", vec![])));

    let z: Val = 42_u64.into_val(env);
    assert!(
        z.shallow_eq(&runtime.invoke_contract(addr, "get_z", vec![])),
        "sibling scalar isolated"
    );
}
