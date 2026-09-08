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
fn u128_arithmetic() {
    let runtime = build_solidity(
        r#"contract test {
            function add(uint128 a, uint128 b) public pure returns (uint128) { return a + b; }
            function sub(uint128 a, uint128 b) public pure returns (uint128) { return a - b; }
            function pow(uint128 a, uint128 b) public pure returns (uint128) { return a ** b; }
            function mul(uint128 a, uint128 b) public pure returns (uint128) { return a * b; }
            function div(uint128 a, uint128 b) public pure returns (uint128) { return a / b; }
            function mod(uint128 a, uint128 b) public pure returns (uint128) { return a % b; }
        }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();

    // Test uint128 addition
    let a: u128 = 1u128 << 64;
    let b: u128 = 5;
    let res: Val = runtime.invoke_contract(
        addr,
        "add",
        vec![a.into_val(&runtime.env), b.into_val(&runtime.env)],
    );
    assert_eq!(u128::from_val(&runtime.env, &res), a + b);

    // Test uint128 subtraction
    let a: u128 = 1u128 << 64;
    let b: u128 = 5;
    let res: Val = runtime.invoke_contract(
        addr,
        "sub",
        vec![a.into_val(&runtime.env), b.into_val(&runtime.env)],
    );
    assert_eq!(u128::from_val(&runtime.env, &res), a - b);

    // Test uint128 power
    let a: u128 = 1u128 << 25;
    let b: u128 = 5;
    let res: Val = runtime.invoke_contract(
        addr,
        "pow",
        vec![a.into_val(&runtime.env), b.into_val(&runtime.env)],
    );
    assert_eq!(u128::from_val(&runtime.env, &res), a.pow(b as u32));

    // Test uint128 multiply
    let a: u128 = 1u128 << 62;
    let b: u128 = 5;
    let res: Val = runtime.invoke_contract(
        addr,
        "mul",
        vec![a.into_val(&runtime.env), b.into_val(&runtime.env)],
    );
    assert_eq!(u128::from_val(&runtime.env, &res), a * b);

    // Test uint128 divisibility
    let a: u128 = 1u128 << 62;
    let b: u128 = 5;
    let res: Val = runtime.invoke_contract(
        addr,
        "div",
        vec![a.into_val(&runtime.env), b.into_val(&runtime.env)],
    );
    assert_eq!(u128::from_val(&runtime.env, &res), a / b);

    // Test uint128 modularity
    let a: u128 = 1u128 << 62;
    let b: u128 = 5;
    let res: Val = runtime.invoke_contract(
        addr,
        "mod",
        vec![a.into_val(&runtime.env), b.into_val(&runtime.env)],
    );
    assert_eq!(u128::from_val(&runtime.env, &res), a % b);
}

#[test]
fn i128_arithmetic() {
    let runtime = build_solidity(
        r#"contract test {
            function add_i(int128 a, int128 b) public pure returns (int128) { return a + b; }
            function sub_i(int128 a, int128 b) public pure returns (int128) { return a - b; }
            function mul_i(int128 a, int128 b) public pure returns (int128) { return a * b; }
            function div_i(int128 a, int128 b) public pure returns (int128) { return a / b; }
            function mod_i(int128 a, int128 b) public pure returns (int128) { return a % b; }
            function neg_i(int128 a) public pure returns (int128) { return -a; }
        }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();

    // Test int128 addition
    let a_i: i128 = 1i128 << 62;
    let b_i: i128 = 5;
    let res: Val = runtime.invoke_contract(
        addr,
        "add_i",
        vec![a_i.into_val(&runtime.env), b_i.into_val(&runtime.env)],
    );
    assert_eq!(i128::from_val(&runtime.env, &res), a_i + b_i);

    // Test int128 subtraction
    let res: Val = runtime.invoke_contract(
        addr,
        "sub_i",
        vec![a_i.into_val(&runtime.env), b_i.into_val(&runtime.env)],
    );
    assert_eq!(i128::from_val(&runtime.env, &res), a_i - b_i);

    // Test int128 multiply
    let a_i: i128 = 1i128 << 62;
    let b_i: i128 = 5;
    let res: Val = runtime.invoke_contract(
        addr,
        "mul_i",
        vec![a_i.into_val(&runtime.env), b_i.into_val(&runtime.env)],
    );
    assert_eq!(i128::from_val(&runtime.env, &res), a_i * b_i);

    // Test int128 divisibility
    let a_i: i128 = 1i128 << 62;
    let b_i: i128 = 5;
    let res: Val = runtime.invoke_contract(
        addr,
        "div_i",
        vec![a_i.into_val(&runtime.env), b_i.into_val(&runtime.env)],
    );
    assert_eq!(i128::from_val(&runtime.env, &res), a_i / b_i);

    // Test int128 modularity
    let a_i: i128 = 1i128 << 62;
    let b_i: i128 = 5;
    let res: Val = runtime.invoke_contract(
        addr,
        "mod_i",
        vec![a_i.into_val(&runtime.env), b_i.into_val(&runtime.env)],
    );
    assert_eq!(i128::from_val(&runtime.env, &res), a_i % b_i);

    // Test int128 negation
    let a_i: i128 = 1i128 << 62;
    let res: Val = runtime.invoke_contract(addr, "neg_i", vec![a_i.into_val(&runtime.env)]);
    assert_eq!(i128::from_val(&runtime.env, &res), -a_i);
}

