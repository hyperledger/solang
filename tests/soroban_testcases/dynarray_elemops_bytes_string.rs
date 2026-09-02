// SPDX-License-Identifier: Apache-2.0

use crate::build_solidity;
use soroban_sdk::{Bytes, BytesN, Env, IntoVal, String as SString, TryFromVal, Val, Vec as SVec};

fn b4(env: &Env, x: u32) -> BytesN<4> {
    BytesN::from_array(env, &x.to_be_bytes())
}

const BYTESN_1D: &str = r#"
    contract c {
        bytes4[] stored;

        function echo(bytes4[] memory a) public pure returns (bytes4[] memory) { return a; }
        function store(bytes4[] memory a) public { stored = a; }
        function load() public view returns (bytes4[] memory) { return stored; }
        function build(bytes4 seed, uint32 n) public pure returns (bytes4[] memory) {
            bytes4[] memory o = new bytes4[](n);
            for (uint32 i = 0; i < n; i++) o[i] = seed;
            return o;
        }

        function andf(bytes4[] memory a, bytes4[] memory b) public pure returns (bytes4[] memory) {
            for (uint32 i = 0; i < a.length; i++) a[i] = a[i] & b[i]; return a; }
        function orf(bytes4[] memory a, bytes4[] memory b) public pure returns (bytes4[] memory) {
            for (uint32 i = 0; i < a.length; i++) a[i] = a[i] | b[i]; return a; }
        function xorf(bytes4[] memory a, bytes4[] memory b) public pure returns (bytes4[] memory) {
            for (uint32 i = 0; i < a.length; i++) a[i] = a[i] ^ b[i]; return a; }
        function notf(bytes4[] memory a) public pure returns (bytes4[] memory) {
            for (uint32 i = 0; i < a.length; i++) a[i] = ~a[i]; return a; }
        function shl(bytes4[] memory a) public pure returns (bytes4[] memory) {
            for (uint32 i = 0; i < a.length; i++) a[i] = a[i] << 8; return a; }
        function shr(bytes4[] memory a) public pure returns (bytes4[] memory) {
            for (uint32 i = 0; i < a.length; i++) a[i] = a[i] >> 8; return a; }
        function cast_rt(bytes4[] memory a) public pure returns (bytes4[] memory) {
            for (uint32 i = 0; i < a.length; i++) a[i] = bytes4(uint32(a[i])); return a; }

        function eqf(bytes4[] memory a, bytes4[] memory b) public pure returns (bool[] memory) {
            bool[] memory o = new bool[](a.length);
            for (uint32 i = 0; i < a.length; i++) o[i] = a[i] == b[i]; return o; }
        function nef(bytes4[] memory a, bytes4[] memory b) public pure returns (bool[] memory) {
            bool[] memory o = new bool[](a.length);
            for (uint32 i = 0; i < a.length; i++) o[i] = a[i] != b[i]; return o; }

        // container ops on storage
        function len() public view returns (uint32) { return uint32(stored.length); }
        function push(bytes4 v) public { stored.push(v); }
        function pop() public { stored.pop(); }
        function set_i(uint32 i, bytes4 v) public { stored[i] = v; }
        function get_i(uint32 i) public view returns (bytes4) { return stored[i]; }

        // storage <-> local
        function store_and(bytes4[] memory b) public {
            for (uint32 i = 0; i < b.length; i++) stored[i] = stored[i] & b[i]; }
        function combine(bytes4[] memory b) public view returns (bytes4[] memory) {
            bytes4[] memory o = new bytes4[](b.length);
            for (uint32 i = 0; i < b.length; i++) o[i] = stored[i] ^ b[i]; return o; }
    }
"#;

