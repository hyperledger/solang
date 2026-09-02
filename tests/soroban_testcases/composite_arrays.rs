// SPDX-License-Identifier: Apache-2.0

use crate::build_solidity;
use soroban_sdk::{IntoVal, Val};

#[test]
fn array_of_structs_push_member_rw_pop() {
    let src = r#"
        contract c {
            struct Point { int32 x; int32 y; }
            Point[] pts;
            function push(int32 x, int32 y) public { pts.push(Point(x, y)); }
            function set_x(uint32 i, int32 v) public { pts[i].x = v; }
            function set_y(uint32 i, int32 v) public { pts[i].y = v; }
            function get_x(uint32 i) public view returns (int32) { return pts[i].x; }
            function get_y(uint32 i) public view returns (int32) { return pts[i].y; }
            function pop() public { pts.pop(); }
            function len() public view returns (uint32) { return uint32(pts.length); }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    runtime.invoke_contract(addr, "push", vec![1_i32.into_val(env), 2_i32.into_val(env)]);
    runtime.invoke_contract(addr, "push", vec![3_i32.into_val(env), 4_i32.into_val(env)]);
    runtime.invoke_contract(addr, "push", vec![5_i32.into_val(env), 6_i32.into_val(env)]);

    runtime.invoke_contract(
        addr,
        "set_x",
        vec![1_u32.into_val(env), 99_i32.into_val(env)],
    );

    let three: Val = 3_u32.into_val(env);
    assert!(three.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));

    for (getter, i, v) in [
        ("get_x", 0u32, 1i32),
        ("get_y", 0, 2),
        ("get_x", 1, 99),
        ("get_y", 1, 4),
        ("get_x", 2, 5),
        ("get_y", 2, 6),
    ] {
        let exp: Val = v.into_val(env);
        let got = runtime.invoke_contract(addr, getter, vec![i.into_val(env)]);
        assert!(exp.shallow_eq(&got), "{getter}({i}) should be {v}");
    }

    runtime.invoke_contract(addr, "pop", vec![]);
    let two: Val = 2_u32.into_val(env);
    assert!(two.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));
    let x0: Val = 1_i32.into_val(env);
    let y1: Val = 4_i32.into_val(env);
    assert!(x0.shallow_eq(&runtime.invoke_contract(addr, "get_x", vec![0_u32.into_val(env)])));
    assert!(y1.shallow_eq(&runtime.invoke_contract(addr, "get_y", vec![1_u32.into_val(env)])));
}

