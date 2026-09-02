// SPDX-License-Identifier: Apache-2.0

use crate::build_solidity;
use soroban_sdk::{
    contracttype, vec as svec, Address, IntoVal, String as SString, TryFromVal, Vec, I256, U256,
};

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Point {
    pub x: u32,
    pub y: u32,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Holder {
    pub xs: Vec<u32>,
    pub tag: u32,
}

#[test]
fn u32_param_return_read_construct() {
    let runtime = build_solidity(
        r#"
        contract c {
            function echo(uint32[3] memory a) public pure returns (uint32[3] memory) {
                return a;
            }
            function readsum(uint32[3] memory a) public pure returns (uint32) {
                return a[0] + a[1] + a[2];
            }
            function make() public pure returns (uint32[3] memory) {
                uint32[3] memory out;
                out[0] = 5;
                out[1] = 6;
                out[2] = 7;
                return out;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let input: Vec<u32> = svec![env, 10, 20, 30];
    let res = runtime.invoke_contract(addr, "echo", vec![input.clone().into_val(env)]);
    let got = Vec::<u32>::try_from_val(env, &res).unwrap();
    assert_eq!(got, input);

    let res = runtime.invoke_contract(addr, "readsum", vec![input.into_val(env)]);
    let got = u32::try_from_val(env, &res).unwrap();
    assert_eq!(got, 60);

    let res = runtime.invoke_contract(addr, "make", vec![]);
    let got = Vec::<u32>::try_from_val(env, &res).unwrap();
    assert_eq!(got, svec![env, 5, 6, 7]);
}

#[test]
fn wide_int_elements() {
    let runtime = build_solidity(
        r#"
        contract c {
            function echo_u128(uint128[2] memory a) public pure returns (uint128[2] memory) {
                return a;
            }
            function echo_i128(int128[2] memory a) public pure returns (int128[2] memory) {
                return a;
            }
            function echo_u256(uint256[2] memory a) public pure returns (uint256[2] memory) {
                return a;
            }
            function echo_i256(int256[2] memory a) public pure returns (int256[2] memory) {
                return a;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let u128s: Vec<u128> = svec![env, 1u128 << 100, 42u128];
    let res = runtime.invoke_contract(addr, "echo_u128", vec![u128s.clone().into_val(env)]);
    assert_eq!(Vec::<u128>::try_from_val(env, &res).unwrap(), u128s);

    let i128s: Vec<i128> = svec![env, -(1i128 << 100), 7i128];
    let res = runtime.invoke_contract(addr, "echo_i128", vec![i128s.clone().into_val(env)]);
    assert_eq!(Vec::<i128>::try_from_val(env, &res).unwrap(), i128s);

    let u256s: Vec<U256> = svec![env, U256::from_u32(env, 123), U256::from_u32(env, 456)];
    let res = runtime.invoke_contract(addr, "echo_u256", vec![u256s.clone().into_val(env)]);
    assert_eq!(Vec::<U256>::try_from_val(env, &res).unwrap(), u256s);

    let i256s: Vec<I256> = svec![env, I256::from_i32(env, -123), I256::from_i32(env, 456)];
    let res = runtime.invoke_contract(addr, "echo_i256", vec![i256s.clone().into_val(env)]);
    assert_eq!(Vec::<I256>::try_from_val(env, &res).unwrap(), i256s);
}

#[test]
fn bool_address_bytesn_elements() {
    let runtime = build_solidity(
        r#"
        contract c {
            function echo_bool(bool[3] memory a) public pure returns (bool[3] memory) {
                return a;
            }
            function echo_addr(address[2] memory a) public pure returns (address[2] memory) {
                return a;
            }
            function echo_b32(bytes32[2] memory a) public pure returns (bytes32[2] memory) {
                return a;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let bools: Vec<bool> = svec![env, true, false, true];
    let res = runtime.invoke_contract(addr, "echo_bool", vec![bools.clone().into_val(env)]);
    assert_eq!(Vec::<bool>::try_from_val(env, &res).unwrap(), bools);

    use soroban_sdk::testutils::Address as _;
    let addrs: Vec<Address> = svec![env, Address::generate(env), Address::generate(env)];
    let res = runtime.invoke_contract(addr, "echo_addr", vec![addrs.clone().into_val(env)]);
    assert_eq!(Vec::<Address>::try_from_val(env, &res).unwrap(), addrs);

    use soroban_sdk::BytesN;
    let b32s: Vec<BytesN<32>> = svec![
        env,
        BytesN::from_array(env, &[1u8; 32]),
        BytesN::from_array(env, &[2u8; 32])
    ];
    let res = runtime.invoke_contract(addr, "echo_b32", vec![b32s.clone().into_val(env)]);
    assert_eq!(Vec::<BytesN<32>>::try_from_val(env, &res).unwrap(), b32s);
}

#[test]
fn string_elements() {
    let runtime = build_solidity(
        r#"
        contract c {
            function echo(string[2] memory a) public pure returns (string[2] memory) {
                return a;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    let input: Vec<SString> = svec![
        env,
        SString::from_str(env, "hi"),
        SString::from_str(env, "there")
    ];
    let res = runtime.invoke_contract(addr, "echo", vec![input.clone().into_val(env)]);
    assert_eq!(Vec::<SString>::try_from_val(env, &res).unwrap(), input);
}

#[test]
fn struct_elements() {
    let runtime = build_solidity(
        r#"
        contract c {
            struct Point { uint32 x; uint32 y; }
            function echo(Point[2] memory a) public pure returns (Point[2] memory) {
                return a;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    let input: Vec<Point> = svec![env, Point { x: 1, y: 2 }, Point { x: 3, y: 4 }];
    let res = runtime.invoke_contract(addr, "echo", vec![input.clone().into_val(env)]);
    assert_eq!(Vec::<Point>::try_from_val(env, &res).unwrap(), input);
}

#[test]
fn static_array_struct_field() {
    let runtime = build_solidity(
        r#"
        contract c {
            struct Holder { uint32[3] xs; uint32 tag; }
            function echo(Holder memory h) public pure returns (Holder memory) {
                return h;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    let input = Holder {
        xs: svec![env, 7, 8, 9],
        tag: 42,
    };
    let res = runtime.invoke_contract(addr, "echo", vec![input.clone().into_val(env)]);
    assert_eq!(Holder::try_from_val(env, &res).unwrap(), input);
}

#[test]
fn multidim_fixed() {
    let runtime = build_solidity(
        r#"
        contract c {
            function echo(uint32[2][3] memory a) public pure returns (uint32[2][3] memory) {
                return a;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    let input: Vec<Vec<u32>> = svec![env, svec![env, 1, 2], svec![env, 3, 4], svec![env, 5, 6]];
    let res = runtime.invoke_contract(addr, "echo", vec![input.clone().into_val(env)]);
    assert_eq!(Vec::<Vec<u32>>::try_from_val(env, &res).unwrap(), input);
}

#[test]
fn fixed_of_dynamic() {
    let runtime = build_solidity(
        r#"
        contract c {
            function echo(uint32[][2] memory a) public pure returns (uint32[][2] memory) {
                return a;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    let input: Vec<Vec<u32>> = svec![env, svec![env, 1, 2, 3], svec![env, 4, 5]];
    let res = runtime.invoke_contract(addr, "echo", vec![input.clone().into_val(env)]);
    assert_eq!(Vec::<Vec<u32>>::try_from_val(env, &res).unwrap(), input);
}

#[test]
fn dynamic_of_fixed() {
    let runtime = build_solidity(
        r#"
        contract c {
            function echo(uint32[2][] memory a) public pure returns (uint32[2][] memory) {
                return a;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    let input: Vec<Vec<u32>> = svec![env, svec![env, 1, 2], svec![env, 3, 4], svec![env, 5, 6]];
    let res = runtime.invoke_contract(addr, "echo", vec![input.clone().into_val(env)]);
    assert_eq!(Vec::<Vec<u32>>::try_from_val(env, &res).unwrap(), input);
}

#[test]
fn storage_roundtrip() {
    let runtime = build_solidity(
        r#"
        contract c {
            uint32[3] stored;

            function set(uint32[3] memory a) public {
                stored = a;
            }
            function get() public view returns (uint32[3] memory) {
                return stored;
            }
            function at(uint32 i) public view returns (uint32) {
                return stored[i];
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let input: Vec<u32> = svec![env, 11, 22, 33];
    runtime.invoke_contract(addr, "set", vec![input.clone().into_val(env)]);

    let res = runtime.invoke_contract(addr, "get", vec![]);
    assert_eq!(Vec::<u32>::try_from_val(env, &res).unwrap(), input);

    let res = runtime.invoke_contract(addr, "at", vec![0u32.into_val(env)]);
    assert_eq!(u32::try_from_val(env, &res).unwrap(), 11);

    let res = runtime.invoke_contract(addr, "at", vec![1u32.into_val(env)]);
    assert_eq!(u32::try_from_val(env, &res).unwrap(), 22);

    let res = runtime.invoke_contract(addr, "at", vec![2u32.into_val(env)]);
    assert_eq!(u32::try_from_val(env, &res).unwrap(), 33);
}

#[test]
fn storage_default_zero_init() {
    let runtime = build_solidity(
        r#"
        contract c {
            uint32[3] stored;
            function get() public view returns (uint32[3] memory) { return stored; }
        }
        "#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    let res = runtime.invoke_contract(addr, "get", vec![]);
    let got = Vec::<u32>::try_from_val(env, &res).unwrap();
    assert_eq!(got, svec![env, 0, 0, 0]);
}

#[test]
fn storage_default_multidim_fixed() {
    let runtime = build_solidity(
        r#"
        contract c {
            uint32[2][3] stored;
            function get() public view returns (uint32[2][3] memory) { return stored; }
        }
        "#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    let res = runtime.invoke_contract(addr, "get", vec![]);
    let got = Vec::<Vec<u32>>::try_from_val(env, &res).unwrap();
    assert_eq!(
        got,
        svec![env, svec![env, 0, 0], svec![env, 0, 0], svec![env, 0, 0]]
    );
}

#[test]
fn storage_default_fixed_of_dynamic() {
    let runtime = build_solidity(
        r#"
        contract c {
            uint32[][2] stored;
            function get() public view returns (uint32[][2] memory) { return stored; }
        }
        "#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    let res = runtime.invoke_contract(addr, "get", vec![]);
    let got = Vec::<Vec<u32>>::try_from_val(env, &res).unwrap();
    let empty: Vec<u32> = svec![env];
    assert_eq!(got, svec![env, empty.clone(), empty]);
}

#[test]
fn storage_default_dynamic_of_fixed() {
    let runtime = build_solidity(
        r#"
        contract c {
            uint32[2][] stored;
            function get() public view returns (uint32[2][] memory) { return stored; }
        }
        "#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    let res = runtime.invoke_contract(addr, "get", vec![]);
    let got = Vec::<Vec<u32>>::try_from_val(env, &res).unwrap();
    let empty: Vec<Vec<u32>> = svec![env];
    assert_eq!(got, empty);
}