#[test]
fn cov_dynarr_bytesn_1d_test() {
    let runtime = build_solidity(BYTESN_1D, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let a_vals = [0xF0F0F0F0u32, 0x12345678, 0x0000FFFF];
    let b_vals = [0x0FF00FF0u32, 0x000000FF, 0xFFFF0000];
    let a_arr = || {
        let mut v = SVec::new(env);
        for &x in &a_vals {
            v.push_back(b4(env, x));
        }
        v
    };
    let b_arr = || {
        let mut v = SVec::new(env);
        for &x in &b_vals {
            v.push_back(b4(env, x));
        }
        v
    };
    let check = |name: &str, args: Vec<Val>, exp: &[u32]| {
        let res = runtime.invoke_contract(addr, name, args);
        let mut e = SVec::new(env);
        for &x in exp {
            e.push_back(b4(env, x));
        }
        assert_eq!(
            SVec::<BytesN<4>>::try_from_val(env, &res).unwrap(),
            e,
            "op {name}"
        );
    };
    let pair = |name: &str, exp: &[u32]| {
        check(
            name,
            vec![a_arr().into_val(env), b_arr().into_val(env)],
            exp,
        )
    };

    check("echo", vec![a_arr().into_val(env)], &a_vals);
    check(
        "build",
        vec![b4(env, 0xAABBCCDD).into_val(env), 2u32.into_val(env)],
        &[0xAABBCCDD, 0xAABBCCDD],
    );

    pair(
        "andf",
        &[
            0xF0F0F0F0 & 0x0FF00FF0,
            0x12345678 & 0x000000FF,
            0x0000FFFF & 0xFFFF0000,
        ],
    );
    pair(
        "orf",
        &[
            0xF0F0F0F0 | 0x0FF00FF0,
            0x12345678 | 0x000000FF,
            0x0000FFFF | 0xFFFF0000,
        ],
    );
    pair(
        "xorf",
        &[
            0xF0F0F0F0 ^ 0x0FF00FF0,
            0x12345678 ^ 0x000000FF,
            0x0000FFFF ^ 0xFFFF0000,
        ],
    );
    check(
        "notf",
        vec![a_arr().into_val(env)],
        &[!0xF0F0F0F0u32, !0x12345678u32, !0x0000FFFFu32],
    );
    check(
        "shl",
        vec![a_arr().into_val(env)],
        &[0xF0F0F000, 0x34567800, 0x00FFFF00],
    );
    check(
        "shr",
        vec![a_arr().into_val(env)],
        &[0x00F0F0F0, 0x00123456, 0x000000FF],
    );
    check("cast_rt", vec![a_arr().into_val(env)], &a_vals);

    let check_bool = |name: &str, exp: &[bool]| {
        let res = runtime.invoke_contract(
            addr,
            name,
            vec![a_arr().into_val(env), b_arr().into_val(env)],
        );
        let mut e = SVec::new(env);
        for &x in exp {
            e.push_back(x);
        }
        assert_eq!(
            SVec::<bool>::try_from_val(env, &res).unwrap(),
            e,
            "cmp {name}"
        );
    };
    check_bool("eqf", &[false, false, false]);
    check_bool("nef", &[true, true, true]);

    runtime.invoke_contract(addr, "store", vec![a_arr().into_val(env)]);
    check("load", vec![], &a_vals);
    check(
        "combine",
        vec![b_arr().into_val(env)],
        &[
            0xF0F0F0F0 ^ 0x0FF00FF0,
            0x12345678 ^ 0x000000FF,
            0x0000FFFF ^ 0xFFFF0000,
        ],
    );
    check("load", vec![], &a_vals); // combine must not mutate storage
    runtime.invoke_contract(addr, "store_and", vec![b_arr().into_val(env)]);
    check(
        "load",
        vec![],
        &[
            0xF0F0F0F0 & 0x0FF00FF0,
            0x12345678 & 0x000000FF,
            0x0000FFFF & 0xFFFF0000,
        ],
    );

    let three: Val = 3u32.into_val(env);
    assert!(three.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));
    let g0 = runtime.invoke_contract(addr, "get_i", vec![0u32.into_val(env)]);
    assert_eq!(
        BytesN::<4>::try_from_val(env, &g0).unwrap(),
        b4(env, 0xF0F0F0F0 & 0x0FF00FF0)
    );
    runtime.invoke_contract(
        addr,
        "set_i",
        vec![0u32.into_val(env), b4(env, 0x11223344).into_val(env)],
    );
    let g0 = runtime.invoke_contract(addr, "get_i", vec![0u32.into_val(env)]);
    assert_eq!(
        BytesN::<4>::try_from_val(env, &g0).unwrap(),
        b4(env, 0x11223344)
    );
    runtime.invoke_contract(addr, "push", vec![b4(env, 0xDEADBEEF).into_val(env)]);
    let four: Val = 4u32.into_val(env);
    assert!(four.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));
    runtime.invoke_contract(addr, "pop", vec![]);
    assert!(three.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));
}

