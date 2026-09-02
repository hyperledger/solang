// SPDX-License-Identifier: Apache-2.0

use crate::build_solidity;
use soroban_sdk::testutils::Events as _;
use soroban_sdk::{
    contracttype, Address, Bytes, BytesN, FromVal, IntoVal, String as SString, TryFromVal,
    Vec as SVec, I256, U256,
};

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Test {
    pub a: u32,
    pub b: bool,
    pub c: SString,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TupleStruct {
    pub test: Test,
    pub simple: u32,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComplexStruct {
    pub admin: Address,
    pub a64: u64,
    pub assets_vec: soroban_sdk::Vec<u32>,
    pub base_asset: u32,
    pub a32: u32,
    pub b32: u32,
    pub c32: u32,
    pub complex_enum3: u32,
}

const CONTRACT: &str = r#"
    contract other_custom_types {
        struct Test {
            uint32 a;
            bool b;
            string c;
        }

        enum SimpleEnum { First, Second, Third }
        enum RoyalCard { Jack, Queen, King }

        struct TupleStruct {
            Test test;
            SimpleEnum simple;
        }

        struct ComplexStruct {
            address admin;
            uint64 a64;
            uint32[] assets_vec;
            SimpleEnum base_asset;
            uint32 a32;
            uint32 b32;
            uint32 c32;
            RoyalCard complex_enum3;
        }

        event AuthEvent(address indexed hello, string world);

        uint32 persistent count;

        function hello(string memory v) public pure returns (string memory) { return v; }

        function auth(address addr, string memory world) public returns (address) {
            addr.requireAuth();
            emit AuthEvent(addr, world);
            return addr;
        }

        function get_count() public view returns (uint32) {
            return count;
        }

        function inc() public returns (uint32) {
            count += 1;
            return count;
        }

        function woid() public pure {}

        function u32_fail_on_even(uint32 v) public pure returns (uint32) {
            require(v % 2 == 1, "NumberMustBeOdd");
            return v;
        }

        function u32_(uint32 v) public pure returns (uint32) { return v; }
        function i32_(int32 v) public pure returns (int32) { return v; }
        function i64_(int64 v) public pure returns (int64) { return v; }

        function strukt_hel(Test memory t) public pure returns (string[] memory) {
            string[] memory res = new string[](2);
            res[0] = "Hello";
            res[1] = t.c;
            return res;
        }

        function strukt(Test memory t) public pure returns (Test memory) {
            return t;
        }

        function simple(SimpleEnum v) public pure returns (SimpleEnum) { return v; }
        function addresse(address v) public pure returns (address) { return v; }
        function bytes_(bytes memory v) public pure returns (bytes memory) { return v; }
        function bytes_n(bytes9 v) public pure returns (bytes9) { return v; }
        function card(RoyalCard v) public pure returns (RoyalCard) { return v; }
        function boolean(bool v) public pure returns (bool) { return v; }
        function not(bool v) public pure returns (bool) { return !v; }
        function i128(int128 v) public pure returns (int128) { return v; }
        function u128(uint128 v) public pure returns (uint128) { return v; }

        function multi_args(uint32 a, bool b) public pure returns (uint32) {
            return b ? a : 0;
        }

        function vec(uint32[] memory v) public pure returns (uint32[] memory) { return v; }

        function u256(uint256 v) public pure returns (uint256) { return v; }
        function i256(int256 v) public pure returns (int256) { return v; }
        function string_(string memory v) public pure returns (string memory) { return v; }

        function tuple_strukt(TupleStruct memory t) public pure returns (TupleStruct memory) {
            return t;
        }

        function complex_struct(ComplexStruct memory c) public pure returns (ComplexStruct memory) {
            return c;
        }
    }
"#;

const TEST_ADDRESS: &str = "GDRIX624OGPQEX264NY72UKOJQUASHU3PYKL6DDPGSTWXWJSBOTR6N7W";

#[test]
fn example_other_custom_types_counter() {
    let runtime = build_solidity(CONTRACT, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let c: u32 = FromVal::from_val(env, &runtime.invoke_contract(addr, "get_count", vec![]));
    assert_eq!(c, 0);

    let c: u32 = FromVal::from_val(env, &runtime.invoke_contract(addr, "inc", vec![]));
    assert_eq!(c, 1);

    let c: u32 = FromVal::from_val(env, &runtime.invoke_contract(addr, "inc", vec![]));
    assert_eq!(c, 2);

    let c: u32 = FromVal::from_val(env, &runtime.invoke_contract(addr, "get_count", vec![]));
    assert_eq!(c, 2);
}

#[test]
fn example_other_custom_types_hello_symbol_echo() {
    let runtime = build_solidity(CONTRACT, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let s = SString::from_str(env, "friend");
    let r = SString::from_val(
        env,
        &runtime.invoke_contract(addr, "hello", vec![s.clone().into_val(env)]),
    );
    assert_eq!(r, s);
}

#[test]
fn example_other_custom_types_auth_emits_event_and_echoes() {
    let runtime = build_solidity(CONTRACT, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    runtime.env.mock_all_auths();

    let account = Address::from_str(env, TEST_ADDRESS);
    let world = SString::from_str(env, "world");

    let r = Address::from_val(
        env,
        &runtime.invoke_contract(
            addr,
            "auth",
            vec![account.clone().into_val(env), world.clone().into_val(env)],
        ),
    );
    assert_eq!(r, account);

    assert!(!runtime.env.auths().is_empty());

    let events = env.events().all();
    assert_eq!(events.len(), 1);
    let (_, _topics, data) = events.get(0).unwrap();
    let got = SString::from_val(env, &data);
    assert_eq!(got, world);
}

#[test]
fn example_other_custom_types_primitive_echoes() {
    let runtime = build_solidity(CONTRACT, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let r: u32 = FromVal::from_val(
        env,
        &runtime.invoke_contract(addr, "u32_", vec![7_u32.into_val(env)]),
    );
    assert_eq!(r, 7);

    let r: i32 = FromVal::from_val(
        env,
        &runtime.invoke_contract(addr, "i32_", vec![(-7_i32).into_val(env)]),
    );
    assert_eq!(r, -7);

    let r: i64 = FromVal::from_val(
        env,
        &runtime.invoke_contract(addr, "i64_", vec![(-1234567890_i64).into_val(env)]),
    );
    assert_eq!(r, -1234567890);

    let r: bool = FromVal::from_val(
        env,
        &runtime.invoke_contract(addr, "boolean", vec![true.into_val(env)]),
    );
    assert!(r);

    let r: bool = FromVal::from_val(
        env,
        &runtime.invoke_contract(addr, "not", vec![true.into_val(env)]),
    );
    assert!(!r);

    let r: i128 = FromVal::from_val(
        env,
        &runtime.invoke_contract(addr, "i128", vec![(-170141183460469_i128).into_val(env)]),
    );
    assert_eq!(r, -170141183460469);

    let r: u128 = FromVal::from_val(
        env,
        &runtime.invoke_contract(addr, "u128", vec![(1_u128 << 100).into_val(env)]),
    );
    assert_eq!(r, 1_u128 << 100);
}

#[test]
fn example_other_custom_types_big_ints() {
    let runtime = build_solidity(CONTRACT, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let i = I256::from_i128(env, -42);
    let r = I256::from_val(
        env,
        &runtime.invoke_contract(addr, "i256", vec![i.into_val(env)]),
    );
    assert_eq!(r, I256::from_i128(env, -42));

    let u = U256::from_u128(env, 1_u128 << 120);
    let r = U256::from_val(
        env,
        &runtime.invoke_contract(addr, "u256", vec![u.into_val(env)]),
    );
    assert_eq!(r, U256::from_u128(env, 1_u128 << 120));
}

#[test]
fn example_other_custom_types_string_bytes_address() {
    let runtime = build_solidity(CONTRACT, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let s = SString::from_str(env, "soroban");
    let r = SString::from_val(
        env,
        &runtime.invoke_contract(addr, "string_", vec![s.clone().into_val(env)]),
    );
    assert_eq!(r, s);

    let b = Bytes::from_array(env, &[0xAA, 0xBB, 0xCC]);
    let r = Bytes::from_val(
        env,
        &runtime.invoke_contract(addr, "bytes_", vec![b.clone().into_val(env)]),
    );
    assert_eq!(r, b);

    let bn = BytesN::<9>::from_array(env, &[1, 2, 3, 4, 5, 6, 7, 8, 9]);
    let r = BytesN::<9>::from_val(
        env,
        &runtime.invoke_contract(addr, "bytes_n", vec![bn.clone().into_val(env)]),
    );
    assert_eq!(r, bn);

    let a = Address::from_str(env, TEST_ADDRESS);
    let r = Address::from_val(
        env,
        &runtime.invoke_contract(addr, "addresse", vec![a.clone().into_val(env)]),
    );
    assert_eq!(r, a);
}

#[test]
fn example_other_custom_types_enum_echoes() {
    let runtime = build_solidity(CONTRACT, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    for variant in 0_u32..=2 {
        let r: u32 = FromVal::from_val(
            env,
            &runtime.invoke_contract(addr, "simple", vec![variant.into_val(env)]),
        );
        assert_eq!(r, variant);

        let r: u32 = FromVal::from_val(
            env,
            &runtime.invoke_contract(addr, "card", vec![variant.into_val(env)]),
        );
        assert_eq!(r, variant);
    }
}

#[test]
fn example_other_custom_types_vec_echo() {
    let runtime = build_solidity(CONTRACT, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let input: SVec<u32> = soroban_sdk::vec![env, 10_u32, 20, 30, 40];
    let res = runtime.invoke_contract(addr, "vec", vec![input.clone().into_val(env)]);
    let got = SVec::<u32>::try_from_val(env, &res).unwrap();
    assert_eq!(got, input);
}

#[test]
fn example_other_custom_types_multi_args() {
    let runtime = build_solidity(CONTRACT, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let r: u32 = FromVal::from_val(
        env,
        &runtime.invoke_contract(
            addr,
            "multi_args",
            vec![9_u32.into_val(env), true.into_val(env)],
        ),
    );
    assert_eq!(r, 9);

    let r: u32 = FromVal::from_val(
        env,
        &runtime.invoke_contract(
            addr,
            "multi_args",
            vec![9_u32.into_val(env), false.into_val(env)],
        ),
    );
    assert_eq!(r, 0);
}

#[test]
fn example_other_custom_types_void() {
    let runtime = build_solidity(CONTRACT, |_| {});
    let addr = runtime.contracts.last().unwrap();

    let res = runtime.invoke_contract(addr, "woid", vec![]);
    assert!(res.is_void());
}

#[test]
fn example_other_custom_types_struct_roundtrip() {
    let runtime = build_solidity(CONTRACT, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let input = Test {
        a: 42,
        b: true,
        c: SString::from_str(env, "world"),
    };
    let r = Test::from_val(
        env,
        &runtime.invoke_contract(addr, "strukt", vec![input.clone().into_val(env)]),
    );
    assert_eq!(r, input);
}

#[test]
fn example_other_custom_types_tuple_strukt_roundtrip() {
    let runtime = build_solidity(CONTRACT, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let input = TupleStruct {
        test: Test {
            a: 7,
            b: true,
            c: SString::from_str(env, "x"),
        },
        simple: 2,
    };
    let r = TupleStruct::from_val(
        env,
        &runtime.invoke_contract(addr, "tuple_strukt", vec![input.clone().into_val(env)]),
    );
    assert_eq!(r, input);
}

#[test]
fn example_other_custom_types_complex_struct_roundtrip() {
    let runtime = build_solidity(CONTRACT, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let input = ComplexStruct {
        admin: Address::from_str(env, TEST_ADDRESS),
        a64: 123456789,
        assets_vec: soroban_sdk::vec![env, 1_u32, 2, 3],
        base_asset: 1,
        a32: 10,
        b32: 20,
        c32: 30,
        complex_enum3: 2,
    };
    let r = ComplexStruct::from_val(
        env,
        &runtime.invoke_contract(addr, "complex_struct", vec![input.clone().into_val(env)]),
    );
    assert_eq!(r, input);
}

#[test]
fn example_other_custom_types_strukt_hel() {
    let runtime = build_solidity(CONTRACT, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let input = Test {
        a: 1,
        b: false,
        c: SString::from_str(env, "Dev"),
    };
    let res = runtime.invoke_contract(addr, "strukt_hel", vec![input.into_val(env)]);
    let got = SVec::<SString>::try_from_val(env, &res).unwrap();
    assert_eq!(
        got,
        soroban_sdk::vec![
            env,
            SString::from_str(env, "Hello"),
            SString::from_str(env, "Dev"),
        ]
    );
}

#[test]
fn example_other_custom_types_fail_on_even_ok() {
    let runtime = build_solidity(CONTRACT, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let r: u32 = FromVal::from_val(
        env,
        &runtime.invoke_contract(addr, "u32_fail_on_even", vec![5_u32.into_val(env)]),
    );
    assert_eq!(r, 5);
}

#[test]
#[should_panic]
fn example_other_custom_types_fail_on_even_reverts() {
    let runtime = build_solidity(CONTRACT, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    runtime.invoke_contract(addr, "u32_fail_on_even", vec![4_u32.into_val(env)]);
}