#[test]
#[should_panic]
fn u128_overflow_add() {
    let runtime = build_solidity(
        r#"contract test {
            function add(uint128 a, uint128 b) public pure returns (uint128) { return a + b; }
            function sub(uint128 a, uint128 b) public pure returns (uint128) { return a - b; }
        }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();

    // Test uint128 MAX + 1
    let a: u128 = u128::MAX;
    let b: u128 = 1;
    let res: Val = runtime.invoke_contract(
        addr,
        "add",
        vec![a.into_val(&runtime.env), b.into_val(&runtime.env)],
    );
    assert_eq!(u128::from_val(&runtime.env, &res), a + b);
}

#[test]
#[should_panic]
fn u128_overflow_sub() {
    let runtime = build_solidity(
        r#"contract test {
            function add(uint128 a, uint128 b) public pure returns (uint128) { return a + b; }
            function sub(uint128 a, uint128 b) public pure returns (uint128) { return a - b; }
        }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();

    // Test uint128 MIN - 1
    let a: u128 = u128::MIN;
    let b: u128 = 1;
    let res: Val = runtime.invoke_contract(
        addr,
        "sub",
        vec![a.into_val(&runtime.env), b.into_val(&runtime.env)],
    );
    assert_eq!(u128::from_val(&runtime.env, &res), a - b);
}

#[test]
#[should_panic]
fn i128_overflow_add() {
    let runtime = build_solidity(
        r#"contract test {
            function add_i(int128 a, int128 b) public pure returns (int128) { return a + b; }
        }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();

    let a: i128 = i128::MAX;
    let b: i128 = 1;
    let res: Val = runtime.invoke_contract(
        addr,
        "add_i",
        vec![a.into_val(&runtime.env), b.into_val(&runtime.env)],
    );
    assert_eq!(i128::from_val(&runtime.env, &res), a + b);
}

#[test]
#[should_panic]
fn i128_overflow_sub() {
    let runtime = build_solidity(
        r#"contract test {
            function sub_i(int128 a, int128 b) public pure returns (int128) { return a - b; }
        }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();

    let a: i128 = i128::MIN;
    let b: i128 = 1;
    let res: Val = runtime.invoke_contract(
        addr,
        "sub_i",
        vec![a.into_val(&runtime.env), b.into_val(&runtime.env)],
    );
    assert_eq!(i128::from_val(&runtime.env, &res), a - b);
}

#[test]
fn u128_unchecked_wrapping() {
    let runtime = build_solidity(
        r#"contract test {
            function add_unchecked(uint128 a, uint128 b) public pure returns (uint128) { unchecked { return a + b; } }
        }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();

    // uint128 wrap
    let a: u128 = u128::MAX;
    let b: u128 = 1;
    let res: Val = runtime.invoke_contract(
        addr,
        "add_unchecked",
        vec![a.into_val(&runtime.env), b.into_val(&runtime.env)],
    );
    assert_eq!(u128::from_val(&runtime.env, &res), a.wrapping_add(b));
}

#[test]
fn i128_unchecked_wrapping() {
    let runtime = build_solidity(
        r#"contract test {
            function add_unchecked_i(int128 a, int128 b) public pure returns (int128) { unchecked { return a + b; } }
        }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();

    // int128 wrap
    let a_i: i128 = i128::MAX;
    let b_i: i128 = 1;
    let res_i: Val = runtime.invoke_contract(
        addr,
        "add_unchecked_i",
        vec![a_i.into_val(&runtime.env), b_i.into_val(&runtime.env)],
    );
    assert_eq!(i128::from_val(&runtime.env, &res_i), a_i.wrapping_add(b_i));
}

