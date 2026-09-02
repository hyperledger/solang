// SPDX-License-Identifier: Apache-2.0

use crate::build_solidity;
use soroban_sdk::{contracttype, Bytes, IntoVal, String as SString, TryFromVal, Val, Vec as SVec};

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Point {
    pub x: i64,
    pub y: u64,
}

fn p(x: i64, y: u64) -> Point {
    Point { x, y }
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Nested {
    pub inner: Point,
    pub z: i64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Mixed {
    pub flag: bool,
    pub tag: Bytes,
    pub data: Bytes,
    pub name: SString,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Rec {
    pub id: i64,
    pub name: SString,
}

fn rec(env: &soroban_sdk::Env, id: i64, name: &str) -> Rec {
    Rec {
        id,
        name: SString::from_str(env, name),
    }
}

#[test]
fn cov_dynarr_struct_1d_test() {
    let runtime = build_solidity(
        r#"
        contract c {
            struct Point { int64 x; uint64 y; }
            Point[] stored;

            function echo(Point[] memory a) public pure returns (Point[] memory) {
                return a;
            }

            function store(Point[] memory a) public { stored = a; }
            function load() public view returns (Point[] memory) { return stored; }

            function len() public view returns (uint32) { return uint32(stored.length); }
            function push(Point memory e) public { stored.push(e); }
            function pop() public { stored.pop(); }
            function set_i(uint32 i, Point memory e) public { stored[i] = e; }
            function get_i(uint32 i) public view returns (Point memory) { return stored[i]; }

            function mem_get_i(Point[] memory a, uint32 i) public pure returns (Point memory) {
                Point[] memory local = a;
                Point memory e = local[i];
                return e;
            }

            function mem_set_i(Point[] memory a, uint32 i, Point memory e)
                public pure returns (Point[] memory) {
                Point[] memory local = a;
                local[i] = e;
                return local;
            }
            
            function mem_len(Point[] memory a) public pure returns (uint32) {
                Point[] memory local = a;
                return uint32(local.length);
            }

            function mem_alloc(uint32 n) public pure returns (Point[] memory) {
                Point[] memory local = new Point[](n);
                for (uint32 i = 0; i < n; i++)
                    local[i] = Point(int64(uint64(i)), i);
                return local;
            }

            function build_double(Point[] memory a) public pure returns (Point[] memory) {
                Point[] memory local = new Point[](a.length);
                for (uint32 i = 0; i < a.length; i++)
                    local[i] = Point(a[i].x + a[i].x, a[i].y + a[i].y);
                return local;
            }

            function fold_sum_x(Point[] memory a) public pure returns (int64) {
                int64 s = 0;
                for (uint32 i = 0; i < a.length; i++) s += a[i].x;
                return s;
            }

            function merge_add(Point[] memory a) public {
                for (uint32 i = 0; i < a.length; i++) {
                    stored[i].x = stored[i].x + a[i].x;
                    stored[i].y = stored[i].y + a[i].y;
                }
            }

            function combine(Point[] memory a) public view returns (Point[] memory) {
                Point[] memory local = new Point[](a.length);
                for (uint32 i = 0; i < a.length; i++)
                    local[i] = Point(a[i].x - stored[i].x, a[i].y + stored[i].y);
                return local;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let input: SVec<Point> = soroban_sdk::vec![env, p(1, 2), p(3, 4), p(5, 6)];

    let res = runtime.invoke_contract(addr, "echo", vec![input.clone().into_val(env)]);
    assert_eq!(SVec::<Point>::try_from_val(env, &res).unwrap(), input);

    let res = runtime.invoke_contract(addr, "build_double", vec![input.clone().into_val(env)]);
    assert_eq!(
        SVec::<Point>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, p(2, 4), p(6, 8), p(10, 12)]
    );

    let expected: Val = 9_i64.into_val(env);
    let res = runtime.invoke_contract(addr, "fold_sum_x", vec![input.clone().into_val(env)]);
    assert!(expected.shallow_eq(&res));

    let three: Val = 3u32.into_val(env);
    let res = runtime.invoke_contract(addr, "mem_len", vec![input.clone().into_val(env)]);
    assert!(three.shallow_eq(&res));

    let res = runtime.invoke_contract(
        addr,
        "mem_get_i",
        vec![input.clone().into_val(env), 1u32.into_val(env)],
    );
    assert_eq!(Point::try_from_val(env, &res).unwrap(), p(3, 4));

    let res = runtime.invoke_contract(
        addr,
        "mem_set_i",
        vec![
            input.clone().into_val(env),
            1u32.into_val(env),
            p(99, 88).into_val(env),
        ],
    );
    assert_eq!(
        SVec::<Point>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, p(1, 2), p(99, 88), p(5, 6)]
    );

    let res = runtime.invoke_contract(addr, "mem_alloc", vec![3u32.into_val(env)]);
    assert_eq!(
        SVec::<Point>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, p(0, 0), p(1, 1), p(2, 2)]
    );

    runtime.invoke_contract(addr, "store", vec![input.clone().into_val(env)]);
    let res = runtime.invoke_contract(addr, "load", vec![]);
    assert_eq!(SVec::<Point>::try_from_val(env, &res).unwrap(), input);

    let tens: SVec<Point> = soroban_sdk::vec![env, p(10, 10), p(10, 10), p(10, 10)];
    runtime.invoke_contract(addr, "merge_add", vec![tens.into_val(env)]);
    let merged: SVec<Point> = soroban_sdk::vec![env, p(11, 12), p(13, 14), p(15, 16)];
    let res = runtime.invoke_contract(addr, "load", vec![]);
    assert_eq!(SVec::<Point>::try_from_val(env, &res).unwrap(), merged);

    let hundreds: SVec<Point> = soroban_sdk::vec![env, p(100, 100), p(100, 100), p(100, 100)];
    let res = runtime.invoke_contract(addr, "combine", vec![hundreds.into_val(env)]);
    assert_eq!(
        SVec::<Point>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, p(89, 112), p(87, 114), p(85, 116)]
    );

    let res = runtime.invoke_contract(addr, "get_i", vec![0u32.into_val(env)]);
    assert_eq!(Point::try_from_val(env, &res).unwrap(), p(11, 12));

    runtime.invoke_contract(
        addr,
        "set_i",
        vec![0u32.into_val(env), p(1, 1).into_val(env)],
    );
    let res = runtime.invoke_contract(addr, "get_i", vec![0u32.into_val(env)]);
    assert_eq!(Point::try_from_val(env, &res).unwrap(), p(1, 1));

    let three: Val = 3u32.into_val(env);
    let four: Val = 4u32.into_val(env);
    runtime.invoke_contract(addr, "push", vec![p(2, 2).into_val(env)]);
    assert!(four.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));
    let res = runtime.invoke_contract(addr, "get_i", vec![3u32.into_val(env)]);
    assert_eq!(Point::try_from_val(env, &res).unwrap(), p(2, 2));

    runtime.invoke_contract(addr, "pop", vec![]);
    assert!(three.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));
    let res = runtime.invoke_contract(addr, "load", vec![]);
    assert_eq!(
        SVec::<Point>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, p(1, 1), p(13, 14), p(15, 16)]
    );
}

