// SPDX-License-Identifier: Apache-2.0

use crate::build_solidity;
use soroban_sdk::{Env, IntoVal, TryFromVal, Val, Vec as SVec};

fn bvec(env: &Env, vals: &[bool]) -> SVec<bool> {
    let mut v = SVec::new(env);
    for &x in vals {
        v.push_back(x);
    }
    v
}

const SRC_1D: &str = r#"
    contract c {
        bool[] stored;

        function echo(bool[] memory a) public pure returns (bool[] memory) { return a; }
        function store(bool[] memory a) public { stored = a; }
        function load() public view returns (bool[] memory) { return stored; }
        function build(bool seed, uint32 n) public pure returns (bool[] memory) {
            bool[] memory o = new bool[](n);
            for (uint32 i = 0; i < n; i++) o[i] = seed;
            return o;
        }

        // element ops
        function notf(bool[] memory a) public pure returns (bool[] memory) {
            for (uint32 i = 0; i < a.length; i++) a[i] = !a[i]; return a; }
        function andf(bool[] memory a, bool[] memory b) public pure returns (bool[] memory) {
            for (uint32 i = 0; i < a.length; i++) a[i] = a[i] && b[i]; return a; }
        function orf(bool[] memory a, bool[] memory b) public pure returns (bool[] memory) {
            for (uint32 i = 0; i < a.length; i++) a[i] = a[i] || b[i]; return a; }
        function eqf(bool[] memory a, bool[] memory b) public pure returns (bool[] memory) {
            bool[] memory o = new bool[](a.length);
            for (uint32 i = 0; i < a.length; i++) o[i] = a[i] == b[i]; return o; }
        function nef(bool[] memory a, bool[] memory b) public pure returns (bool[] memory) {
            bool[] memory o = new bool[](a.length);
            for (uint32 i = 0; i < a.length; i++) o[i] = a[i] != b[i]; return o; }
        // ternary select: o[i] = c[i] ? x[i] : y[i]
        function sel(bool[] memory c, bool[] memory x, bool[] memory y)
            public pure returns (bool[] memory) {
            bool[] memory o = new bool[](c.length);
            for (uint32 i = 0; i < c.length; i++) o[i] = c[i] ? x[i] : y[i]; return o; }

        // container ops on storage
        function len() public view returns (uint32) { return uint32(stored.length); }
        function push(bool v) public { stored.push(v); }
        function pop() public { stored.pop(); }
        function set_i(uint32 i, bool v) public { stored[i] = v; }
        function get_i(uint32 i) public view returns (bool) { return stored[i]; }

        // storage <-> local interactions
        function store_and(bool[] memory b) public {
            for (uint32 i = 0; i < b.length; i++) stored[i] = stored[i] && b[i]; }
        function combine(bool[] memory b) public view returns (bool[] memory) {
            bool[] memory o = new bool[](b.length);
            for (uint32 i = 0; i < b.length; i++) o[i] = stored[i] || b[i]; return o; }
    }
"#;

#[test]
fn cov_dynarr_bool_1d_test() {
    let runtime = build_solidity(SRC_1D, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let a = [true, false, true, false];
    let b = [true, true, false, false];
    let a_arr = || bvec(env, &a);
    let b_arr = || bvec(env, &b);

    let check = |name: &str, args: Vec<Val>, exp: &[bool]| {
        let res = runtime.invoke_contract(addr, name, args);
        assert_eq!(
            SVec::<bool>::try_from_val(env, &res).unwrap(),
            bvec(env, exp),
            "op {name}"
        );
    };
    let pair = |name: &str, exp: &[bool]| {
        check(
            name,
            vec![a_arr().into_val(env), b_arr().into_val(env)],
            exp,
        )
    };

    check("echo", vec![a_arr().into_val(env)], &a);
    check(
        "build",
        vec![true.into_val(env), 3u32.into_val(env)],
        &[true, true, true],
    );

    check(
        "notf",
        vec![a_arr().into_val(env)],
        &[false, true, false, true],
    );
    pair("andf", &[true, false, false, false]);
    pair("orf", &[true, true, true, false]);
    pair("eqf", &[true, false, false, true]);
    pair("nef", &[false, true, true, false]);

    let c = [true, false, true, false];
    let x = [true, true, true, true];
    let y = [false, false, false, false];
    check(
        "sel",
        vec![
            bvec(env, &c).into_val(env),
            bvec(env, &x).into_val(env),
            bvec(env, &y).into_val(env),
        ],
        &[true, false, true, false],
    );

    runtime.invoke_contract(addr, "store", vec![a_arr().into_val(env)]);
    check("load", vec![], &a);
    check(
        "combine",
        vec![b_arr().into_val(env)],
        &[true, true, true, false],
    );
    check("load", vec![], &a); // combine must not mutate storage
    runtime.invoke_contract(addr, "store_and", vec![b_arr().into_val(env)]);
    check("load", vec![], &[true, false, false, false]);

    let four: Val = 4u32.into_val(env);
    assert!(four.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));
    let t: Val = true.into_val(env);
    let f: Val = false.into_val(env);
    assert!(t.shallow_eq(&runtime.invoke_contract(addr, "get_i", vec![0u32.into_val(env)])));
    runtime.invoke_contract(addr, "set_i", vec![0u32.into_val(env), false.into_val(env)]);
    assert!(f.shallow_eq(&runtime.invoke_contract(addr, "get_i", vec![0u32.into_val(env)])));
    runtime.invoke_contract(addr, "push", vec![true.into_val(env)]);
    let five: Val = 5u32.into_val(env);
    assert!(five.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));
    assert!(t.shallow_eq(&runtime.invoke_contract(addr, "get_i", vec![4u32.into_val(env)])));
    runtime.invoke_contract(addr, "pop", vec![]);
    assert!(four.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));
}