#[test]
fn u128_comparison_ops() {
    let runtime = build_solidity(
        r#"contract math {
        function eq(uint128 a, uint128 b) public pure returns (bool) { return a == b; }
        function ne(uint128 a, uint128 b) public pure returns (bool) { return a != b; }
        function lt(uint128 a, uint128 b) public pure returns (bool) { return a < b; }
        function lte(uint128 a, uint128 b) public pure returns (bool) { return a <= b; }
        function gt(uint128 a, uint128 b) public pure returns (bool) { return a > b; }
        function gte(uint128 a, uint128 b) public pure returns (bool) { return a >= b; }
       }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();

    let pairs = [
        (0u128, 0u128),
        (1, 0),
        (5, 5),
        (255, 128),
        (1u128 << 64, 5),
        (5, 1u128 << 64),
        (1u128 << 64, u128::MAX),
        (u128::MAX, u128::MAX),
    ];

    for (a, b) in pairs {
        let check = |func: &str, expected: bool| {
            let res = runtime.invoke_contract(
                addr,
                func,
                vec![a.into_val(&runtime.env), b.into_val(&runtime.env)],
            );
            assert_eq!(
                bool::from_val(&runtime.env, &res),
                expected,
                "{func}({a}, {b})"
            );
        };
        check("eq", a == b);
        check("ne", a != b);
        check("lt", a < b);
        check("lte", a <= b);
        check("gt", a > b);
        check("gte", a >= b);
    }
}

#[test]
fn i128_comparison_ops() {
    let runtime = build_solidity(
        r#"contract math {
        function eq(int128 a, int128 b) public pure returns (bool) { return a == b; }
        function ne(int128 a, int128 b) public pure returns (bool) { return a != b; }
        function lt(int128 a, int128 b) public pure returns (bool) { return a < b; }
        function lte(int128 a, int128 b) public pure returns (bool) { return a <= b; }
        function gt(int128 a, int128 b) public pure returns (bool) { return a > b; }
        function gte(int128 a, int128 b) public pure returns (bool) { return a >= b; }
       }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();

    let pairs: [(i128, i128); 8] = [
        (0i128, 0i128),
        (1, 0),
        (-5, -5),
        (128, -255),
        (1i128 << 62, 5),
        (5, 1i128 << 62),
        (1i128 << 62, i128::MAX),
        (i128::MIN, i128::MAX),
    ];

    for (a, b) in pairs {
        let check = |func: &str, expected: bool| {
            let res = runtime.invoke_contract(
                addr,
                func,
                vec![a.into_val(&runtime.env), b.into_val(&runtime.env)],
            );
            assert_eq!(
                bool::from_val(&runtime.env, &res),
                expected,
                "{func}({a}, {b})"
            );
        };
        check("eq", a == b);
        check("ne", a != b);
        check("lt", a < b);
        check("lte", a <= b);
        check("gt", a > b);
        check("gte", a >= b);
    }
}

