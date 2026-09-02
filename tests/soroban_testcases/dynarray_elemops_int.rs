// SPDX-License-Identifier: Apache-2.0

use crate::build_solidity;
use soroban_sdk::{Env, IntoVal, TryFromVal, Val, Vec as SVec, I256, U256};

fn src(ty: &str, signed: bool) -> String {
    let cast = if signed { "int256" } else { "uint256" };
    let neg = if signed {
        "function neg(TY[] memory a) public pure returns (TY[] memory) {\
         for (uint32 i=0;i<a.length;i++) a[i] = -a[i]; return a; }"
    } else {
        ""
    };
    let pow = if signed {
        ""
    } else {
        "function powf(TY[] memory a) public pure returns (TY[] memory) {\
         for (uint32 i=0;i<a.length;i++) a[i] = a[i] ** 2; return a; }"
    };
    let base = r#"
        contract c {
            TY[] stored;

            function echo(TY[] memory a) public pure returns (TY[] memory) { return a; }
            function store(TY[] memory a) public { stored = a; }
            function load() public view returns (TY[] memory) { return stored; }
            function build(TY seed, uint32 n) public pure returns (TY[] memory) {
                TY[] memory o = new TY[](n);
                for (uint32 i = 0; i < n; i++) o[i] = seed;
                return o;
            }

            function add(TY[] memory a, TY[] memory b) public pure returns (TY[] memory) {
                for (uint32 i = 0; i < a.length; i++) a[i] = a[i] + b[i]; return a; }
            function subf(TY[] memory a, TY[] memory b) public pure returns (TY[] memory) {
                for (uint32 i = 0; i < a.length; i++) a[i] = a[i] - b[i]; return a; }
            function mul(TY[] memory a, TY[] memory b) public pure returns (TY[] memory) {
                for (uint32 i = 0; i < a.length; i++) a[i] = a[i] * b[i]; return a; }
            function divf(TY[] memory a, TY[] memory b) public pure returns (TY[] memory) {
                for (uint32 i = 0; i < a.length; i++) a[i] = a[i] / b[i]; return a; }
            function modf(TY[] memory a, TY[] memory b) public pure returns (TY[] memory) {
                for (uint32 i = 0; i < a.length; i++) a[i] = a[i] % b[i]; return a; }
            POW
            function andf(TY[] memory a, TY[] memory b) public pure returns (TY[] memory) {
                for (uint32 i = 0; i < a.length; i++) a[i] = a[i] & b[i]; return a; }
            function orf(TY[] memory a, TY[] memory b) public pure returns (TY[] memory) {
                for (uint32 i = 0; i < a.length; i++) a[i] = a[i] | b[i]; return a; }
            function xorf(TY[] memory a, TY[] memory b) public pure returns (TY[] memory) {
                for (uint32 i = 0; i < a.length; i++) a[i] = a[i] ^ b[i]; return a; }
            function shl(TY[] memory a) public pure returns (TY[] memory) {
                for (uint32 i = 0; i < a.length; i++) a[i] = a[i] << 1; return a; }
            function shr(TY[] memory a) public pure returns (TY[] memory) {
                for (uint32 i = 0; i < a.length; i++) a[i] = a[i] >> 1; return a; }
            function not_twice(TY[] memory a) public pure returns (TY[] memory) {
                for (uint32 i = 0; i < a.length; i++) a[i] = ~~a[i]; return a; }
            function cast_rt(TY[] memory a) public pure returns (TY[] memory) {
                for (uint32 i = 0; i < a.length; i++) a[i] = TY(CAST(a[i])); return a; }

            function eqf(TY[] memory a, TY[] memory b) public pure returns (bool[] memory) {
                bool[] memory o = new bool[](a.length);
                for (uint32 i = 0; i < a.length; i++) o[i] = a[i] == b[i]; return o; }
            function nef(TY[] memory a, TY[] memory b) public pure returns (bool[] memory) {
                bool[] memory o = new bool[](a.length);
                for (uint32 i = 0; i < a.length; i++) o[i] = a[i] != b[i]; return o; }
            function ltf(TY[] memory a, TY[] memory b) public pure returns (bool[] memory) {
                bool[] memory o = new bool[](a.length);
                for (uint32 i = 0; i < a.length; i++) o[i] = a[i] < b[i]; return o; }
            function lef(TY[] memory a, TY[] memory b) public pure returns (bool[] memory) {
                bool[] memory o = new bool[](a.length);
                for (uint32 i = 0; i < a.length; i++) o[i] = a[i] <= b[i]; return o; }
            function gtf(TY[] memory a, TY[] memory b) public pure returns (bool[] memory) {
                bool[] memory o = new bool[](a.length);
                for (uint32 i = 0; i < a.length; i++) o[i] = a[i] > b[i]; return o; }
            function gef(TY[] memory a, TY[] memory b) public pure returns (bool[] memory) {
                bool[] memory o = new bool[](a.length);
                for (uint32 i = 0; i < a.length; i++) o[i] = a[i] >= b[i]; return o; }

            function store_add(TY[] memory b) public {
                for (uint32 i = 0; i < b.length; i++) stored[i] = stored[i] + b[i]; }
            function combine(TY[] memory b) public view returns (TY[] memory) {
                TY[] memory o = new TY[](b.length);
                for (uint32 i = 0; i < b.length; i++) o[i] = stored[i] + b[i]; return o; }

            NEG
        }
    "#;
    base.replace("CAST", cast)
        .replace("NEG", neg)
        .replace("POW", pow)
        .replace("TY", ty)
}