#[test]
fn cov_dynarr_struct_2d_test() {
    let runtime = build_solidity(
        r#"
        contract c {
            struct Point { int64 x; uint64 y; }
            Point[][] stored;

            function echo(Point[][] memory a) public pure returns (Point[][] memory) {
                return a;
            }
            function store(Point[][] memory a) public { stored = a; }
            function load() public view returns (Point[][] memory) { return stored; }

            // container ops on the outer dimension (element = a row)
            function len() public view returns (uint32) { return uint32(stored.length); }
            function push(Point[] memory row) public { stored.push(row); }
            function pop() public { stored.pop(); }
            function set_i(uint32 i, Point[] memory row) public { stored[i] = row; }
            function get_i(uint32 i) public view returns (Point[] memory) { return stored[i]; }

            // --- same ops on a LOCAL (memory) 2-D array ---
            function mem_len(Point[][] memory a) public pure returns (uint32) {
                Point[][] memory local = a;
                return uint32(local.length);
            }
            // read a whole row from a memory local
            function mem_get_i(Point[][] memory a, uint32 i) public pure returns (Point[] memory) {
                Point[][] memory local = a;
                return local[i];
            }
            // read an inner element a[i][j] from a memory local
            function mem_get_ij(Point[][] memory a, uint32 i, uint32 j)
                public pure returns (Point memory) {
                Point[][] memory local = a;
                Point memory e = local[i][j]; // named local: direct return ICEs (see §3)
                return e;
            }
            // write an inner element on a memory local; return whole to observe
            function mem_set_ij(Point[][] memory a, uint32 i, uint32 j, Point memory e)
                public pure returns (Point[][] memory) {
                Point[][] memory local = a;
                local[i][j] = e;
                return local;
            }

            // per-member doubling at the innermost level
            function build_double(Point[][] memory a) public pure returns (Point[][] memory) {
                Point[][] memory local = new Point[][](a.length);
                for (uint32 i = 0; i < a.length; i++) {
                    local[i] = new Point[](a[i].length);
                    for (uint32 j = 0; j < a[i].length; j++)
                        local[i][j] = Point(a[i][j].x + a[i][j].x, a[i][j].y + a[i][j].y);
                }
                return local;
            }

            // member fold across both levels
            function fold_sum_x(Point[][] memory a) public pure returns (int64) {
                int64 s = 0;
                for (uint32 i = 0; i < a.length; i++)
                    for (uint32 j = 0; j < a[i].length; j++)
                        s += a[i][j].x;
                return s;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let row0: SVec<Point> = soroban_sdk::vec![env, p(1, 2), p(3, 4)];
    let row1: SVec<Point> = soroban_sdk::vec![env, p(5, 6)];
    let input: SVec<SVec<Point>> = soroban_sdk::vec![env, row0.clone(), row1];

    let res = runtime.invoke_contract(addr, "echo", vec![input.clone().into_val(env)]);
    assert_eq!(SVec::<SVec<Point>>::try_from_val(env, &res).unwrap(), input);

    let expected: Val = 9_i64.into_val(env);
    let res = runtime.invoke_contract(addr, "fold_sum_x", vec![input.clone().into_val(env)]);
    assert!(expected.shallow_eq(&res));

    let d0: SVec<Point> = soroban_sdk::vec![env, p(2, 4), p(6, 8)];
    let d1: SVec<Point> = soroban_sdk::vec![env, p(10, 12)];
    let doubled: SVec<SVec<Point>> = soroban_sdk::vec![env, d0, d1];
    let res = runtime.invoke_contract(addr, "build_double", vec![input.clone().into_val(env)]);
    assert_eq!(
        SVec::<SVec<Point>>::try_from_val(env, &res).unwrap(),
        doubled
    );

    let two: Val = 2u32.into_val(env);
    let res = runtime.invoke_contract(addr, "mem_len", vec![input.clone().into_val(env)]);
    assert!(two.shallow_eq(&res));

    let res = runtime.invoke_contract(
        addr,
        "mem_get_i",
        vec![input.clone().into_val(env), 0u32.into_val(env)],
    );
    assert_eq!(SVec::<Point>::try_from_val(env, &res).unwrap(), row0);

    let res = runtime.invoke_contract(
        addr,
        "mem_get_ij",
        vec![
            input.clone().into_val(env),
            0u32.into_val(env),
            1u32.into_val(env),
        ],
    );
    assert_eq!(Point::try_from_val(env, &res).unwrap(), p(3, 4));

    let res = runtime.invoke_contract(
        addr,
        "mem_set_ij",
        vec![
            input.clone().into_val(env),
            0u32.into_val(env),
            1u32.into_val(env),
            p(77, 66).into_val(env),
        ],
    );
    let mrow0: SVec<Point> = soroban_sdk::vec![env, p(1, 2), p(77, 66)];
    let mrow1: SVec<Point> = soroban_sdk::vec![env, p(5, 6)];
    assert_eq!(
        SVec::<SVec<Point>>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, mrow0, mrow1]
    );

    runtime.invoke_contract(addr, "store", vec![input.clone().into_val(env)]);
    let res = runtime.invoke_contract(addr, "load", vec![]);
    assert_eq!(SVec::<SVec<Point>>::try_from_val(env, &res).unwrap(), input);

    let res = runtime.invoke_contract(addr, "get_i", vec![0u32.into_val(env)]);
    assert_eq!(SVec::<Point>::try_from_val(env, &res).unwrap(), row0);

    let new_row: SVec<Point> = soroban_sdk::vec![env, p(9, 9), p(8, 8)];
    runtime.invoke_contract(
        addr,
        "set_i",
        vec![1u32.into_val(env), new_row.clone().into_val(env)],
    );
    let res = runtime.invoke_contract(addr, "get_i", vec![1u32.into_val(env)]);
    assert_eq!(SVec::<Point>::try_from_val(env, &res).unwrap(), new_row);

    let pushed: SVec<Point> = soroban_sdk::vec![env, p(0, 0)];
    let two: Val = 2u32.into_val(env);
    let three: Val = 3u32.into_val(env);
    runtime.invoke_contract(addr, "push", vec![pushed.clone().into_val(env)]);
    assert!(three.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));
    let res = runtime.invoke_contract(addr, "get_i", vec![2u32.into_val(env)]);
    assert_eq!(SVec::<Point>::try_from_val(env, &res).unwrap(), pushed);

    runtime.invoke_contract(addr, "pop", vec![]);
    assert!(two.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));
}