const BYTESN_ND: &str = r#"
    contract c {
        bytes4[][] stored2;
        bytes4[][][] stored3;

        function echo2(bytes4[][] memory a) public pure returns (bytes4[][] memory) { return a; }
        function store2(bytes4[][] memory a) public { stored2 = a; }
        function get2_i(uint32 i) public view returns (bytes4[] memory) { return stored2[i]; }
        function mem_get_ij(bytes4[][] memory a, uint32 i, uint32 j) public pure returns (bytes4) {
            bytes4[][] memory local = a; return local[i][j];
        }
        // xor-reduce every inner element into one bytes4
        function xor_all2(bytes4[][] memory a) public pure returns (bytes4) {
            bytes4 acc = 0x00000000;
            for (uint32 i = 0; i < a.length; i++)
                for (uint32 j = 0; j < a[i].length; j++) acc = acc ^ a[i][j];
            return acc;
        }

        function echo3(bytes4[][][] memory a) public pure returns (bytes4[][][] memory) { return a; }
        function store3(bytes4[][][] memory a) public { stored3 = a; }
        function get3_i(uint32 i) public view returns (bytes4[][] memory) { return stored3[i]; }
        function mem_get_ijk(bytes4[][][] memory a, uint32 i, uint32 j, uint32 k)
            public pure returns (bytes4) {
            bytes4[][][] memory local = a; return local[i][j][k];
        }
    }
"#;