macro_rules! int_1d_test {
    ($name:ident, $sol:literal, $signed:literal, $elem:ty, $mk:expr) => {
        #[test]
        fn $name() {
            let runtime = build_solidity(&src($sol, $signed), |_| {});
            let addr = runtime.contracts.last().unwrap();
            let env: &Env = &runtime.env;
            let mk = $mk;

            let make = |vals: &[i128]| -> SVec<$elem> {
                let mut v = SVec::new(env);
                for &x in vals {
                    v.push_back(mk(env, x));
                }
                v
            };
            let check = |name: &str, args: std::vec::Vec<Val>, exp: &[i128]| {
                let res = runtime.invoke_contract(addr, name, args);
                assert_eq!(
                    SVec::<$elem>::try_from_val(env, &res).unwrap(),
                    make(exp),
                    "op {}",
                    name
                );
            };
            let a_vals = [12i128, 20, 30, 40];
            let b_vals = [3i128, 4, 5, 6];
            let a_arr = || make(&a_vals);
            let b_arr = || make(&b_vals);
            let pair = |name: &str, exp: &[i128]| {
                check(
                    name,
                    std::vec![a_arr().into_val(env), b_arr().into_val(env)],
                    exp,
                )
            };
            let check_bool = |name: &str, exp: &[bool]| {
                let res = runtime.invoke_contract(
                    addr,
                    name,
                    std::vec![a_arr().into_val(env), b_arr().into_val(env)],
                );
                let mut e = SVec::new(env);
                for &x in exp {
                    e.push_back(x);
                }
                assert_eq!(
                    SVec::<bool>::try_from_val(env, &res).unwrap(),
                    e,
                    "cmp {}",
                    name
                );
            };

            check("echo", std::vec![a_arr().into_val(env)], &a_vals);
            check(
                "build",
                std::vec![mk(env, 7).into_val(env), 4u32.into_val(env)],
                &[7, 7, 7, 7],
            );
            runtime.invoke_contract(addr, "store", std::vec![a_arr().into_val(env)]);
            check("load", std::vec![], &a_vals);
            check(
                "combine",
                std::vec![b_arr().into_val(env)],
                &[15, 24, 35, 46],
            );
            check("load", std::vec![], &a_vals);
            runtime.invoke_contract(addr, "store_add", std::vec![b_arr().into_val(env)]);
            check("load", std::vec![], &[15, 24, 35, 46]);
            pair("add", &[15, 24, 35, 46]);
            pair("subf", &[9, 16, 25, 34]);
            pair("mul", &[36, 80, 150, 240]);
            pair("divf", &[4, 5, 6, 6]);
            pair("modf", &[0, 0, 0, 4]);
            if !$signed {
                check(
                    "powf",
                    std::vec![a_arr().into_val(env)],
                    &[144, 400, 900, 1600],
                );
            }

            pair("andf", &[0, 4, 4, 0]);
            pair("orf", &[15, 20, 31, 46]);
            pair("xorf", &[15, 16, 27, 46]);
            check("shl", std::vec![a_arr().into_val(env)], &[24, 40, 60, 80]);
            check("shr", std::vec![a_arr().into_val(env)], &[6, 10, 15, 20]);
            check("not_twice", std::vec![a_arr().into_val(env)], &a_vals);

            check("cast_rt", std::vec![a_arr().into_val(env)], &a_vals);

            check_bool("eqf", &[false, false, false, false]);
            check_bool("nef", &[true, true, true, true]);
            check_bool("ltf", &[false, false, false, false]);
            check_bool("lef", &[false, false, false, false]);
            check_bool("gtf", &[true, true, true, true]);
            check_bool("gef", &[true, true, true, true]);

            if $signed {
                check(
                    "neg",
                    std::vec![a_arr().into_val(env)],
                    &[-12, -20, -30, -40],
                );
            }
        }
    };
}