#[test]
fn u128_bitwise_ops() {
    let runtime = build_solidity(
        r#"contract math {
        function and(uint128 a, uint128 b) public returns (uint128) {
            return a & b;
        }
        function or(uint128 a, uint128 b) public returns (uint128) {
            return a | b;
        }
        function xor(uint128 a, uint128 b) public returns (uint128) {
            return a ^ b;
        }
        function not(uint128 a) public returns (uint128) {
            return ~a;
        }
        function shl(uint128 a, uint64 b) public returns (uint128) {
            return a << b;
        }
        function shr(uint128 a, uint64 b) public returns (uint128) {
            return a >> b;
        }
       }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();

    {
        let a: u128 = 0b1010;
        let b: u128 = 0b1100;
        let res = runtime.invoke_contract(
            addr,
            "and",
            vec![a.into_val(&runtime.env), b.into_val(&runtime.env)],
        );
        assert_eq!(u128::from_val(&runtime.env, &res), 0b1000);
    }
    {
        let a: u128 = u128::MAX;
        let b: u128 = 1u128 << 70;
        let res = runtime.invoke_contract(
            addr,
            "and",
            vec![a.into_val(&runtime.env), b.into_val(&runtime.env)],
        );
        assert_eq!(u128::from_val(&runtime.env, &res), 1u128 << 70);
    }
    {
        let a: u128 = 0b1010;
        let b: u128 = 0b0101;
        let res = runtime.invoke_contract(
            addr,
            "or",
            vec![a.into_val(&runtime.env), b.into_val(&runtime.env)],
        );
        assert_eq!(u128::from_val(&runtime.env, &res), 0b1111);
    }
    {
        let a: u128 = 1u128 << 70;
        let b: u128 = 1u128 << 69;
        let res = runtime.invoke_contract(
            addr,
            "or",
            vec![a.into_val(&runtime.env), b.into_val(&runtime.env)],
        );
        assert_eq!(
            u128::from_val(&runtime.env, &res),
            (1u128 << 70) + (1u128 << 69)
        );
    }
    {
        let a: u128 = 0b1010;
        let b: u128 = 0b1100;
        let res = runtime.invoke_contract(
            addr,
            "xor",
            vec![a.into_val(&runtime.env), b.into_val(&runtime.env)],
        );
        assert_eq!(u128::from_val(&runtime.env, &res), 0b0110);
    }
    {
        let a: u128 = 0;
        let res = runtime.invoke_contract(addr, "not", vec![a.into_val(&runtime.env)]);
        assert_eq!(u128::from_val(&runtime.env, &res), u128::MAX);
    }
    {
        let a: u128 = u128::MAX;
        let res = runtime.invoke_contract(addr, "not", vec![a.into_val(&runtime.env)]);
        assert_eq!(u128::from_val(&runtime.env, &res), 0);
    }
    {
        let a: u128 = 1;
        let res = runtime.invoke_contract(
            addr,
            "shl",
            vec![a.into_val(&runtime.env), 4u64.into_val(&runtime.env)],
        );
        assert_eq!(u128::from_val(&runtime.env, &res), 16);
    }
    {
        let a: u128 = 1;
        let res = runtime.invoke_contract(
            addr,
            "shl",
            vec![a.into_val(&runtime.env), 127u64.into_val(&runtime.env)],
        );
        assert_eq!(u128::from_val(&runtime.env, &res), 1u128 << 127);
    }
    {
        let a: u128 = 16;
        let res = runtime.invoke_contract(
            addr,
            "shr",
            vec![a.into_val(&runtime.env), 4u64.into_val(&runtime.env)],
        );
        assert_eq!(u128::from_val(&runtime.env, &res), 1);
    }
    {
        let a: u128 = 1u128 << 127;
        let res = runtime.invoke_contract(
            addr,
            "shr",
            vec![a.into_val(&runtime.env), 127u64.into_val(&runtime.env)],
        );
        assert_eq!(u128::from_val(&runtime.env, &res), 1);
    }
}

#[test]
fn i128_bitwise_ops() {
    let runtime = build_solidity(
        r#"contract math {
        function and(int128 a, int128 b) public returns (int128) {
            return a & b;
        }
        function or(int128 a, int128 b) public returns (int128) {
            return a | b;
        }
        function xor(int128 a, int128 b) public returns (int128) {
            return a ^ b;
        }
        function not(int128 a) public returns (int128) {
            return ~a;
        }
        function shl(int128 a, uint64 b) public returns (int128) {
            return a << b;
        }
        function shr(int128 a, uint64 b) public returns (int128) {
            return a >> b;
        }
       }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();

    {
        let a: i128 = 0b1010;
        let b: i128 = 0b1100;
        let res = runtime.invoke_contract(
            addr,
            "and",
            vec![a.into_val(&runtime.env), b.into_val(&runtime.env)],
        );
        assert_eq!(i128::from_val(&runtime.env, &res), 0b1000);
    }
    {
        let a: i128 = -1;
        let b: i128 = 1i128 << 70;
        let res = runtime.invoke_contract(
            addr,
            "and",
            vec![a.into_val(&runtime.env), b.into_val(&runtime.env)],
        );
        assert_eq!(i128::from_val(&runtime.env, &res), 1i128 << 70);
    }
    {
        let a: i128 = 0b1010;
        let b: i128 = 0b0101;
        let res = runtime.invoke_contract(
            addr,
            "or",
            vec![a.into_val(&runtime.env), b.into_val(&runtime.env)],
        );
        assert_eq!(i128::from_val(&runtime.env, &res), 0b1111);
    }
    {
        let a: i128 = 0b1010;
        let b: i128 = 0b1100;
        let res = runtime.invoke_contract(
            addr,
            "xor",
            vec![a.into_val(&runtime.env), b.into_val(&runtime.env)],
        );
        assert_eq!(i128::from_val(&runtime.env, &res), 0b0110);
    }
    {
        let a: i128 = 0;
        let res = runtime.invoke_contract(addr, "not", vec![a.into_val(&runtime.env)]);
        assert_eq!(i128::from_val(&runtime.env, &res), -1);
    }
    {
        let a: i128 = -1;
        let res = runtime.invoke_contract(addr, "not", vec![a.into_val(&runtime.env)]);
        assert_eq!(i128::from_val(&runtime.env, &res), 0);
    }
    {
        let a: i128 = 1;
        let res = runtime.invoke_contract(
            addr,
            "shl",
            vec![a.into_val(&runtime.env), 4u64.into_val(&runtime.env)],
        );
        assert_eq!(i128::from_val(&runtime.env, &res), 16);
    }
    {
        let a: i128 = -1;
        let res = runtime.invoke_contract(
            addr,
            "shl",
            vec![a.into_val(&runtime.env), 4u64.into_val(&runtime.env)],
        );
        assert_eq!(i128::from_val(&runtime.env, &res), -16);
    }
    {
        let a: i128 = 16;
        let res = runtime.invoke_contract(
            addr,
            "shr",
            vec![a.into_val(&runtime.env), 4u64.into_val(&runtime.env)],
        );
        assert_eq!(i128::from_val(&runtime.env, &res), 1);
    }
    {
        let a: i128 = -16;
        let res = runtime.invoke_contract(
            addr,
            "shr",
            vec![a.into_val(&runtime.env), 4u64.into_val(&runtime.env)],
        );
        assert_eq!(i128::from_val(&runtime.env, &res), -1);
    }
}