#[test]
fn cov_dynarr_bytesn_2d_test() {
    let runtime = build_solidity(BYTESN_ND, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let row0: SVec<BytesN<4>> = soroban_sdk::vec![env, b4(env, 0x11111111), b4(env, 0x22222222)];
    let row1: SVec<BytesN<4>> = soroban_sdk::vec![env, b4(env, 0x0F0F0F0F)]; // ragged
    let input: SVec<SVec<BytesN<4>>> = soroban_sdk::vec![env, row0.clone(), row1];

    let res = runtime.invoke_contract(addr, "echo2", vec![input.clone().into_val(env)]);
    assert_eq!(
        SVec::<SVec<BytesN<4>>>::try_from_val(env, &res).unwrap(),
        input
    );

    let res = runtime.invoke_contract(addr, "xor_all2", vec![input.clone().into_val(env)]);
    assert_eq!(
        BytesN::<4>::try_from_val(env, &res).unwrap(),
        b4(env, 0x11111111 ^ 0x22222222 ^ 0x0F0F0F0F)
    );

    let res = runtime.invoke_contract(
        addr,
        "mem_get_ij",
        vec![
            input.clone().into_val(env),
            0u32.into_val(env),
            1u32.into_val(env),
        ],
    );
    assert_eq!(
        BytesN::<4>::try_from_val(env, &res).unwrap(),
        b4(env, 0x22222222)
    );

    runtime.invoke_contract(addr, "store2", vec![input.clone().into_val(env)]);
    let res = runtime.invoke_contract(addr, "get2_i", vec![0u32.into_val(env)]);
    assert_eq!(SVec::<BytesN<4>>::try_from_val(env, &res).unwrap(), row0);
}

#[test]
fn cov_dynarr_bytesn_3d_test() {
    let runtime = build_solidity(BYTESN_ND, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let plane0: SVec<SVec<BytesN<4>>> = soroban_sdk::vec![
        env,
        soroban_sdk::vec![env, b4(env, 0xAAAAAAAA)],
        soroban_sdk::vec![env, b4(env, 0xBBBBBBBB), b4(env, 0xCCCCCCCC)]
    ];
    let plane1: SVec<SVec<BytesN<4>>> =
        soroban_sdk::vec![env, soroban_sdk::vec![env, b4(env, 0xDDDDDDDD)]];
    let input: SVec<SVec<SVec<BytesN<4>>>> = soroban_sdk::vec![env, plane0.clone(), plane1];

    let res = runtime.invoke_contract(addr, "echo3", vec![input.clone().into_val(env)]);
    assert_eq!(
        SVec::<SVec<SVec<BytesN<4>>>>::try_from_val(env, &res).unwrap(),
        input
    );

    let res = runtime.invoke_contract(
        addr,
        "mem_get_ijk",
        vec![
            input.clone().into_val(env),
            0u32.into_val(env),
            1u32.into_val(env),
            1u32.into_val(env),
        ],
    );
    assert_eq!(
        BytesN::<4>::try_from_val(env, &res).unwrap(),
        b4(env, 0xCCCCCCCC)
    );

    runtime.invoke_contract(addr, "store3", vec![input.into_val(env)]);
    let res = runtime.invoke_contract(addr, "get3_i", vec![0u32.into_val(env)]);
    assert_eq!(
        SVec::<SVec<BytesN<4>>>::try_from_val(env, &res).unwrap(),
        plane0
    );
}

const BYTES_1D: &str = r#"
    contract c {
        bytes[] stored;

        function echo(bytes[] memory a) public pure returns (bytes[] memory) { return a; }
        function store(bytes[] memory a) public { stored = a; }
        function load() public view returns (bytes[] memory) { return stored; }

        function lens(bytes[] memory a) public pure returns (uint32[] memory) {
            uint32[] memory o = new uint32[](a.length);
            for (uint32 i = 0; i < a.length; i++) o[i] = uint32(a[i].length); return o; }
        // per-element concat: a[i] = a[i] ++ b[i]
        function concatf(bytes[] memory a, bytes[] memory b) public pure returns (bytes[] memory) {
            for (uint32 i = 0; i < a.length; i++) a[i] = bytes.concat(a[i], b[i]); return a; }
        function idx(bytes[] memory a, uint32 i, uint32 j) public pure returns (bytes1) {
            return a[i][j]; }
        function set_byte(bytes[] memory a, uint32 i, uint32 j, bytes1 v)
            public pure returns (bytes[] memory) { a[i][j] = v; return a; }
        function eqf(bytes[] memory a, bytes[] memory b) public pure returns (bool[] memory) {
            bool[] memory o = new bool[](a.length);
            for (uint32 i = 0; i < a.length; i++) o[i] = a[i] == b[i]; return o; }

        function len() public view returns (uint32) { return uint32(stored.length); }
        function push(bytes memory v) public { stored.push(v); }
        function pop() public { stored.pop(); }
        function get_i(uint32 i) public view returns (bytes memory) { return stored[i]; }
        function s_byte(uint32 i, uint32 j) public view returns (bytes1) { return stored[i][j]; }
    }
"#;

fn by(env: &Env, s: &[u8]) -> Bytes {
    Bytes::from_slice(env, s)
}

#[test]
fn cov_dynarr_bytes_1d_test() {
    let runtime = build_solidity(BYTES_1D, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let a: SVec<Bytes> = soroban_sdk::vec![
        env,
        by(env, &[]),
        by(env, &[0x01]),
        by(env, &[0xde, 0xad, 0xbe, 0xef])
    ];
    let b: SVec<Bytes> =
        soroban_sdk::vec![env, by(env, &[0xaa]), by(env, &[0x02, 0x03]), by(env, &[])];

    let res = runtime.invoke_contract(addr, "echo", vec![a.clone().into_val(env)]);
    assert_eq!(SVec::<Bytes>::try_from_val(env, &res).unwrap(), a);

    let res = runtime.invoke_contract(addr, "lens", vec![a.clone().into_val(env)]);
    assert_eq!(
        SVec::<u32>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, 0u32, 1u32, 4u32]
    );

    let res = runtime.invoke_contract(
        addr,
        "concatf",
        vec![a.clone().into_val(env), b.clone().into_val(env)],
    );
    assert_eq!(
        SVec::<Bytes>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![
            env,
            by(env, &[0xaa]),
            by(env, &[0x01, 0x02, 0x03]),
            by(env, &[0xde, 0xad, 0xbe, 0xef])
        ]
    );

    let res = runtime.invoke_contract(
        addr,
        "idx",
        vec![
            a.clone().into_val(env),
            2u32.into_val(env),
            1u32.into_val(env),
        ],
    );
    assert_eq!(Bytes::try_from_val(env, &res).unwrap(), by(env, &[0xad]));

    let res = runtime.invoke_contract(
        addr,
        "set_byte",
        vec![
            a.clone().into_val(env),
            2u32.into_val(env),
            0u32.into_val(env),
            BytesN::from_array(env, &[0x99]).into_val(env),
        ],
    );
    assert_eq!(
        SVec::<Bytes>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![
            env,
            by(env, &[]),
            by(env, &[0x01]),
            by(env, &[0x99, 0xad, 0xbe, 0xef])
        ]
    );

    let same: SVec<Bytes> = a.clone();
    let res = runtime.invoke_contract(
        addr,
        "eqf",
        vec![a.clone().into_val(env), same.into_val(env)],
    );
    assert_eq!(
        SVec::<bool>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, true, true, true]
    );
    let res = runtime.invoke_contract(
        addr,
        "eqf",
        vec![a.clone().into_val(env), b.clone().into_val(env)],
    );
    assert_eq!(
        SVec::<bool>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, false, false, false]
    );

    runtime.invoke_contract(addr, "store", vec![a.clone().into_val(env)]);
    let res = runtime.invoke_contract(addr, "load", vec![]);
    assert_eq!(SVec::<Bytes>::try_from_val(env, &res).unwrap(), a);

    let res = runtime.invoke_contract(addr, "get_i", vec![2u32.into_val(env)]);
    assert_eq!(
        Bytes::try_from_val(env, &res).unwrap(),
        by(env, &[0xde, 0xad, 0xbe, 0xef])
    );

    let res = runtime.invoke_contract(addr, "s_byte", vec![2u32.into_val(env), 3u32.into_val(env)]);
    assert_eq!(Bytes::try_from_val(env, &res).unwrap(), by(env, &[0xef]));

    let three: Val = 3u32.into_val(env);
    assert!(three.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));
    runtime.invoke_contract(addr, "push", vec![by(env, &[0x55, 0x66]).into_val(env)]);
    let four: Val = 4u32.into_val(env);
    assert!(four.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));
    let res = runtime.invoke_contract(addr, "get_i", vec![3u32.into_val(env)]);
    assert_eq!(
        Bytes::try_from_val(env, &res).unwrap(),
        by(env, &[0x55, 0x66])
    );
    runtime.invoke_contract(addr, "pop", vec![]);
    assert!(three.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));
}