int_1d_test!(
    cov_dynarr_int_u32_1d_test,
    "uint32",
    false,
    u32,
    |_e: &Env, x: i128| x as u32
);
int_1d_test!(
    cov_dynarr_int_i32_1d_test,
    "int32",
    true,
    i32,
    |_e: &Env, x: i128| x as i32
);
int_1d_test!(
    cov_dynarr_int_u64_1d_test,
    "uint64",
    false,
    u64,
    |_e: &Env, x: i128| x as u64
);
int_1d_test!(
    cov_dynarr_int_i64_1d_test,
    "int64",
    true,
    i64,
    |_e: &Env, x: i128| x as i64
);
int_1d_test!(
    cov_dynarr_int_u128_1d_test,
    "uint128",
    false,
    u128,
    |_e: &Env, x: i128| x as u128
);
int_1d_test!(
    cov_dynarr_int_i128_1d_test,
    "int128",
    true,
    i128,
    |_e: &Env, x: i128| x
);
int_1d_test!(
    cov_dynarr_int_u256_1d_test,
    "uint256",
    false,
    U256,
    |e: &Env, x: i128| U256::from_u128(e, x as u128)
);
int_1d_test!(
    cov_dynarr_int_i256_1d_test,
    "int256",
    true,
    I256,
    |e: &Env, x: i128| I256::from_i128(e, x)
);

