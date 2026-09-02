// SPDX-License-Identifier: Apache-2.0

use crate::build_solidity;
use soroban_sdk::{contracttype, IntoVal, TryFromVal, Vec as SVec};

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Point {
    pub x: i64,
    pub y: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pair {
    pub a: u32,
    pub b: u32,
}

#[test]
fn struct_array_roundtrip_and_build() {
    let runtime = build_solidity(
        r#"
        contract c {
            struct Point { int64 x; uint64 y; }

            function echo(Point[] memory a) public pure returns (Point[] memory) {
                return a;
            }

            function alias_return(Point[] memory a) public pure returns (Point[] memory) {
                Point[] memory b = a;
                return b;
            }

            function build() public pure returns (Point[] memory) {
                Point[] memory out = new Point[](3);
                out[0] = Point(-1, 2);
                out[1] = Point(0, 0);
                out[2] = Point(1000, 5);
                return out;
            }

            function sum_x(Point[] memory a) public pure returns (int64) {
                int64 s = 0;
                for (uint32 i = 0; i < a.length; i++) {
                    s += a[i].x;
                }
                return s;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let input: SVec<Point> = soroban_sdk::vec![
        env,
        Point { x: -1, y: 2 },
        Point { x: 0, y: 0 },
        Point { x: 1000, y: 5 },
    ];

    let res = runtime.invoke_contract(addr, "echo", vec![input.clone().into_val(env)]);
    assert_eq!(SVec::<Point>::try_from_val(env, &res).unwrap(), input);

    let res = runtime.invoke_contract(addr, "alias_return", vec![input.clone().into_val(env)]);
    assert_eq!(SVec::<Point>::try_from_val(env, &res).unwrap(), input);

    let res = runtime.invoke_contract(addr, "sum_x", vec![input.into_val(env)]);
    let expected: soroban_sdk::Val = 999_i64.into_val(env);
    assert!(expected.shallow_eq(&res));

    let res = runtime.invoke_contract(addr, "build", vec![]);
    let got = SVec::<Point>::try_from_val(env, &res).unwrap();
    assert_eq!(
        got,
        soroban_sdk::vec![
            env,
            Point { x: -1, y: 2 },
            Point { x: 0, y: 0 },
            Point { x: 1000, y: 5 },
        ]
    );
}

#[test]
fn struct_array_storage_assign_update_return() {
    let runtime = build_solidity(
        r#"
        contract c {
            struct Point { int64 x; uint64 y; }

            Point[] stored;

            function set(Point[] memory a) public {
                stored = a;
            }

            function get() public view returns (Point[] memory) {
                return stored;
            }

            function len() public view returns (uint32) {
                return uint32(stored.length);
            }

            function make() internal pure returns (Point[] memory) {
                Point[] memory m = new Point[](2);
                m[0] = Point(11, 12);
                m[1] = Point(13, 14);
                return m;
            }
            function set_from_return() public {
                stored = make();
            }

            function replace_from_local() public {
                Point[] memory local = new Point[](2);
                local[0] = Point(7, 8);
                local[1] = Point(9, 10);
                stored = local;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let input: SVec<Point> = soroban_sdk::vec![
        env,
        Point { x: -1, y: 2 },
        Point { x: 3, y: 4 },
        Point { x: 5, y: 6 },
    ];
    runtime.invoke_contract(addr, "set", vec![input.clone().into_val(env)]);

    let res = runtime.invoke_contract(addr, "get", vec![]);
    assert_eq!(SVec::<Point>::try_from_val(env, &res).unwrap(), input);

    let three: soroban_sdk::Val = 3_u32.into_val(env);
    assert!(three.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));

    runtime.invoke_contract(addr, "set_from_return", vec![]);
    let res = runtime.invoke_contract(addr, "get", vec![]);
    assert_eq!(
        SVec::<Point>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, Point { x: 11, y: 12 }, Point { x: 13, y: 14 }]
    );

    runtime.invoke_contract(addr, "replace_from_local", vec![]);
    let res = runtime.invoke_contract(addr, "get", vec![]);
    assert_eq!(
        SVec::<Point>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![env, Point { x: 7, y: 8 }, Point { x: 9, y: 10 }]
    );
}

#[test]
fn struct_array_storage_element_access_after_assign() {
    let runtime = build_solidity(
        r#"
        contract c {
            struct Point { int64 x; uint64 y; }

            Point[] stored;

            function set(Point[] memory a) public {
                stored = a;
            }

            // whole-array read (ABI Map form) — must coexist with element access below
            function get() public view returns (Point[] memory) {
                return stored;
            }

            // per-element field read from a whole-assigned struct[] storage array
            function at_x(uint32 i) public view returns (int64) {
                return stored[i].x;
            }

            // whole-element read
            function at(uint32 i) public view returns (Point memory) {
                return stored[i];
            }

            // in-place field update of a stored element
            function bump_x(uint32 i, int64 dx) public {
                stored[i].x += dx;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let input: SVec<Point> = soroban_sdk::vec![
        env,
        Point { x: -1, y: 2 },
        Point { x: 3, y: 4 },
        Point { x: 5, y: 6 },
    ];
    runtime.invoke_contract(addr, "set", vec![input.clone().into_val(env)]);

    let res = runtime.invoke_contract(addr, "at_x", vec![2_u32.into_val(env)]);
    let five: soroban_sdk::Val = 5_i64.into_val(env);
    assert!(five.shallow_eq(&res));

    let res = runtime.invoke_contract(addr, "at", vec![1_u32.into_val(env)]);
    assert_eq!(
        Point::try_from_val(env, &res).unwrap(),
        Point { x: 3, y: 4 }
    );

    let res = runtime.invoke_contract(addr, "get", vec![]);
    assert_eq!(SVec::<Point>::try_from_val(env, &res).unwrap(), input);

    runtime.invoke_contract(
        addr,
        "bump_x",
        vec![1_u32.into_val(env), 100_i64.into_val(env)],
    );
    let res = runtime.invoke_contract(addr, "at_x", vec![1_u32.into_val(env)]);
    let expected: soroban_sdk::Val = 103_i64.into_val(env);
    assert!(expected.shallow_eq(&res));

    let res = runtime.invoke_contract(addr, "get", vec![]);
    assert_eq!(
        SVec::<Point>::try_from_val(env, &res).unwrap(),
        soroban_sdk::vec![
            env,
            Point { x: -1, y: 2 },
            Point { x: 103, y: 4 },
            Point { x: 5, y: 6 },
        ]
    );
}

#[test]
fn struct_array_mutate_elements() {
    let runtime = build_solidity(
        r#"
        contract c {
            struct Pair { uint32 a; uint32 b; }

            function swap_all(Pair[] memory a) public pure returns (Pair[] memory) {
                Pair[] memory out = new Pair[](a.length);
                for (uint32 i = 0; i < a.length; i++) {
                    out[i] = Pair(a[i].b, a[i].a);
                }
                return out;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let input: SVec<Pair> = soroban_sdk::vec![
        env,
        Pair { a: 1, b: 2 },
        Pair { a: 10, b: 20 },
        Pair { a: 100, b: 200 },
    ];

    let res = runtime.invoke_contract(addr, "swap_all", vec![input.into_val(env)]);
    let got = SVec::<Pair>::try_from_val(env, &res).unwrap();
    assert_eq!(
        got,
        soroban_sdk::vec![
            env,
            Pair { a: 2, b: 1 },
            Pair { a: 20, b: 10 },
            Pair { a: 200, b: 100 },
        ]
    );
}