#[test]
#[ignore = "sema panic 'not an array' on a[i].push() for memory bytes[]; reference_soroban_mem_bytes_elem_push_ice"]
fn cov_dynarr_bytes_mem_inner_push_ice() {
    let runtime = build_solidity(
        r#"
        contract c {
            function push_inner(bytes[] memory a, uint32 i, bytes1 v)
                public pure returns (bytes[] memory) { a[i].push(v); return a; }
        }
        "#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    let a: SVec<Bytes> = soroban_sdk::vec![env, by(env, &[0x01])];
    let res = runtime.invoke_contract(
        addr,
        "push_inner",
        vec![
            a.into_val(env),
            0u32.into_val(env),
            BytesN::from_array(env, &[0x77]).into_val(env),
        ],
    );
    assert_eq!(
        SVec::<Bytes>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, by(env, &[0x01, 0x77])]
    );
}

const BYTES_ND: &str = r#"
    contract c {
        bytes[][] stored2;
        bytes[][][] stored3;

        function echo2(bytes[][] memory a) public pure returns (bytes[][] memory) { return a; }
        function store2(bytes[][] memory a) public { stored2 = a; }
        function get2_i(uint32 i) public view returns (bytes[] memory) { return stored2[i]; }
        function byte_at2(bytes[][] memory a, uint32 i, uint32 j, uint32 k)
            public pure returns (bytes1) { return a[i][j][k]; }
        function total_len2(bytes[][] memory a) public pure returns (uint32) {
            uint32 s = 0;
            for (uint32 i = 0; i < a.length; i++)
                for (uint32 j = 0; j < a[i].length; j++) s += uint32(a[i][j].length);
            return s; }

        function echo3(bytes[][][] memory a) public pure returns (bytes[][][] memory) { return a; }
        function store3(bytes[][][] memory a) public { stored3 = a; }
        function byte_at3(bytes[][][] memory a, uint32 i, uint32 j, uint32 k, uint32 l)
            public pure returns (bytes1) { return a[i][j][k][l]; }
    }