const SRC_3D: &str = r#"
    contract c {
        struct Point { int64 x; uint64 y; }
        Point[][][] stored;

        function echo(Point[][][] memory a) public pure returns (Point[][][] memory) {
            return a;
        }
        function store(Point[][][] memory a) public { stored = a; }
        function load() public view returns (Point[][][] memory) { return stored; }

        // container ops on the outer dimension (element = a plane)
        function len() public view returns (uint32) { return uint32(stored.length); }
        function push(Point[][] memory plane) public { stored.push(plane); }
        function pop() public { stored.pop(); }
        function set_i(uint32 i, Point[][] memory plane) public { stored[i] = plane; }
        function get_i(uint32 i) public view returns (Point[][] memory) { return stored[i]; }

        // --- same ops on a LOCAL (memory) 3-D array ---
        function mem_len(Point[][][] memory a) public pure returns (uint32) {
            Point[][][] memory local = a;
            return uint32(local.length);
        }
        function mem_get_i(Point[][][] memory a, uint32 i) public pure returns (Point[][] memory) {
            Point[][][] memory local = a;
            return local[i];
        }

        function mem_get_ijk(Point[][][] memory a, uint32 i, uint32 j, uint32 k)
            public pure returns (Point memory) {
            Point[][][] memory local = a;
            Point memory e = local[i][j][k]; // named local: direct return ICEs (see §3)
            return e;
        }

        function fold_sum_x(Point[][][] memory a) public pure returns (int64) {
            int64 s = 0;
            for (uint32 i = 0; i < a.length; i++)
                for (uint32 j = 0; j < a[i].length; j++)
                    for (uint32 k = 0; k < a[i][j].length; k++)
                        s += a[i][j][k].x;
            return s;
        }
    }
"#;

fn input_3d(env: &soroban_sdk::Env) -> SVec<SVec<SVec<Point>>> {
    let plane0: SVec<SVec<Point>> = soroban_sdk::vec![
        env,
        soroban_sdk::vec![env, p(1, 2)],
        soroban_sdk::vec![env, p(3, 4)]
    ];
    let plane1: SVec<SVec<Point>> =
        soroban_sdk::vec![env, soroban_sdk::vec![env, p(5, 6), p(7, 8)]];
    soroban_sdk::vec![env, plane0, plane1]
}