#[test]
fn array_of_structs_mixed_field_types() {
    let src = r#"
        contract c {
            struct Rec { uint64 id; int32 delta; bool active; }
            Rec[] recs;
            function push(uint64 id, int32 delta, bool active) public {
                recs.push(Rec(id, delta, active));
            }
            function set_active(uint32 i, bool v) public { recs[i].active = v; }
            function get_id(uint32 i) public view returns (uint64) { return recs[i].id; }
            function get_delta(uint32 i) public view returns (int32) { return recs[i].delta; }
            function get_active(uint32 i) public view returns (bool) { return recs[i].active; }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    runtime.invoke_contract(
        addr,
        "push",
        vec![
            10_u64.into_val(env),
            (-5_i32).into_val(env),
            false.into_val(env),
        ],
    );
    runtime.invoke_contract(
        addr,
        "push",
        vec![
            20_u64.into_val(env),
            7_i32.into_val(env),
            true.into_val(env),
        ],
    );
    runtime.invoke_contract(
        addr,
        "set_active",
        vec![0_u32.into_val(env), true.into_val(env)],
    );

    let id0: Val = 10_u64.into_val(env);
    let d0: Val = (-5_i32).into_val(env);
    let act0: Val = true.into_val(env);
    let id1: Val = 20_u64.into_val(env);
    let act1: Val = true.into_val(env);
    assert!(id0.shallow_eq(&runtime.invoke_contract(addr, "get_id", vec![0_u32.into_val(env)])));
    assert!(d0.shallow_eq(&runtime.invoke_contract(addr, "get_delta", vec![0_u32.into_val(env)])));
    assert!(act0.shallow_eq(&runtime.invoke_contract(
        addr,
        "get_active",
        vec![0_u32.into_val(env)]
    )));
    assert!(id1.shallow_eq(&runtime.invoke_contract(addr, "get_id", vec![1_u32.into_val(env)])));
    assert!(act1.shallow_eq(&runtime.invoke_contract(
        addr,
        "get_active",
        vec![1_u32.into_val(env)]
    )));
}

#[test]
fn array_of_addresses() {
    let src = r#"
        contract c {
            address[] addrs;
            function push(address a) public { addrs.push(a); }
            function set(uint32 i, address a) public { addrs[i] = a; }
            function get(uint32 i) public view returns (address) { return addrs[i]; }
            function len() public view returns (uint32) { return uint32(addrs.length); }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    use soroban_sdk::{testutils::Address as _, Address, FromVal};

    let a0 = Address::generate(env);
    let a1 = Address::generate(env);
    let a2 = Address::generate(env);
    runtime.invoke_contract(addr, "push", vec![a0.into_val(env)]);
    runtime.invoke_contract(addr, "push", vec![a1.into_val(env)]);
    runtime.invoke_contract(addr, "set", vec![0_u32.into_val(env), a2.into_val(env)]);

    let len: Val = 2_u32.into_val(env);
    assert!(len.shallow_eq(&runtime.invoke_contract(addr, "len", vec![])));
    let got0 = runtime.invoke_contract(addr, "get", vec![0_u32.into_val(env)]);
    assert!(Address::from_val(env, &got0) == a2);
    let got1 = runtime.invoke_contract(addr, "get", vec![1_u32.into_val(env)]);
    assert!(Address::from_val(env, &got1) == a1);
}

#[test]
fn array_of_arrays_nested_push() {
    let src = r#"
        contract c {
            uint64[][] m;
            function add_row() public { m.push(); }
            function push_into(uint32 i, uint64 v) public { m[i].push(v); }
            function set(uint32 i, uint32 j, uint64 v) public { m[i][j] = v; }
            function get(uint32 i, uint32 j) public view returns (uint64) { return m[i][j]; }
            function outer_len() public view returns (uint32) { return uint32(m.length); }
            function inner_len(uint32 i) public view returns (uint32) { return uint32(m[i].length); }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    runtime.invoke_contract(addr, "add_row", vec![]);
    runtime.invoke_contract(addr, "add_row", vec![]);
    runtime.invoke_contract(
        addr,
        "push_into",
        vec![0_u32.into_val(env), 10_u64.into_val(env)],
    );
    runtime.invoke_contract(
        addr,
        "push_into",
        vec![0_u32.into_val(env), 11_u64.into_val(env)],
    );
    runtime.invoke_contract(
        addr,
        "push_into",
        vec![1_u32.into_val(env), 20_u64.into_val(env)],
    );
    runtime.invoke_contract(
        addr,
        "set",
        vec![
            1_u32.into_val(env),
            0_u32.into_val(env),
            99_u64.into_val(env),
        ],
    );

    for (i, j, v) in [(0u32, 0u32, 10u64), (0, 1, 11), (1, 0, 99)] {
        let exp: Val = v.into_val(env);
        let got = runtime.invoke_contract(addr, "get", vec![i.into_val(env), j.into_val(env)]);
        assert!(exp.shallow_eq(&got), "m[{i}][{j}] should be {v}");
    }
}

#[test]
fn struct_of_arrays_field_push() {
    let src = r#"
        contract c {
            struct S { uint64[] a; uint64 z; }
            S s;
            function push_a(uint64 v) public { s.a.push(v); }
            function set_a(uint32 i, uint64 v) public { s.a[i] = v; }
            function set_z(uint64 v) public { s.z = v; }
            function get_a(uint32 i) public view returns (uint64) { return s.a[i]; }
            function get_z() public view returns (uint64) { return s.z; }
            function alen() public view returns (uint32) { return uint32(s.a.length); }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    runtime.invoke_contract(addr, "push_a", vec![7_u64.into_val(env)]);
    runtime.invoke_contract(addr, "push_a", vec![8_u64.into_val(env)]);
    runtime.invoke_contract(addr, "set_z", vec![5_u64.into_val(env)]);

    let a0: Val = 7_u64.into_val(env);
    let a1: Val = 8_u64.into_val(env);
    let z: Val = 5_u64.into_val(env);
    assert!(a0.shallow_eq(&runtime.invoke_contract(addr, "get_a", vec![0_u32.into_val(env)])));
    assert!(a1.shallow_eq(&runtime.invoke_contract(addr, "get_a", vec![1_u32.into_val(env)])));
    assert!(z.shallow_eq(&runtime.invoke_contract(addr, "get_z", vec![])));
}