"#;

#[test]
fn cov_dynarr_bytes_2d_test() {
    let runtime = build_solidity(BYTES_ND, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let row0: SVec<Bytes> = soroban_sdk::vec![env, by(env, &[0x01, 0x02]), by(env, &[0x03])];
    let row1: SVec<Bytes> = soroban_sdk::vec![env, by(env, &[0x04, 0x05, 0x06])]; // ragged
    let input: SVec<SVec<Bytes>> = soroban_sdk::vec![env, row0.clone(), row1];

    let res = runtime.invoke_contract(addr, "echo2", vec![input.clone().into_val(env)]);
    assert_eq!(SVec::<SVec<Bytes>>::try_from_val(env, &res).unwrap(), input);

    let six: Val = 6u32.into_val(env);
    let res = runtime.invoke_contract(addr, "total_len2", vec![input.clone().into_val(env)]);
    assert!(six.shallow_eq(&res));

    let res = runtime.invoke_contract(
        addr,
        "byte_at2",
        vec![
            input.clone().into_val(env),
            0u32.into_val(env),
            0u32.into_val(env),
            1u32.into_val(env),
        ],
    );
    assert_eq!(Bytes::try_from_val(env, &res).unwrap(), by(env, &[0x02]));

    runtime.invoke_contract(addr, "store2", vec![input.into_val(env)]);
    let res = runtime.invoke_contract(addr, "get2_i", vec![0u32.into_val(env)]);
    assert_eq!(SVec::<Bytes>::try_from_val(env, &res).unwrap(), row0);
}

#[test]
fn cov_dynarr_bytes_3d_test() {
    let runtime = build_solidity(BYTES_ND, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let plane0: SVec<SVec<Bytes>> = soroban_sdk::vec![
        env,
        soroban_sdk::vec![env, by(env, &[0xaa]), by(env, &[0xbb, 0xcc])]
    ];
    let plane1: SVec<SVec<Bytes>> =
        soroban_sdk::vec![env, soroban_sdk::vec![env, by(env, &[0xdd])]];
    let input: SVec<SVec<SVec<Bytes>>> = soroban_sdk::vec![env, plane0, plane1];

    let res = runtime.invoke_contract(addr, "echo3", vec![input.clone().into_val(env)]);
    assert_eq!(
        SVec::<SVec<SVec<Bytes>>>::try_from_val(env, &res).unwrap(),
        input
    );

    let res = runtime.invoke_contract(
        addr,
        "byte_at3",
        vec![
            input.into_val(env),
            0u32.into_val(env),
            0u32.into_val(env),
            1u32.into_val(env),
            0u32.into_val(env),
        ],
    );
    assert_eq!(Bytes::try_from_val(env, &res).unwrap(), by(env, &[0xbb]));
}

fn ss(env: &Env, s: &str) -> SString {
    SString::from_str(env, s)
}

const STRING_1D: &str = r#"
    contract c {
        string[] stored;

        function echo(string[] memory a) public pure returns (string[] memory) { return a; }
        function store(string[] memory a) public { stored = a; }
        function load() public view returns (string[] memory) { return stored; }
        function build(uint32 n) public pure returns (string[] memory) {
            string[] memory o = new string[](n);
            for (uint32 i = 0; i < n; i++) o[i] = "x";
            return o; }

        function lens(string[] memory a) public pure returns (uint32[] memory) {
            uint32[] memory o = new uint32[](a.length);
            for (uint32 i = 0; i < a.length; i++) o[i] = uint32(bytes(a[i]).length); return o; }
        function concatf(string[] memory a, string[] memory b) public pure returns (string[] memory) {
            for (uint32 i = 0; i < a.length; i++) a[i] = string.concat(a[i], b[i]); return a; }
        function eqf(string[] memory a, string[] memory b) public pure returns (bool[] memory) {
            bool[] memory o = new bool[](a.length);
            for (uint32 i = 0; i < a.length; i++) o[i] = a[i] == b[i]; return o; }

        // container ops on storage
        function len() public view returns (uint32) { return uint32(stored.length); }
        function push(string memory v) public { stored.push(v); }
        function pop() public { stored.pop(); }
        function get_i(uint32 i) public view returns (string memory) { return stored[i]; }
        function s_len(uint32 i) public view returns (uint32) { return uint32(bytes(stored[i]).length); }
    }
"#;

#[test]
fn cov_dynarr_string_1d_test() {
    let runtime = build_solidity(STRING_1D, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let a: SVec<SString> =
        soroban_sdk::vec![env, ss(env, "hello"), ss(env, ""), ss(env, "soroban")];
    let b: SVec<SString> = soroban_sdk::vec![env, ss(env, " world"), ss(env, "x"), ss(env, "!")];

    let res = runtime.invoke_contract(addr, "echo", vec![a.clone().into_val(env)]);
    assert_eq!(SVec::<SString>::try_from_val(env, &res).unwrap(), a);

    let res = runtime.invoke_contract(addr, "build", vec![3u32.into_val(env)]);
    assert_eq!(
        SVec::<SString>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, ss(env, "x"), ss(env, "x"), ss(env, "x")]
    );

    let res = runtime.invoke_contract(addr, "lens", vec![a.clone().into_val(env)]);
    assert_eq!(
        SVec::<u32>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, 5u32, 0u32, 7u32]
    );

    let res = runtime.invoke_contract(
        addr,
        "concatf",
        vec![a.clone().into_val(env), b.clone().into_val(env)],
    );
    assert_eq!(
        SVec::<SString>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![
            env,
            ss(env, "hello world"),
            ss(env, "x"),
            ss(env, "soroban!")
        ]
    );

    let res = runtime.invoke_contract(
        addr,
        "eqf",
        vec![a.clone().into_val(env), a.clone().into_val(env)],
    );
    assert_eq!(
        SVec::<bool>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, true, true, true]
    );
    let res = runtime.invoke_contract(
        addr,
        "eqf",
        vec![a.clone().into_val(env), b.clone().into_val(env)],
    );
    assert_eq!(
        SVec::<bool>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, false, false, false]
    );

    runtime.invoke_contract(addr, "store", vec![a.clone().into_val(env)]);
    let res = runtime.invoke_contract(addr, "load", vec![]);
    assert_eq!(SVec::<SString>::try_from_val(env, &res).unwrap(), a);

    let res = runtime.invoke_contract(addr, "get_i", vec![0u32.into_val(env)]);
    assert_eq!(SString::try_from_val(env, &res).unwrap(), ss(env, "hello"));
    let five: Val = 5u32.into_val(env);
    assert!(five.shallow_eq(&runtime.invoke_contract(addr, "s_len", vec![0u32.into_val(env)])));

    let three: Val = 3u32.into_val(env);
    assert!(three.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));
    runtime.invoke_contract(addr, "push", vec![ss(env, "tail").into_val(env)]);
    let four: Val = 4u32.into_val(env);
    assert!(four.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));
    let res = runtime.invoke_contract(addr, "get_i", vec![3u32.into_val(env)]);
    assert_eq!(SString::try_from_val(env, &res).unwrap(), ss(env, "tail"));
    runtime.invoke_contract(addr, "pop", vec![]);
    assert!(three.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));
}