#[test]
fn cov_dynarr_struct_3d_echo_fold_test() {
    let runtime = build_solidity(SRC_3D, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    let input = input_3d(env);

    let res = runtime.invoke_contract(addr, "echo", vec![input.clone().into_val(env)]);
    assert_eq!(
        SVec::<SVec<SVec<Point>>>::try_from_val(env, &res).unwrap(),
        input
    );

    let expected: Val = 16_i64.into_val(env);
    let res = runtime.invoke_contract(addr, "fold_sum_x", vec![input.into_val(env)]);
    assert!(expected.shallow_eq(&res));
}

#[test]
fn cov_dynarr_struct_3d_store_subscript_test() {
    let runtime = build_solidity(SRC_3D, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    let input = input_3d(env);

    runtime.invoke_contract(addr, "store", vec![input.clone().into_val(env)]);
    let res = runtime.invoke_contract(addr, "load", vec![]);
    assert_eq!(
        SVec::<SVec<SVec<Point>>>::try_from_val(env, &res).unwrap(),
        input
    );

    let plane0: SVec<SVec<Point>> = soroban_sdk::vec![
        env,
        soroban_sdk::vec![env, p(1, 2)],
        soroban_sdk::vec![env, p(3, 4)]
    ];
    let res = runtime.invoke_contract(addr, "get_i", vec![0u32.into_val(env)]);
    assert_eq!(
        SVec::<SVec<Point>>::try_from_val(env, &res).unwrap(),
        plane0
    );

    let new_plane: SVec<SVec<Point>> = soroban_sdk::vec![env, soroban_sdk::vec![env, p(9, 9)]];
    runtime.invoke_contract(
        addr,
        "set_i",
        vec![1u32.into_val(env), new_plane.clone().into_val(env)],
    );
    let res = runtime.invoke_contract(addr, "get_i", vec![1u32.into_val(env)]);
    assert_eq!(
        SVec::<SVec<Point>>::try_from_val(env, &res).unwrap(),
        new_plane
    );
}

#[test]
fn cov_dynarr_struct_3d_memory_local_test() {
    let runtime = build_solidity(SRC_3D, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    let input = input_3d(env);

    let two: Val = 2u32.into_val(env);
    let res = runtime.invoke_contract(addr, "mem_len", vec![input.clone().into_val(env)]);
    assert!(two.shallow_eq(&res));

    let plane0: SVec<SVec<Point>> = soroban_sdk::vec![
        env,
        soroban_sdk::vec![env, p(1, 2)],
        soroban_sdk::vec![env, p(3, 4)]
    ];
    let res = runtime.invoke_contract(
        addr,
        "mem_get_i",
        vec![input.clone().into_val(env), 0u32.into_val(env)],
    );
    assert_eq!(
        SVec::<SVec<Point>>::try_from_val(env, &res).unwrap(),
        plane0
    );

    let res = runtime.invoke_contract(
        addr,
        "mem_get_ijk",
        vec![
            input.into_val(env),
            1u32.into_val(env),
            0u32.into_val(env),
            1u32.into_val(env),
        ],
    );
    assert_eq!(Point::try_from_val(env, &res).unwrap(), p(7, 8));
}

#[test]
fn cov_dynarr_struct_3d_pushpop_test() {
    let runtime = build_solidity(SRC_3D, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    runtime.invoke_contract(addr, "store", vec![input_3d(env).into_val(env)]);

    let pushed: SVec<SVec<Point>> =
        soroban_sdk::vec![env, soroban_sdk::vec![env, p(0, 0), p(1, 1)]];
    let two: Val = 2u32.into_val(env);
    let three: Val = 3u32.into_val(env);
    runtime.invoke_contract(addr, "push", vec![pushed.clone().into_val(env)]);
    assert!(three.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));
    let res = runtime.invoke_contract(addr, "get_i", vec![2u32.into_val(env)]);
    assert_eq!(
        SVec::<SVec<Point>>::try_from_val(env, &res).unwrap(),
        pushed
    );

    runtime.invoke_contract(addr, "pop", vec![]);
    assert!(two.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));
}

// ---- Known bug: whole-struct return from a memory-array subscript ----------
//
// Returning a WHOLE struct value directly from a `Type::Ref(Struct)` lvalue used
// to ICE the Soroban backend (LLVM doRAUW: a `ret {i64,i64}` in a `ptr`-returning
// function). Fixed by keeping the reference in `try_load_and_cast` on Soroban.
// This exercises all five variants that route through that arm: memory subscript
// (1-D / 2-D), fixed-array subscript, struct-typed member, and ternary.
#[test]
fn cov_dynarr_struct_mem_struct_return() {
    let runtime = build_solidity(
        r#"
        contract c {
            struct Point { int64 x; uint64 y; }
            struct Nested { Point inner; int64 z; }

            // 1-D memory subscript
            function ret_1d(Point[] memory a, uint32 i) public pure returns (Point memory) {
                return a[i];
            }
            // 2-D memory subscript
            function ret_2d(Point[][] memory a, uint32 i, uint32 j) public pure returns (Point memory) {
                return a[i][j];
            }
            // fixed-array subscript
            function ret_fixed(Point[3] memory a, uint32 i) public pure returns (Point memory) {
                return a[i];
            }
            // struct-typed member
            function ret_member(Nested memory n) public pure returns (Point memory) {
                return n.inner;
            }
            // ternary of struct lvalues
            function ret_ternary(Point[] memory a, bool c1) public pure returns (Point memory) {
                return c1 ? a[0] : a[1];
            }
        }
        "#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let flat: SVec<Point> = soroban_sdk::vec![env, p(1, 2), p(3, 4)];

    // 1-D: a[1]
    let res = runtime.invoke_contract(
        addr,
        "ret_1d",
        vec![flat.clone().into_val(env), 1u32.into_val(env)],
    );
    assert_eq!(Point::try_from_val(env, &res).unwrap(), p(3, 4));

    // 2-D: a[0][1]
    let row: SVec<Point> = soroban_sdk::vec![env, p(5, 6), p(7, 8)];
    let nested2d: SVec<SVec<Point>> = soroban_sdk::vec![env, row];
    let res = runtime.invoke_contract(
        addr,
        "ret_2d",
        vec![
            nested2d.into_val(env),
            0u32.into_val(env),
            1u32.into_val(env),
        ],
    );
    assert_eq!(Point::try_from_val(env, &res).unwrap(), p(7, 8));

    // fixed P[3]: a[2]
    let fixed: SVec<Point> = soroban_sdk::vec![env, p(1, 1), p(2, 2), p(3, 3)];
    let res = runtime.invoke_contract(
        addr,
        "ret_fixed",
        vec![fixed.into_val(env), 2u32.into_val(env)],
    );
    assert_eq!(Point::try_from_val(env, &res).unwrap(), p(3, 3));

    // struct member: n.inner
    let n = Nested {
        inner: p(9, 10),
        z: 42,
    };
    let res = runtime.invoke_contract(addr, "ret_member", vec![n.into_val(env)]);
    assert_eq!(Point::try_from_val(env, &res).unwrap(), p(9, 10));

    // ternary: true -> a[0], false -> a[1]
    let res = runtime.invoke_contract(
        addr,
        "ret_ternary",
        vec![flat.clone().into_val(env), true.into_val(env)],
    );
    assert_eq!(Point::try_from_val(env, &res).unwrap(), p(1, 2));
    let res = runtime.invoke_contract(
        addr,
        "ret_ternary",
        vec![flat.into_val(env), false.into_val(env)],
    );
    assert_eq!(Point::try_from_val(env, &res).unwrap(), p(3, 4));
}

// Same struct paths (param/return, B1 memory-subscript return, storage round-trip,
// per-member reads) but with bool / bytesN / bytes / string members.
#[test]
fn cov_dynarr_struct_mixed_members_test() {
    let runtime = build_solidity(
        r#"
        contract c {
            struct M { bool flag; bytes4 tag; bytes data; string name; }
            M[] stored;

            // param -> return (ABI map round-trip)
            function echo(M memory m) public pure returns (M memory) { return m; }

            // B1: whole struct returned from a memory-array subscript
            function ret_sub(M[] memory a, uint32 i) public pure returns (M memory) {
                return a[i];
            }

            // storage round-trip
            function store(M[] memory a) public { stored = a; }
            function load() public view returns (M[] memory) { return stored; }
            function get_i(uint32 i) public view returns (M memory) { return stored[i]; }

            // per-member reads out of storage (path descent), one per field type
            function s_flag(uint32 i) public view returns (bool) { return stored[i].flag; }
            function s_tag(uint32 i) public view returns (bytes4) { return stored[i].tag; }
            function s_data(uint32 i) public view returns (bytes memory) { return stored[i].data; }
            function s_name(uint32 i) public view returns (string memory) { return stored[i].name; }
        }
        "#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let m0 = Mixed {
        flag: true,
        tag: Bytes::from_array(env, &[1, 2, 3, 4]),
        data: Bytes::from_array(env, &[0xaa, 0xbb, 0xcc]),
        name: SString::from_str(env, "alice"),
    };
    let m1 = Mixed {
        flag: false,
        tag: Bytes::from_array(env, &[9, 8, 7, 6]),
        data: Bytes::from_array(env, &[0xff]),
        name: SString::from_str(env, "bob"),
    };
    let arr: SVec<Mixed> = soroban_sdk::vec![env, m0.clone(), m1.clone()];

    // echo: whole-struct param -> return round-trip
    let res = runtime.invoke_contract(addr, "echo", vec![m0.clone().into_val(env)]);
    assert_eq!(Mixed::try_from_val(env, &res).unwrap(), m0);

    // B1: whole struct returned from a memory-array subscript
    let res = runtime.invoke_contract(
        addr,
        "ret_sub",
        vec![arr.clone().into_val(env), 1u32.into_val(env)],
    );
    assert_eq!(Mixed::try_from_val(env, &res).unwrap(), m1);

    // storage round-trip: store then load / get_i
    runtime.invoke_contract(addr, "store", vec![arr.clone().into_val(env)]);
    let res = runtime.invoke_contract(addr, "load", vec![]);
    assert_eq!(SVec::<Mixed>::try_from_val(env, &res).unwrap(), arr);
    let res = runtime.invoke_contract(addr, "get_i", vec![0u32.into_val(env)]);
    assert_eq!(Mixed::try_from_val(env, &res).unwrap(), m0);

    // per-member reads from storage, one per field type
    let flag1: Val = false.into_val(env);
    assert!(flag1.shallow_eq(&runtime.invoke_contract(addr, "s_flag", vec![1u32.into_val(env)])));

    let res = runtime.invoke_contract(addr, "s_tag", vec![1u32.into_val(env)]);
    assert_eq!(
        Bytes::try_from_val(env, &res).unwrap(),
        Bytes::from_array(env, &[9, 8, 7, 6])
    );

    let res = runtime.invoke_contract(addr, "s_data", vec![0u32.into_val(env)]);
    assert_eq!(
        Bytes::try_from_val(env, &res).unwrap(),
        Bytes::from_array(env, &[0xaa, 0xbb, 0xcc])
    );

    let res = runtime.invoke_contract(addr, "s_name", vec![0u32.into_val(env)]);
    assert_eq!(
        SString::try_from_val(env, &res).unwrap(),
        SString::from_str(env, "alice")
    );
}

#[test]
fn cov_dynarr_struct_local_member_rw_test() {
    let runtime = build_solidity(
        r#"
        contract c {
            struct Point { int64 x; uint64 y; }

            function get_x(Point[] memory a, uint32 i) public pure returns (int64) { return a[i].x; }
            function get_y(Point[] memory a, uint32 i) public pure returns (uint64) { return a[i].y; }

            function set_x(Point[] memory a, uint32 i, int64 v) public pure returns (Point[] memory) {
                a[i].x = v; return a;
            }

            function add_x(Point[] memory a, uint32 i, int64 d) public pure returns (Point[] memory) {
                a[i].x += d; return a;
            }

            function combine(Point[] memory a, uint32 i, uint32 j) public pure returns (Point[] memory) {
                a[i].x = a[i].x + int64(a[j].y); return a;
            }

            function swap_members(Point[] memory a, uint32 i) public pure returns (Point[] memory) {
                int64 t = a[i].x; a[i].x = int64(a[i].y); a[i].y = uint64(t); return a;
            }

            function set_both(Point[] memory a, uint32 i, int64 x, uint64 y)
                public pure returns (Point[] memory) {
                a[i].x = x; a[i].y = y; return a;
            }
        }
        "#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let input: SVec<Point> = soroban_sdk::vec![env, p(1, 2), p(3, 4), p(5, 6)];

    let x1: Val = 3i64.into_val(env);
    assert!(x1.shallow_eq(&runtime.invoke_contract(
        addr,
        "get_x",
        vec![input.clone().into_val(env), 1u32.into_val(env)]
    )));
    let y2: Val = 6u64.into_val(env);
    assert!(y2.shallow_eq(&runtime.invoke_contract(
        addr,
        "get_y",
        vec![input.clone().into_val(env), 2u32.into_val(env)]
    )));

    let res = runtime.invoke_contract(
        addr,
        "set_x",
        vec![
            input.clone().into_val(env),
            1u32.into_val(env),
            99i64.into_val(env),
        ],
    );
    assert_eq!(
        SVec::<Point>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, p(1, 2), p(99, 4), p(5, 6)]
    );

    let res = runtime.invoke_contract(
        addr,
        "add_x",
        vec![
            input.clone().into_val(env),
            0u32.into_val(env),
            10i64.into_val(env),
        ],
    );
    assert_eq!(
        SVec::<Point>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, p(11, 2), p(3, 4), p(5, 6)]
    );

    let res = runtime.invoke_contract(
        addr,
        "combine",
        vec![
            input.clone().into_val(env),
            0u32.into_val(env),
            2u32.into_val(env),
        ],
    );
    assert_eq!(
        SVec::<Point>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, p(7, 2), p(3, 4), p(5, 6)]
    );

    let res = runtime.invoke_contract(
        addr,
        "swap_members",
        vec![input.clone().into_val(env), 1u32.into_val(env)],
    );
    assert_eq!(
        SVec::<Point>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, p(1, 2), p(4, 3), p(5, 6)]
    );

    let res = runtime.invoke_contract(
        addr,
        "set_both",
        vec![
            input.into_val(env),
            2u32.into_val(env),
            77i64.into_val(env),
            88u64.into_val(env),
        ],
    );
    assert_eq!(
        SVec::<Point>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, p(1, 2), p(3, 4), p(77, 88)]
    );
}

