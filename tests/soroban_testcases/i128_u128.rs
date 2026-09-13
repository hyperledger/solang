// SPDX-License-Identifier: Apache-2.0
use crate::build_solidity;
use soroban_sdk::{FromVal, IntoVal, Val};

#[test]
fn uint128_high_limb_not_dropped_on_encode() {
    let runtime = build_solidity(
        r#"contract test {
            function id(uint128 a) public returns (uint128) { return a; }
        }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let value: u128 = (1u128 << 64) + 5; // high limb set, low limb < 2**56
    let arg: Val = value.into_val(&runtime.env);
    let res: Val = runtime.invoke_contract(addr, "id", vec![arg]);
    let got: u128 = FromVal::from_val(&runtime.env, &res);
    assert_eq!(got, value, "uint128 high 64 bits were dropped on encode");
}

#[test]
fn i128_u128_encode_decode_coverage() {
    let runtime = build_solidity(
        r#"contract test {
            function id_u(uint128 a) public returns (uint128) { return a; }
            function id_i(int128 a) public returns (int128) { return a; }
        }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();

    // uint128 test values: zero, small (56-bit), boundary, high bits, edge cases
    let u_vals = [
        0u128,
        1,
        (1 << 56) - 1,
        1 << 56,
        (1 << 64) - 1,
        1 << 64,
        (1 << 64) + 5,
        u64::MAX as u128,
        u128::MAX,
    ];

    for &v in &u_vals {
        let res = runtime.invoke_contract(addr, "id_u", vec![v.into_val(&runtime.env)]);
        assert_eq!(
            u128::from_val(&runtime.env, &res),
            v,
            "uint128 failed for {}",
            v
        );
    }

    // int128 test values: zero, small pos/neg (56-bit), boundary, high bits, edge cases
    let i_vals = [
        0i128,
        1,
        (1 << 55) - 1,
        1 << 55,
        (1 << 63) - 1,
        -(1 << 55),
        -(1 << 55) - 1,
        -1,
        i64::MIN as i128,
        i128::MIN,
        i128::MAX,
    ];

    for &v in &i_vals {
        let res = runtime.invoke_contract(addr, "id_i", vec![v.into_val(&runtime.env)]);
        assert_eq!(
            i128::from_val(&runtime.env, &res),
            v,
            "int128 failed for {}",
            v
        );
    }
}