#[test]
fn u128_abi_edge_cases() {
    let runtime = build_solidity(
        r#"contract math {
        function identity(uint128 a) public returns (uint128) {
            return a;
        }
        function to_i128(uint128 a) public returns (int128) {
            return int128(a);
        }
        function to_u256(uint128 a) public returns (uint256) {
            return uint256(a);
        }
        function to_u64(uint128 a) public returns (uint64) {
            return uint64(a);
        }
       }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();

    {
        let res = runtime.invoke_contract(addr, "identity", vec![42u128.into_val(&runtime.env)]);
        assert_eq!(u128::from_val(&runtime.env, &res), 42u128);
    }
    {
        let res = runtime.invoke_contract(addr, "identity", vec![u128::MAX.into_val(&runtime.env)]);
        assert_eq!(u128::from_val(&runtime.env, &res), u128::MAX);
    }
    {
        let res = runtime.invoke_contract(addr, "to_i128", vec![u128::MAX.into_val(&runtime.env)]);
        assert_eq!(i128::from_val(&runtime.env, &res), -1i128);
    }
    {
        let res = runtime.invoke_contract(addr, "to_u256", vec![u128::MAX.into_val(&runtime.env)]);
        assert_eq!(
            soroban_sdk::U256::from_val(&runtime.env, &res),
            soroban_sdk::U256::from_u128(&runtime.env, u128::MAX)
        );
    }
    {
        let res = runtime.invoke_contract(
            addr,
            "to_u64",
            vec![(1u128 << 70 | 42u128).into_val(&runtime.env)],
        );
        assert_eq!(u64::from_val(&runtime.env, &res), 42u64);
    }
}

#[test]
fn i128_abi_edge_cases() {
    let runtime = build_solidity(
        r#"contract math {
        function identity(int128 a) public returns (int128) {
            return a;
        }
        function negate(int128 a) public returns (int128) {
            return -a;
        }
        function to_u128(int128 a) public returns (uint128) {
            return uint128(a);
        }
        function to_i256(int128 a) public returns (int256) {
            return int256(a);
        }
        function to_i64(int128 a) public returns (int64) {
            return int64(a);
        }
       }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();

    {
        let res = runtime.invoke_contract(addr, "identity", vec![5i128.into_val(&runtime.env)]);
        assert_eq!(i128::from_val(&runtime.env, &res), 5i128);
    }
    {
        let res = runtime.invoke_contract(addr, "identity", vec![(-1i128).into_val(&runtime.env)]);
        assert_eq!(i128::from_val(&runtime.env, &res), -1i128);
    }
    {
        let res = runtime.invoke_contract(addr, "negate", vec![7i128.into_val(&runtime.env)]);
        assert_eq!(i128::from_val(&runtime.env, &res), -7i128);
    }
    {
        let res = runtime.invoke_contract(addr, "to_u128", vec![(-1i128).into_val(&runtime.env)]);
        assert_eq!(u128::from_val(&runtime.env, &res), u128::MAX);
    }
    {
        let res = runtime.invoke_contract(addr, "to_i256", vec![(-1i128).into_val(&runtime.env)]);
        assert_eq!(
            soroban_sdk::I256::from_val(&runtime.env, &res),
            soroban_sdk::I256::from_i128(&runtime.env, -1i128)
        );
    }
    {
        let res = runtime.invoke_contract(addr, "to_i64", vec![(-1i128).into_val(&runtime.env)]);
        assert_eq!(i64::from_val(&runtime.env, &res), -1i64);
    }
}