#[test]
fn cov_dynarr_struct_local_member_mixed_test() {
    let runtime = build_solidity(
        r#"
        contract c {
            struct M { bool flag; bytes4 tag; bytes data; string name; }

            function get_flag(M[] memory a, uint32 i) public pure returns (bool) { return a[i].flag; }
            function get_tag(M[] memory a, uint32 i) public pure returns (bytes4) { return a[i].tag; }
            function get_data(M[] memory a, uint32 i) public pure returns (bytes memory) { return a[i].data; }
            function get_name(M[] memory a, uint32 i) public pure returns (string memory) { return a[i].name; }

            function set_flag(M[] memory a, uint32 i, bool v) public pure returns (M[] memory) {
                a[i].flag = v; return a;
            }
            function toggle_flag(M[] memory a, uint32 i) public pure returns (M[] memory) {
                a[i].flag = !a[i].flag; return a;
            }

            function copy_tag(M[] memory a, uint32 i, uint32 j) public pure returns (M[] memory) {
                a[i].tag = a[j].tag; return a;
            }

            function copy_data(M[] memory a, uint32 i, uint32 j) public pure returns (M[] memory) {
                a[i].data = a[j].data; return a;
            }

            function copy_name(M[] memory a, uint32 i, uint32 j) public pure returns (M[] memory) {
                a[i].name = a[j].name; return a;
            }
        }
        "#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let m0 = Mixed {
        flag: true,
        tag: Bytes::from_array(env, &[1, 2, 3, 4]),
        data: Bytes::from_array(env, &[0xaa, 0xbb, 0xcc]),
        name: SString::from_str(env, "alice"),
    };
    let m1 = Mixed {
        flag: false,
        tag: Bytes::from_array(env, &[9, 8, 7, 6]),
        data: Bytes::from_array(env, &[0xff]),
        name: SString::from_str(env, "bob"),
    };
    let arr: SVec<Mixed> = soroban_sdk::vec![env, m0.clone(), m1.clone()];

    let flag0: Val = true.into_val(env);
    assert!(flag0.shallow_eq(&runtime.invoke_contract(
        addr,
        "get_flag",
        vec![arr.clone().into_val(env), 0u32.into_val(env)]
    )));
    let res = runtime.invoke_contract(
        addr,
        "get_tag",
        vec![arr.clone().into_val(env), 1u32.into_val(env)],
    );
    assert_eq!(
        Bytes::try_from_val(env, &res).unwrap(),
        Bytes::from_array(env, &[9, 8, 7, 6])
    );
    let res = runtime.invoke_contract(
        addr,
        "get_name",
        vec![arr.clone().into_val(env), 0u32.into_val(env)],
    );
    assert_eq!(
        SString::try_from_val(env, &res).unwrap(),
        SString::from_str(env, "alice")
    );

    let res = runtime.invoke_contract(
        addr,
        "set_flag",
        vec![
            arr.clone().into_val(env),
            0u32.into_val(env),
            false.into_val(env),
        ],
    );
    let got = SVec::<Mixed>::try_from_val(env, &res).unwrap();
    assert!(!got.get(0).unwrap().flag);
    assert_eq!(got.get(1).unwrap(), m1.clone());

    let res = runtime.invoke_contract(
        addr,
        "toggle_flag",
        vec![arr.clone().into_val(env), 1u32.into_val(env)],
    );
    assert!(
        SVec::<Mixed>::try_from_val(env, &res)
            .unwrap()
            .get(1)
            .unwrap()
            .flag
    );

    let res = runtime.invoke_contract(
        addr,
        "copy_name",
        vec![
            arr.clone().into_val(env),
            0u32.into_val(env),
            1u32.into_val(env),
        ],
    );
    assert_eq!(
        SVec::<Mixed>::try_from_val(env, &res)
            .unwrap()
            .get(0)
            .unwrap()
            .name,
        SString::from_str(env, "bob")
    );

    let res = runtime.invoke_contract(
        addr,
        "copy_data",
        vec![
            arr.clone().into_val(env),
            0u32.into_val(env),
            1u32.into_val(env),
        ],
    );
    assert_eq!(
        SVec::<Mixed>::try_from_val(env, &res)
            .unwrap()
            .get(0)
            .unwrap()
            .data,
        Bytes::from_array(env, &[0xff])
    );

    let res = runtime.invoke_contract(
        addr,
        "copy_tag",
        vec![arr.into_val(env), 0u32.into_val(env), 1u32.into_val(env)],
    );
    assert_eq!(
        SVec::<Mixed>::try_from_val(env, &res)
            .unwrap()
            .get(0)
            .unwrap()
            .tag,
        Bytes::from_array(env, &[9, 8, 7, 6])
    );
}