#[test]
fn i128_u128_matrix_operations() {
    let runtime = build_solidity(
        r#"contract test {
            // B. Arithmetic
            function add_u(uint128 a, uint128 b) public returns (uint128) { return a + b; }
            function sub_i(int128 a, int128 b) public returns (int128) { return a - b; }
            function mul_u(uint128 a, uint128 b) public returns (uint128) { return a * b; }
            function div_i(int128 a, int128 b) public returns (int128) { return a / b; }
            function pow_u(uint128 a, uint128 b) public returns (uint128) { return a ** b; }
            function un_minus(int128 a) public returns (int128) { return -a; }

            // C. Overflow (Unchecked wraps)
            function wrap_add_u(uint128 a, uint128 b) public returns (uint128) { unchecked { return a + b; } }
            function wrap_sub_u(uint128 a, uint128 b) public returns (uint128) { unchecked { return a - b; } }

            // D. Comparison
            function eq_u(uint128 a, uint128 b) public returns (bool) { return a == b; }
            function lt_i(int128 a, int128 b) public returns (bool) { return a < b; }

            // E. Bitwise
            function shl_u(uint128 a, uint8 b) public returns (uint128) { return a << b; }
            function shr_i(int128 a, uint8 b) public returns (int128) { return a >> b; }
            function bitand_u(uint128 a, uint128 b) public returns (uint128) { return a & b; }

            // F. Casts (validating internal state mapping)
            function cast_to_u64(uint128 a) public returns (uint64) { return uint64(a); }
            function cast_i_to_u(int128 a) public returns (uint128) { return uint128(a); }
            function cast_u96(uint96 a) public returns (uint96) { return a; }

            // G. Storage
            uint128 persistent_u128;
            function set_u128(uint128 a) public { persistent_u128 = a; }
            function get_u128() public returns (uint128) { return persistent_u128; }
            function rmw_u128(uint128 a) public { persistent_u128 += a; }

            // H. Constants
            function min_i() public returns (int128) { return type(int128).min; }
            function max_u() public returns (uint128) { return type(uint128).max; }
        }"#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    macro_rules! invoke {
        ($fn_name:expr, $args:expr, $ret_ty:ty) => {{
            let vals: Vec<Val> = $args.into_iter().map(|a: Val| a).collect();
            let res = runtime.invoke_contract(addr, $fn_name, vals);
            <$ret_ty>::from_val(env, &res)
        }};
    }

    // B. Arithmetic
    assert_eq!(
        invoke!(
            "add_u",
            vec![100u128.into_val(env), 200u128.into_val(env)],
            u128
        ),
        300u128
    );
    assert_eq!(
        invoke!(
            "sub_i",
            vec![10i128.into_val(env), 20i128.into_val(env)],
            i128
        ),
        -10i128
    );
    assert_eq!(
        invoke!(
            "pow_u",
            vec![2u128.into_val(env), 65u128.into_val(env)],
            u128
        ),
        1u128 << 65
    );
    assert_eq!(
        invoke!("un_minus", vec![42i128.into_val(env)], i128),
        -42i128
    );

    // C. Overflow (Unchecked wrap behavior)
    assert_eq!(
        invoke!(
            "wrap_add_u",
            vec![u128::MAX.into_val(env), 1u128.into_val(env)],
            u128
        ),
        0u128
    );
    assert_eq!(
        invoke!(
            "wrap_sub_u",
            vec![0u128.into_val(env), 1u128.into_val(env)],
            u128
        ),
        u128::MAX
    );

    // D. Comparison
    assert!(invoke!(
        "eq_u",
        vec![10u128.into_val(env), 10u128.into_val(env)],
        bool
    ),);
    assert!(invoke!(
        "lt_i",
        vec![(-5i128).into_val(env), 0i128.into_val(env)],
        bool
    ),);

    // E. Bitwise
    assert_eq!(
        invoke!(
            "shl_u",
            vec![1u128.into_val(env), 70u32.into_val(env)],
            u128
        ),
        1u128 << 70
    );
    assert_eq!(
        invoke!(
            "bitand_u",
            vec![0xFFu128.into_val(env), 0x0Fu128.into_val(env)],
            u128
        ),
        0x0F
    );

    // F. Casts
    assert_eq!(
        invoke!("cast_to_u64", vec![u128::MAX.into_val(env)], u64),
        u64::MAX
    );
    assert_eq!(
        invoke!("cast_u96", vec![(1u128 << 80).into_val(env)], u128),
        1u128 << 80
    );

    // G. Storage (RMW and cross-limb boundary)
    let high_limb_val = (1u128 << 64) + 123456789;
    runtime.invoke_contract(addr, "set_u128", vec![high_limb_val.into_val(env)]);
    assert_eq!(invoke!("get_u128", vec![], u128), high_limb_val);

    runtime.invoke_contract(addr, "rmw_u128", vec![1u128.into_val(env)]);
    assert_eq!(invoke!("get_u128", vec![], u128), high_limb_val + 1);

    // H. Constants
    assert_eq!(invoke!("min_i", vec![], i128), i128::MIN);
    assert_eq!(invoke!("max_u", vec![], u128), u128::MAX);
}

#[test]
#[should_panic]
fn i128_u128_overflow_reverts() {
    let runtime = build_solidity(
        r#"contract test {
            function add_u(uint128 a, uint128 b) public returns (uint128) { return a + b; }
        }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    // Checked arithmetic revert will trigger a VM panic in the Soroban test host
    runtime.invoke_contract(
        addr,
        "add_u",
        vec![u128::MAX.into_val(env), 1u128.into_val(env)],
    );
}