#[test]
fn cov_dynarr_bool_short_circuit_test() {
    let runtime = build_solidity(
        r#"
        contract c {
            function sc_and(bool[] memory a, uint32 i) public pure returns (bool) {
                return i < a.length && a[i];
            }
            function sc_or(bool[] memory a, uint32 i) public pure returns (bool) {
                return i >= a.length || a[i];
            }
        }
        "#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    let a = bvec(env, &[true, false]);

    let f: Val = false.into_val(env);
    assert!(f.shallow_eq(&runtime.invoke_contract(
        addr,
        "sc_and",
        vec![a.clone().into_val(env), 2u32.into_val(env)]
    )));

    let t: Val = true.into_val(env);
    assert!(t.shallow_eq(&runtime.invoke_contract(
        addr,
        "sc_or",
        vec![a.clone().into_val(env), 2u32.into_val(env)]
    )));

    assert!(t.shallow_eq(&runtime.invoke_contract(
        addr,
        "sc_and",
        vec![a.clone().into_val(env), 0u32.into_val(env)]
    )));
    assert!(f.shallow_eq(&runtime.invoke_contract(
        addr,
        "sc_or",
        vec![a.into_val(env), 1u32.into_val(env)]
    )));
}

const SRC_2D: &str = r#"
    contract c {
        bool[][] stored;

        function echo(bool[][] memory a) public pure returns (bool[][] memory) { return a; }
        function store(bool[][] memory a) public { stored = a; }
        function load() public view returns (bool[][] memory) { return stored; }

        function len() public view returns (uint32) { return uint32(stored.length); }
        function push(bool[] memory row) public { stored.push(row); }
        function pop() public { stored.pop(); }
        function get_i(uint32 i) public view returns (bool[] memory) { return stored[i]; }
        function set_i(uint32 i, bool[] memory row) public { stored[i] = row; }

        function mem_get_ij(bool[][] memory a, uint32 i, uint32 j) public pure returns (bool) {
            bool[][] memory local = a; return local[i][j];
        }
        function mem_set_ij(bool[][] memory a, uint32 i, uint32 j, bool v)
            public pure returns (bool[][] memory) {
            bool[][] memory local = a; local[i][j] = v; return local;
        }
        // negate every inner element
        function not_all(bool[][] memory a) public pure returns (bool[][] memory) {
            bool[][] memory local = new bool[][](a.length);
            for (uint32 i = 0; i < a.length; i++) {
                local[i] = new bool[](a[i].length);
                for (uint32 j = 0; j < a[i].length; j++) local[i][j] = !a[i][j];
            }
            return local;
        }
        // count trues across both levels
        function count_true(bool[][] memory a) public pure returns (uint32) {
            uint32 n = 0;
            for (uint32 i = 0; i < a.length; i++)
                for (uint32 j = 0; j < a[i].length; j++)
                    if (a[i][j]) n++;
            return n;
        }
    }
"#;

#[test]
fn cov_dynarr_bool_2d_test() {
    let runtime = build_solidity(SRC_2D, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let row0 = bvec(env, &[true, false]);
    let row1 = bvec(env, &[true]); // ragged
    let input: SVec<SVec<bool>> = soroban_sdk::vec![env, row0.clone(), row1];

    let res = runtime.invoke_contract(addr, "echo", vec![input.clone().into_val(env)]);
    assert_eq!(SVec::<SVec<bool>>::try_from_val(env, &res).unwrap(), input);

    let two: Val = 2u32.into_val(env);
    let res = runtime.invoke_contract(addr, "count_true", vec![input.clone().into_val(env)]);
    assert!(two.shallow_eq(&res));

    let n0 = bvec(env, &[false, true]);
    let n1 = bvec(env, &[false]);
    let negated: SVec<SVec<bool>> = soroban_sdk::vec![env, n0, n1];
    let res = runtime.invoke_contract(addr, "not_all", vec![input.clone().into_val(env)]);
    assert_eq!(
        SVec::<SVec<bool>>::try_from_val(env, &res).unwrap(),
        negated
    );

    let t: Val = true.into_val(env);
    let res = runtime.invoke_contract(
        addr,
        "mem_get_ij",
        vec![
            input.clone().into_val(env),
            0u32.into_val(env),
            0u32.into_val(env),
        ],
    );
    assert!(t.shallow_eq(&res));

    let res = runtime.invoke_contract(
        addr,
        "mem_set_ij",
        vec![
            input.clone().into_val(env),
            0u32.into_val(env),
            1u32.into_val(env),
            true.into_val(env),
        ],
    );
    let mrow0 = bvec(env, &[true, true]);
    let mrow1 = bvec(env, &[true]);
    assert_eq!(
        SVec::<SVec<bool>>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, mrow0, mrow1]
    );

    // storage round-trip + subscript + container ops
    runtime.invoke_contract(addr, "store", vec![input.clone().into_val(env)]);
    let res = runtime.invoke_contract(addr, "load", vec![]);
    assert_eq!(SVec::<SVec<bool>>::try_from_val(env, &res).unwrap(), input);

    let res = runtime.invoke_contract(addr, "get_i", vec![0u32.into_val(env)]);
    assert_eq!(SVec::<bool>::try_from_val(env, &res).unwrap(), row0);

    let new_row = bvec(env, &[false, false, true]);
    runtime.invoke_contract(
        addr,
        "set_i",
        vec![1u32.into_val(env), new_row.clone().into_val(env)],
    );
    let res = runtime.invoke_contract(addr, "get_i", vec![1u32.into_val(env)]);
    assert_eq!(SVec::<bool>::try_from_val(env, &res).unwrap(), new_row);

    let three: Val = 3u32.into_val(env);
    runtime.invoke_contract(addr, "push", vec![bvec(env, &[true]).into_val(env)]);
    assert!(three.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));
    runtime.invoke_contract(addr, "pop", vec![]);
    assert!(two.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));
}