#[test]
fn u128_storage() {
    let runtime = build_solidity(
        r#"contract math {
        uint128 stored;

        function set(uint128 val) public {
            stored = val;
        }

        function get() public returns (uint128) {
            return stored;
        }

        function accumulate(uint128 val) public returns (uint128) {
            stored = stored + val;
            return stored;
        }
       }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();

    let _ = runtime.invoke_contract(addr, "set", vec![99u128.into_val(&runtime.env)]);
    let res = runtime.invoke_contract(addr, "get", vec![]);
    assert_eq!(u128::from_val(&runtime.env, &res), 99u128);

    let _ = runtime.invoke_contract(addr, "set", vec![u128::MAX.into_val(&runtime.env)]);
    let res = runtime.invoke_contract(addr, "get", vec![]);
    assert_eq!(u128::from_val(&runtime.env, &res), u128::MAX);

    let _ = runtime.invoke_contract(addr, "set", vec![(1u128 << 70).into_val(&runtime.env)]);
    let res = runtime.invoke_contract(
        addr,
        "accumulate",
        vec![(1u128 << 70).into_val(&runtime.env)],
    );
    assert_eq!(u128::from_val(&runtime.env, &res), 1u128 << 71);
}

#[test]
fn i128_storage() {
    let runtime = build_solidity(
        r#"contract math {
        int128 stored;

        function set(int128 val) public {
            stored = val;
        }

        function get() public returns (int128) {
            return stored;
        }

        function accumulate(int128 val) public returns (int128) {
            stored = stored + val;
            return stored;
        }
       }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();

    let _ = runtime.invoke_contract(addr, "set", vec![42i128.into_val(&runtime.env)]);
    let res = runtime.invoke_contract(addr, "get", vec![]);
    assert_eq!(i128::from_val(&runtime.env, &res), 42i128);

    let _ = runtime.invoke_contract(addr, "set", vec![i128::MIN.into_val(&runtime.env)]);
    let res = runtime.invoke_contract(addr, "get", vec![]);
    assert_eq!(i128::from_val(&runtime.env, &res), i128::MIN);

    let _ = runtime.invoke_contract(addr, "set", vec![(1i128 << 70).into_val(&runtime.env)]);
    let res = runtime.invoke_contract(
        addr,
        "accumulate",
        vec![(1i128 << 70).into_val(&runtime.env)],
    );
    assert_eq!(i128::from_val(&runtime.env, &res), 1i128 << 71);
}

#[test]
fn u128_constants() {
    let runtime = build_solidity(
        r#"contract math {
        function small() public returns (uint128) {
            return 99;
        }
        function large() public returns (uint128) {
            return type(uint128).max;
        }
       }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();

    let res = runtime.invoke_contract(addr, "small", vec![]);
    assert_eq!(u128::from_val(&runtime.env, &res), 99u128);

    let res = runtime.invoke_contract(addr, "large", vec![]);
    assert_eq!(u128::from_val(&runtime.env, &res), u128::MAX);
}

#[test]
fn i128_constants() {
    let runtime = build_solidity(
        r#"contract math {
        function small_pos() public returns (int128) {
            return 42;
        }
        function small_neg() public returns (int128) {
            return -7;
        }
        function large_min() public returns (int128) {
            return type(int128).min;
        }
        function large_max() public returns (int128) {
            return type(int128).max;
        }
    }"#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();

    let res = runtime.invoke_contract(addr, "small_pos", vec![]);
    assert_eq!(i128::from_val(&runtime.env, &res), 42i128);

    let res = runtime.invoke_contract(addr, "small_neg", vec![]);
    assert_eq!(i128::from_val(&runtime.env, &res), -7i128);

    let res = runtime.invoke_contract(addr, "large_min", vec![]);
    assert_eq!(i128::from_val(&runtime.env, &res), i128::MIN);

    let res = runtime.invoke_contract(addr, "large_max", vec![]);
    assert_eq!(i128::from_val(&runtime.env, &res), i128::MAX);
}