const STRING_ND: &str = r#"
    contract c {
        string[][] stored2;
        string[][][] stored3;

        function echo2(string[][] memory a) public pure returns (string[][] memory) { return a; }
        function store2(string[][] memory a) public { stored2 = a; }
        function get2_i(uint32 i) public view returns (string[] memory) { return stored2[i]; }
        function at2(string[][] memory a, uint32 i, uint32 j) public pure returns (string memory) {
            return a[i][j]; }
        function total_len2(string[][] memory a) public pure returns (uint32) {
            uint32 s = 0;
            for (uint32 i = 0; i < a.length; i++)
                for (uint32 j = 0; j < a[i].length; j++) s += uint32(bytes(a[i][j]).length);
            return s; }

        function echo3(string[][][] memory a) public pure returns (string[][][] memory) { return a; }
        function store3(string[][][] memory a) public { stored3 = a; }
        function at3(string[][][] memory a, uint32 i, uint32 j, uint32 k)
            public pure returns (string memory) { return a[i][j][k]; }
    }
"#;

#[test]
fn cov_dynarr_string_2d_test() {
    let runtime = build_solidity(STRING_ND, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let row0: SVec<SString> = soroban_sdk::vec![env, ss(env, "ab"), ss(env, "c")];
    let row1: SVec<SString> = soroban_sdk::vec![env, ss(env, "def")]; // ragged
    let input: SVec<SVec<SString>> = soroban_sdk::vec![env, row0.clone(), row1];

    let res = runtime.invoke_contract(addr, "echo2", vec![input.clone().into_val(env)]);
    assert_eq!(
        SVec::<SVec<SString>>::try_from_val(env, &res).unwrap(),
        input
    );

    let six: Val = 6u32.into_val(env);
    let res = runtime.invoke_contract(addr, "total_len2", vec![input.clone().into_val(env)]);
    assert!(six.shallow_eq(&res));

    let res = runtime.invoke_contract(
        addr,
        "at2",
        vec![
            input.clone().into_val(env),
            0u32.into_val(env),
            1u32.into_val(env),
        ],
    );
    assert_eq!(SString::try_from_val(env, &res).unwrap(), ss(env, "c"));

    runtime.invoke_contract(addr, "store2", vec![input.into_val(env)]);
    let res = runtime.invoke_contract(addr, "get2_i", vec![0u32.into_val(env)]);
    assert_eq!(SVec::<SString>::try_from_val(env, &res).unwrap(), row0);
}

#[test]
fn cov_dynarr_string_3d_test() {
    let runtime = build_solidity(STRING_ND, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let plane0: SVec<SVec<SString>> =
        soroban_sdk::vec![env, soroban_sdk::vec![env, ss(env, "a"), ss(env, "bb")]];
    let plane1: SVec<SVec<SString>> =
        soroban_sdk::vec![env, soroban_sdk::vec![env, ss(env, "ccc")]];
    let input: SVec<SVec<SVec<SString>>> = soroban_sdk::vec![env, plane0, plane1];

    let res = runtime.invoke_contract(addr, "echo3", vec![input.clone().into_val(env)]);
    assert_eq!(
        SVec::<SVec<SVec<SString>>>::try_from_val(env, &res).unwrap(),
        input
    );

    let res = runtime.invoke_contract(
        addr,
        "at3",
        vec![
            input.into_val(env),
            0u32.into_val(env),
            0u32.into_val(env),
            1u32.into_val(env),
        ],
    );
    assert_eq!(SString::try_from_val(env, &res).unwrap(), ss(env, "bb"));
}