#[test]
fn cov_dynarr_int_bitnot_mem_elem_test() {
    let runtime = build_solidity(
        r#"
        contract c {
            function f(uint64[] memory a, uint32 i) public pure returns (uint64) {
                return ~a[i];
            }
        }
        "#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    let a: SVec<u64> = soroban_sdk::vec![env, 0u64, 1u64];
    let res = runtime.invoke_contract(addr, "f", std::vec![a.into_val(env), 1u32.into_val(env)]);
    assert_eq!(u64::try_from_val(env, &res).unwrap(), !1u64);
}

#[test]
fn cov_dynarr_int_neg_mem_elem_test() {
    let runtime = build_solidity(
        r#"
        contract c {
            function f(int64[] memory a, uint32 i) public pure returns (int64) {
                return -a[i];
            }
        }
        "#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    let a: SVec<i64> = soroban_sdk::vec![env, 5i64, 7i64];
    let res = runtime.invoke_contract(addr, "f", std::vec![a.into_val(env), 1u32.into_val(env)]);
    let expected: Val = (-7i64).into_val(env);
    assert!(expected.shallow_eq(&res));
}

#[test]
fn cov_dynarr_int_unary_storage_subscript_test() {
    let runtime = build_solidity(
        r#"
        contract c {
            int64[] stored;
            function set(int64[] memory a) public { stored = a; }
            function neg_at(uint32 i) public view returns (int64) { return -stored[i]; }
            function not_at(uint32 i) public view returns (int64) { return ~stored[i]; }
        }
        "#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let env: &Env = &runtime.env;

    let a: SVec<i64> = soroban_sdk::vec![env, 5i64, 7i64];
    runtime.invoke_contract(addr, "set", std::vec![a.into_val(env)]);

    let neg0: Val = (-5i64).into_val(env);
    assert!(neg0.shallow_eq(&runtime.invoke_contract(
        addr,
        "neg_at",
        std::vec![0u32.into_val(env)]
    )));
    let neg1: Val = (-7i64).into_val(env);
    assert!(neg1.shallow_eq(&runtime.invoke_contract(
        addr,
        "neg_at",
        std::vec![1u32.into_val(env)]
    )));

    let not0: Val = (!5i64).into_val(env);
    assert!(not0.shallow_eq(&runtime.invoke_contract(
        addr,
        "not_at",
        std::vec![0u32.into_val(env)]
    )));
}

#[test]
fn cov_dynarr_int_overflow_trap_test() {
    let runtime = build_solidity(&src("uint64", false), |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env: &Env = &runtime.env;

    let big: SVec<u64> = soroban_sdk::vec![env, u64::MAX];
    let two: SVec<u64> = soroban_sdk::vec![env, 2u64];
    let logs = runtime.invoke_contract_expect_error(
        addr,
        "mul",
        std::vec![big.into_val(env), two.into_val(env)],
    );
    assert!(
        logs.iter().any(|l| l.contains("overflow")),
        "expected overflow trap, got logs: {logs:?}"
    );
}

const CONTAINER_SRC: &str = r#"
    contract c {
        function len(TY[] memory a) public pure returns (uint32) { return uint32(a.length); }
        function get(TY[] memory a, uint32 i) public pure returns (TY) { return a[i]; }
        function set(TY[] memory a, uint32 i, TY v) public pure returns (TY[] memory) {
            a[i] = v; return a; }
        function push(TY[] memory a, TY v) public pure returns (TY[] memory) {
            a.push(v); return a; }
        function pop(TY[] memory a) public pure returns (TY[] memory) {
            a.pop(); return a; }
        function merge(TY[] memory a, TY[] memory b) public pure returns (TY[] memory) {
            TY[] memory o = new TY[](0);
            for (uint32 i = 0; i < a.length; i++) o.push(a[i]);
            for (uint32 i = 0; i < b.length; i++) o.push(b[i]);
            return o;
        }
    }
"#;

#[test]
fn cov_dynarr_int_u256_container_local_test() {
    let runtime = build_solidity(&CONTAINER_SRC.replace("TY", "uint256"), |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env: &Env = &runtime.env;
    let u = |x: u128| U256::from_u128(env, x);

    let a: SVec<U256> = soroban_sdk::vec![env, u(10), u(20), u(30)];
    let b: SVec<U256> = soroban_sdk::vec![env, u(40), u(50)];

    let three: Val = 3u32.into_val(env);
    assert!(three.shallow_eq(&runtime.invoke_contract(
        addr,
        "len",
        std::vec![a.clone().into_val(env)]
    )));

    let res = runtime.invoke_contract(
        addr,
        "get",
        std::vec![a.clone().into_val(env), 1u32.into_val(env)],
    );
    assert_eq!(U256::try_from_val(env, &res).unwrap(), u(20));

    let res = runtime.invoke_contract(
        addr,
        "set",
        std::vec![
            a.clone().into_val(env),
            0u32.into_val(env),
            u(99).into_val(env)
        ],
    );
    assert_eq!(
        SVec::<U256>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, u(99), u(20), u(30)]
    );

    let res = runtime.invoke_contract(
        addr,
        "push",
        std::vec![a.clone().into_val(env), u(70).into_val(env)],
    );
    assert_eq!(
        SVec::<U256>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, u(10), u(20), u(30), u(70)]
    );

    let res = runtime.invoke_contract(addr, "pop", std::vec![a.clone().into_val(env)]);
    assert_eq!(
        SVec::<U256>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, u(10), u(20)]
    );

    let res = runtime.invoke_contract(addr, "merge", std::vec![a.into_val(env), b.into_val(env)]);
    assert_eq!(
        SVec::<U256>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, u(10), u(20), u(30), u(40), u(50)]
    );
}

#[test]
fn cov_dynarr_int_i256_container_local_test() {
    let runtime = build_solidity(&CONTAINER_SRC.replace("TY", "int256"), |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env: &Env = &runtime.env;
    let i = |x: i128| I256::from_i128(env, x);

    let a: SVec<I256> = soroban_sdk::vec![env, i(10), i(-20), i(30)];
    let b: SVec<I256> = soroban_sdk::vec![env, i(-40), i(50)];

    let three: Val = 3u32.into_val(env);
    assert!(three.shallow_eq(&runtime.invoke_contract(
        addr,
        "len",
        std::vec![a.clone().into_val(env)]
    )));

    let res = runtime.invoke_contract(
        addr,
        "get",
        std::vec![a.clone().into_val(env), 1u32.into_val(env)],
    );
    assert_eq!(I256::try_from_val(env, &res).unwrap(), i(-20));

    let res = runtime.invoke_contract(
        addr,
        "set",
        std::vec![
            a.clone().into_val(env),
            0u32.into_val(env),
            i(-99).into_val(env)
        ],
    );
    assert_eq!(
        SVec::<I256>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, i(-99), i(-20), i(30)]
    );

    let res = runtime.invoke_contract(
        addr,
        "push",
        std::vec![a.clone().into_val(env), i(-70).into_val(env)],
    );
    assert_eq!(
        SVec::<I256>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, i(10), i(-20), i(30), i(-70)]
    );

    let res = runtime.invoke_contract(addr, "pop", std::vec![a.clone().into_val(env)]);
    assert_eq!(
        SVec::<I256>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, i(10), i(-20)]
    );

    let res = runtime.invoke_contract(addr, "merge", std::vec![a.into_val(env), b.into_val(env)]);
    assert_eq!(
        SVec::<I256>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, i(10), i(-20), i(30), i(-40), i(50)]
    );
}