#[test]
fn cov_dynarr_struct_pushpop_1d_test() {
    let runtime = build_solidity(
        r#"
        contract c {
            struct Point { int64 x; uint64 y; }
            Point[] stored;

            function len() public view returns (uint32) { return uint32(stored.length); }
            function get_i(uint32 i) public view returns (Point memory) { return stored[i]; }

            function push_val(Point memory e) public { stored.push(e); }

            function push_empty_assign(int64 x, uint64 y) public {
                stored.push();
                uint32 n = uint32(stored.length);
                stored[n-1].x = x;
                stored[n-1].y = y;
            }

            function bump_last_x(int64 d) public {
                uint32 n = uint32(stored.length);
                stored[n-1].x = stored[n-1].x + d;
            }

            function pop() public { stored.pop(); }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let zero: Val = 0u32.into_val(env);
    let one: Val = 1u32.into_val(env);
    let two: Val = 2u32.into_val(env);

    assert!(zero.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));

    runtime.invoke_contract(
        addr,
        "push_empty_assign",
        vec![7i64.into_val(env), 9u64.into_val(env)],
    );
    assert!(one.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));
    let res = runtime.invoke_contract(addr, "get_i", vec![0u32.into_val(env)]);
    assert_eq!(Point::try_from_val(env, &res).unwrap(), p(7, 9));

    runtime.invoke_contract(addr, "push_val", vec![p(3, 4).into_val(env)]);
    assert!(two.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));
    let res = runtime.invoke_contract(addr, "get_i", vec![1u32.into_val(env)]);
    assert_eq!(Point::try_from_val(env, &res).unwrap(), p(3, 4));

    runtime.invoke_contract(addr, "bump_last_x", vec![10i64.into_val(env)]);
    let res = runtime.invoke_contract(addr, "get_i", vec![1u32.into_val(env)]);
    assert_eq!(Point::try_from_val(env, &res).unwrap(), p(13, 4));

    runtime.invoke_contract(addr, "pop", vec![]);
    assert!(one.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));
    let res = runtime.invoke_contract(addr, "get_i", vec![0u32.into_val(env)]);
    assert_eq!(Point::try_from_val(env, &res).unwrap(), p(7, 9));

    runtime.invoke_contract(addr, "pop", vec![]);
    assert!(zero.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));
}