const SRC_3D: &str = r#"
    contract c {
        bool[][][] stored;

        function echo(bool[][][] memory a) public pure returns (bool[][][] memory) { return a; }
        function store(bool[][][] memory a) public { stored = a; }
        function load() public view returns (bool[][][] memory) { return stored; }
        function get_i(uint32 i) public view returns (bool[][] memory) { return stored[i]; }

        function mem_get_ijk(bool[][][] memory a, uint32 i, uint32 j, uint32 k)
            public pure returns (bool) {
            bool[][][] memory local = a; return local[i][j][k];
        }
        function count_true(bool[][][] memory a) public pure returns (uint32) {
            uint32 n = 0;
            for (uint32 i = 0; i < a.length; i++)
                for (uint32 j = 0; j < a[i].length; j++)
                    for (uint32 k = 0; k < a[i][j].length; k++)
                        if (a[i][j][k]) n++;
            return n;
        }
    }
"#;

fn input_3d(env: &Env) -> SVec<SVec<SVec<bool>>> {
    let plane0: SVec<SVec<bool>> =
        soroban_sdk::vec![env, bvec(env, &[true, false]), bvec(env, &[true])];
    let plane1: SVec<SVec<bool>> = soroban_sdk::vec![env, bvec(env, &[false, true, true])];
    soroban_sdk::vec![env, plane0, plane1]
}

#[test]
fn cov_dynarr_bool_3d_echo_fold_test() {
    let runtime = build_solidity(SRC_3D, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    let input = input_3d(env);

    let res = runtime.invoke_contract(addr, "echo", vec![input.clone().into_val(env)]);
    assert_eq!(
        SVec::<SVec<SVec<bool>>>::try_from_val(env, &res).unwrap(),
        input
    );

    let four: Val = 4u32.into_val(env);
    let res = runtime.invoke_contract(addr, "count_true", vec![input.into_val(env)]);
    assert!(four.shallow_eq(&res));
}

#[test]
fn cov_dynarr_bool_3d_store_subscript_test() {
    let runtime = build_solidity(SRC_3D, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    let input = input_3d(env);

    runtime.invoke_contract(addr, "store", vec![input.clone().into_val(env)]);
    let res = runtime.invoke_contract(addr, "load", vec![]);
    assert_eq!(
        SVec::<SVec<SVec<bool>>>::try_from_val(env, &res).unwrap(),
        input
    );

    let plane0: SVec<SVec<bool>> =
        soroban_sdk::vec![env, bvec(env, &[true, false]), bvec(env, &[true])];
    let res = runtime.invoke_contract(addr, "get_i", vec![0u32.into_val(env)]);
    assert_eq!(SVec::<SVec<bool>>::try_from_val(env, &res).unwrap(), plane0);

    let t: Val = true.into_val(env);
    let res = runtime.invoke_contract(
        addr,
        "mem_get_ijk",
        vec![
            input.into_val(env),
            1u32.into_val(env),
            0u32.into_val(env),
            2u32.into_val(env),
        ],
    );
    assert!(t.shallow_eq(&res));
}
