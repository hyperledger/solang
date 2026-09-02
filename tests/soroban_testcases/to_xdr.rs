// SPDX-License-Identifier: Apache-2.0

use crate::build_solidity;
use soroban_sdk::{
    contracttype, testutils::Address as _, xdr::ToXdr, Address, Bytes, FromVal, IntoVal,
    Vec as SVec,
};

#[contracttype]
#[derive(Clone)]
pub struct Receiver {
    pub index: u32,
    pub recipient: Address,
    pub amount: i128,
}

#[test]
fn to_xdr_uint32_matches_sdk() {
    let runtime = build_solidity(
        r#"contract T {
            function f(uint32 x) public pure returns (bytes) {
                return to_xdr(x);
            }
        }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    for v in [0u32, 1, 42, u32::MAX] {
        let res = runtime.invoke_contract(addr, "f", vec![v.into_val(&runtime.env)]);
        let got = Bytes::from_val(&runtime.env, &res);
        let expected = v.to_xdr(&runtime.env);
        assert_eq!(got, expected, "to_xdr(uint32) mismatch for {v}");
    }
}

#[test]
fn to_xdr_int128_matches_sdk() {
    let runtime = build_solidity(
        r#"contract T {
            function f(int128 x) public pure returns (bytes) {
                return to_xdr(x);
            }
        }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    for v in [0i128, 1, -1, 500, -12345, i128::MAX, i128::MIN] {
        let res = runtime.invoke_contract(addr, "f", vec![v.into_val(&runtime.env)]);
        let got = Bytes::from_val(&runtime.env, &res);
        let expected = v.to_xdr(&runtime.env);
        assert_eq!(got, expected, "to_xdr(int128) mismatch for {v}");
    }
}

#[test]
fn to_xdr_address_matches_sdk() {
    let runtime = build_solidity(
        r#"contract T {
            function f(address x) public pure returns (bytes) {
                return to_xdr(x);
            }
        }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let account = Address::generate(&runtime.env);
    let res = runtime.invoke_contract(addr, "f", vec![account.clone().into_val(&runtime.env)]);
    let got = Bytes::from_val(&runtime.env, &res);
    let expected = account.to_xdr(&runtime.env);
    assert_eq!(got, expected, "to_xdr(address) mismatch");
}

#[test]
fn to_xdr_struct_matches_sdk() {
    let runtime = build_solidity(
        r#"contract T {
            struct Receiver {
                uint32 index;
                address recipient;
                int128 amount;
            }
            function f(uint32 index, address recipient, int128 amount)
                public pure returns (bytes)
            {
                Receiver memory node = Receiver(index, recipient, amount);
                return to_xdr(node);
            }
        }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    let recipient = Address::generate(env);
    let res = runtime.invoke_contract(
        addr,
        "f",
        vec![
            7u32.into_val(env),
            recipient.clone().into_val(env),
            500i128.into_val(env),
        ],
    );
    let got = Bytes::from_val(env, &res);
    let expected = Receiver {
        index: 7,
        recipient: recipient.clone(),
        amount: 500,
    }
    .to_xdr(env);
    assert_eq!(got, expected, "to_xdr(struct) mismatch");
}

#[test]
fn to_xdr_uint32_array_matches_sdk() {
    let runtime = build_solidity(
        r#"contract T {
            function f(uint32[] memory a) public pure returns (bytes) {
                return to_xdr(a);
            }
        }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    let inputs: [SVec<u32>; 3] = [
        soroban_sdk::vec![env, 1u32, 2, 3, 42],
        SVec::<u32>::new(env),
        soroban_sdk::vec![env, u32::MAX],
    ];
    for input in inputs {
        let res = runtime.invoke_contract(addr, "f", vec![input.clone().into_val(env)]);
        let got = Bytes::from_val(env, &res);
        let expected = input.clone().to_xdr(env);
        assert_eq!(got, expected, "to_xdr(uint32[]) mismatch for {input:?}");
    }
}

#[test]
fn to_xdr_storage_value_matches_sdk() {
    let runtime = build_solidity(
        r#"contract T {
            uint32 stored;
            function set(uint32 x) public {
                stored = x;
            }
            function f() public view returns (bytes) {
                return to_xdr(stored);
            }
        }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    for v in [0u32, 1, 42, u32::MAX] {
        runtime.invoke_contract(addr, "set", vec![v.into_val(env)]);
        let res = runtime.invoke_contract(addr, "f", vec![]);
        let got = Bytes::from_val(env, &res);
        let expected = v.to_xdr(env);
        assert_eq!(got, expected, "to_xdr(storage uint32) mismatch for {v}");
    }
}

#[test]
fn to_xdr_int128_array_matches_sdk() {
    let runtime = build_solidity(
        r#"contract T {
            function f(int128[] memory a) public pure returns (bytes) {
                return to_xdr(a);
            }
        }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    let input: SVec<i128> = soroban_sdk::vec![env, 1i128, -2, 500, -12345, i128::MAX, i128::MIN];
    let res = runtime.invoke_contract(addr, "f", vec![input.clone().into_val(env)]);
    let got = Bytes::from_val(env, &res);
    let expected = input.to_xdr(env);
    assert_eq!(got, expected, "to_xdr(int128[]) mismatch");
}