#[test]
fn cov_dynarr_struct_pushpop_inner_2d_test() {
    let runtime = build_solidity(
        r#"
        contract c {
            struct Point { int64 x; uint64 y; }
            Point[][] stored;

            function init_rows(uint32 n) public {
                for (uint32 i = 0; i < n; i++) stored.push();
            }
            function inner_push(uint32 i, Point memory e) public { stored[i].push(e); }
            function inner_pop(uint32 i) public { stored[i].pop(); }
            function inner_len(uint32 i) public view returns (uint32) {
                return uint32(stored[i].length);
            }
            function get_ij(uint32 i, uint32 j) public view returns (Point memory) {
                return stored[i][j];
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let zero: Val = 0u32.into_val(env);
    let one: Val = 1u32.into_val(env);
    let two: Val = 2u32.into_val(env);

    runtime.invoke_contract(addr, "init_rows", vec![2u32.into_val(env)]);
    assert!(zero.shallow_eq(&runtime.invoke_contract(addr, "inner_len", vec![0u32.into_val(env)])));

    runtime.invoke_contract(
        addr,
        "inner_push",
        vec![0u32.into_val(env), p(1, 2).into_val(env)],
    );
    runtime.invoke_contract(
        addr,
        "inner_push",
        vec![0u32.into_val(env), p(3, 4).into_val(env)],
    );
    assert!(two.shallow_eq(&runtime.invoke_contract(addr, "inner_len", vec![0u32.into_val(env)])));

    runtime.invoke_contract(
        addr,
        "inner_push",
        vec![1u32.into_val(env), p(5, 6).into_val(env)],
    );
    assert!(one.shallow_eq(&runtime.invoke_contract(addr, "inner_len", vec![1u32.into_val(env)])));

    let res = runtime.invoke_contract(addr, "get_ij", vec![0u32.into_val(env), 1u32.into_val(env)]);
    assert_eq!(Point::try_from_val(env, &res).unwrap(), p(3, 4));
    let res = runtime.invoke_contract(addr, "get_ij", vec![1u32.into_val(env), 0u32.into_val(env)]);
    assert_eq!(Point::try_from_val(env, &res).unwrap(), p(5, 6));

    runtime.invoke_contract(addr, "inner_pop", vec![0u32.into_val(env)]);
    assert!(one.shallow_eq(&runtime.invoke_contract(addr, "inner_len", vec![0u32.into_val(env)])));
    assert!(one.shallow_eq(&runtime.invoke_contract(addr, "inner_len", vec![1u32.into_val(env)])));
    let res = runtime.invoke_contract(addr, "get_ij", vec![0u32.into_val(env), 0u32.into_val(env)]);
    assert_eq!(Point::try_from_val(env, &res).unwrap(), p(1, 2));
}

#[test]
fn cov_dynarr_struct_pushpop_local_1d_test() {
    let runtime = build_solidity(
        r#"
        contract c {
            struct Point { int64 x; uint64 y; }

            function build_via_push(uint32 n) public pure returns (Point[] memory) {
                Point[] memory local = new Point[](0);
                for (uint32 i = 0; i < n; i++)
                    local.push(Point(i, i));
                return local;
            }

            function push_pop_local() public pure returns (Point[] memory) {
                Point[] memory local = new Point[](0);
                local.push(Point(1, 2));
                local.push(Point(3, 4));
                local.push();               // zero-initialized tail
                local[2].x = 5;
                local[2].y = 6;
                return local;               // [(1,2),(3,4),(5,6)]
            }

            function len_after(uint32 n) public pure returns (uint32) {
                Point[] memory local = new Point[](0);
                for (uint32 i = 0; i < n; i++) local.push(Point(1, 1));
                local.pop();
                return uint32(local.length);
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let res = runtime.invoke_contract(addr, "build_via_push", vec![3u32.into_val(env)]);
    assert_eq!(
        SVec::<Point>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, p(0, 0), p(1, 1), p(2, 2)]
    );

    let res = runtime.invoke_contract(addr, "push_pop_local", vec![]);
    assert_eq!(
        SVec::<Point>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, p(1, 2), p(3, 4), p(5, 6)]
    );

    let four: Val = 4u32.into_val(env);
    let res = runtime.invoke_contract(addr, "len_after", vec![5u32.into_val(env)]);
    assert!(four.shallow_eq(&res));
}

#[test]
fn cov_dynarr_struct_pushpop_mix_storage_to_local_test() {
    let runtime = build_solidity(
        r#"
        contract c {
            struct Point { int64 x; uint64 y; }
            Point[] stored;

            function store(Point[] memory a) public { stored = a; }

            function drain() public returns (Point[] memory) {
                Point[] memory out = new Point[](0);
                while (stored.length > 0) {
                    uint32 n = uint32(stored.length);
                    out.push(stored[n-1]);
                    stored.pop();
                }
                return out;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let input: SVec<Point> = soroban_sdk::vec![env, p(0, 0), p(1, 1), p(2, 2)];
    runtime.invoke_contract(addr, "store", vec![input.into_val(env)]);

    let res = runtime.invoke_contract(addr, "drain", vec![]);
    assert_eq!(
        SVec::<Point>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, p(2, 2), p(1, 1), p(0, 0)]
    );
}

#[test]
fn cov_dynarr_struct_pushpop_mix_local_to_storage_test() {
    let runtime = build_solidity(
        r#"
        contract c {
            struct Point { int64 x; uint64 y; }
            Point[] stored;

            function slen() public view returns (uint32) { return uint32(stored.length); }
            function get_i(uint32 i) public view returns (Point memory) { return stored[i]; }

            function local_to_storage(uint32 n) public {
                Point[] memory local = new Point[](0);
                for (uint32 i = 0; i < n; i++)
                    local.push(Point(i, i * 2));
                for (uint32 i = 0; i < local.length; i++)
                    stored.push(local[i]); 
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    runtime.invoke_contract(addr, "local_to_storage", vec![10u32.into_val(env)]);
    let three: Val = 10u32.into_val(env);
    assert!(three.shallow_eq(&runtime.invoke_contract(addr, "slen", vec![])));
    let res = runtime.invoke_contract(addr, "get_i", vec![9u32.into_val(env)]);
    assert_eq!(Point::try_from_val(env, &res).unwrap(), p(9, 18));
}

#[test]
fn cov_dynarr_struct_local_container_ops_param_test() {
    let runtime = build_solidity(
        r#"
        contract c {
            struct Rec { int64 id; string name; }

            function len(Rec[] memory a) public pure returns (uint32) {
                return uint32(a.length);
            }
            function get(Rec[] memory a, uint32 i) public pure returns (Rec memory) {
                return a[i];
            }
            function set(Rec[] memory a, uint32 i, Rec memory e) public pure returns (Rec[] memory) {
                a[i] = e; return a;
            }
            function push(Rec[] memory a, Rec memory e) public pure returns (Rec[] memory) {
                a.push(e); return a;
            }
            function pop(Rec[] memory a) public pure returns (Rec[] memory) {
                a.pop(); return a;
            }

            function merge(Rec[] memory a, Rec[] memory b) public pure returns (Rec[] memory) {
                Rec[] memory out = new Rec[](0);
                for (uint32 i = 0; i < a.length; i++) out.push(a[i]);
                for (uint32 i = 0; i < b.length; i++) out.push(b[i]);
                return out;
            }

            function get_id(Rec[] memory a, uint32 i) public pure returns (int64) {
                return a[i].id;
            }
            function get_name(Rec[] memory a, uint32 i) public pure returns (string memory) {
                return a[i].name;
            }
        }
        "#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let r0 = rec(env, 1, "alice");
    let r1 = rec(env, 2, "bob");
    let arr: SVec<Rec> = soroban_sdk::vec![env, r0.clone(), r1.clone()];

    let two: Val = 2u32.into_val(env);
    assert!(two.shallow_eq(&runtime.invoke_contract(addr, "len", vec![arr.clone().into_val(env)])));

    let res = runtime.invoke_contract(
        addr,
        "get",
        vec![arr.clone().into_val(env), 1u32.into_val(env)],
    );
    assert_eq!(Rec::try_from_val(env, &res).unwrap(), r1);

    let id1: Val = 2i64.into_val(env);
    assert!(id1.shallow_eq(&runtime.invoke_contract(
        addr,
        "get_id",
        vec![arr.clone().into_val(env), 1u32.into_val(env)]
    )));
    let res = runtime.invoke_contract(
        addr,
        "get_name",
        vec![arr.clone().into_val(env), 0u32.into_val(env)],
    );
    assert_eq!(
        SString::try_from_val(env, &res).unwrap(),
        SString::from_str(env, "alice")
    );

    let r2 = rec(env, 9, "carol");
    let res = runtime.invoke_contract(
        addr,
        "set",
        vec![
            arr.clone().into_val(env),
            0u32.into_val(env),
            r2.clone().into_val(env),
        ],
    );
    assert_eq!(
        SVec::<Rec>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, r2.clone(), r1.clone()]
    );

    let res = runtime.invoke_contract(
        addr,
        "push",
        vec![arr.clone().into_val(env), r2.clone().into_val(env)],
    );
    let pushed = SVec::<Rec>::try_from_val(env, &res).unwrap();
    assert_eq!(
        pushed,
        soroban_sdk::vec![env, r0.clone(), r1.clone(), r2.clone()]
    );
    let three: Val = 3u32.into_val(env);
    assert!(three.shallow_eq(&runtime.invoke_contract(
        addr,
        "len",
        vec![pushed.clone().into_val(env)]
    )));
    let res = runtime.invoke_contract(addr, "get", vec![pushed.into_val(env), 2u32.into_val(env)]);
    assert_eq!(Rec::try_from_val(env, &res).unwrap(), r2);

    let res = runtime.invoke_contract(addr, "pop", vec![arr.clone().into_val(env)]);
    assert_eq!(
        SVec::<Rec>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, r0.clone()]
    );

    let other: SVec<Rec> = soroban_sdk::vec![env, r2.clone()];
    let res = runtime.invoke_contract(
        addr,
        "merge",
        vec![arr.clone().into_val(env), other.into_val(env)],
    );
    assert_eq!(
        SVec::<Rec>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, r0.clone(), r1.clone(), r2]
    );

    let empty: SVec<Rec> = soroban_sdk::vec![env];
    let res = runtime.invoke_contract(addr, "merge", vec![arr.into_val(env), empty.into_val(env)]);
    assert_eq!(
        SVec::<Rec>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, r0, r1]
    );
}
