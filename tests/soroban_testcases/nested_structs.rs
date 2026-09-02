// SPDX-License-Identifier: Apache-2.0

use crate::build_solidity;
use soroban_sdk::{IntoVal, Val};

const VERY_COMPLEX_SRC: &str = r#"
    contract deep {
        struct Coord  { int32 lat; int32 lon; }
        struct Node   { Coord pos; uint64 id; bool active; }
        struct Branch { Node a; Node b; int32 weight; }
        struct Tree   { Branch left; Branch right; uint64 root_id; }
        struct Forest { Tree north; Tree south; bool planted; }

        Forest forest;

        function set_nla_lat(int32 v)   public { forest.north.left.a.pos.lat = v; } // d5
        function set_nla_lon(int32 v)   public { forest.north.left.a.pos.lon = v; } // d5
        function set_nla_id(uint64 v)   public { forest.north.left.a.id = v; }      // d4
        function set_nla_active(bool v) public { forest.north.left.a.active = v; }  // d4
        function set_nlb_lat(int32 v)   public { forest.north.left.b.pos.lat = v; } // d5 (sibling node)
        function set_nl_weight(int32 v) public { forest.north.left.weight = v; }    // d3
        function set_nra_lat(int32 v)   public { forest.north.right.a.pos.lat = v; }// d5 (sibling branch)
        function set_n_rootid(uint64 v) public { forest.north.root_id = v; }        // d2
        function set_sla_lon(int32 v)   public { forest.south.left.a.pos.lon = v; } // d5 (sibling tree)
        function set_planted(bool v)    public { forest.planted = v; }              // d1

        function get_nla_lat()   public view returns (int32)  { return forest.north.left.a.pos.lat; }
        function get_nla_lon()   public view returns (int32)  { return forest.north.left.a.pos.lon; }
        function get_nla_id()    public view returns (uint64) { return forest.north.left.a.id; }
        function get_nla_active() public view returns (bool)  { return forest.north.left.a.active; }
        function get_nlb_lat()   public view returns (int32)  { return forest.north.left.b.pos.lat; }
        function get_nl_weight() public view returns (int32)  { return forest.north.left.weight; }
        function get_nra_lat()   public view returns (int32)  { return forest.north.right.a.pos.lat; }
        function get_n_rootid()  public view returns (uint64) { return forest.north.root_id; }
        function get_sla_lon()   public view returns (int32)  { return forest.south.left.a.pos.lon; }
        function get_planted()   public view returns (bool)   { return forest.planted; }
    }
"#;

const COMPLEX_SRC: &str = r#"
    contract complex {
        struct Leaf { int32 x; int32 y; }
        struct Mid  { Leaf p; Leaf q; uint64 tag; }
        struct Root { Mid l; Mid r; bool flag; }

        Root root;

        function set_lpx(int32 v)  public { root.l.p.x = v; }   // depth 3
        function set_lpy(int32 v)  public { root.l.p.y = v; }   // depth 3
        function set_lqx(int32 v)  public { root.l.q.x = v; }   // depth 3
        function set_rpx(int32 v)  public { root.r.p.x = v; }   // depth 3
        function set_ltag(uint64 v) public { root.l.tag = v; }  // depth 2
        function set_flag(bool v)  public { root.flag = v; }    // depth 1

        function get_lpx()  public view returns (int32)  { return root.l.p.x; }
        function get_lpy()  public view returns (int32)  { return root.l.p.y; }
        function get_lqx()  public view returns (int32)  { return root.l.q.x; }
        function get_rpx()  public view returns (int32)  { return root.r.p.x; }
        function get_ltag() public view returns (uint64) { return root.l.tag; }
        function get_flag() public view returns (bool)   { return root.flag; }
    }
"#;

const SRC: &str = r#"
    contract nested {
        struct S1 { int32 a; }
        struct S2 { S1 inner; }
        struct S3 { S2 inner; }

        int32 d0;
        S1 s1;
        S2 s2;
        S3 s3;

        function set0(int32 v) public { d0 = v; }
        function get0() public view returns (int32) { return d0; }

        function set1(int32 v) public { s1.a = v; }
        function get1() public view returns (int32) { return s1.a; }

        function set2(int32 v) public { s2.inner.a = v; }
        function get2() public view returns (int32) { return s2.inner.a; }

        function set3(int32 v) public { s3.inner.inner.a = v; }
        function get3() public view returns (int32) { return s3.inner.inner.a; }
    }
"#;

fn roundtrip(depth: u32, value: i32) {
    let runtime = build_solidity(SRC, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let setter = format!("set{depth}");
    let getter = format!("get{depth}");

    runtime.invoke_contract(addr, &setter, vec![value.into_val(env)]);
    let got = runtime.invoke_contract(addr, &getter, vec![]);
    let expected: Val = value.into_val(env);
    assert!(
        expected.shallow_eq(&got),
        "depth {depth}: expected {value}, got a different value"
    );
}

#[test]
fn nested_struct_depth0() {
    roundtrip(0, 100);
}

#[test]
fn nested_struct_depth1() {
    roundtrip(1, 101);
}

#[test]
fn nested_struct_depth2() {
    roundtrip(2, 102);
}

#[test]
fn nested_struct_depth3() {
    roundtrip(3, 103);
}

#[test]
fn complex_nested_struct_isolation() {
    let runtime = build_solidity(COMPLEX_SRC, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    runtime.invoke_contract(addr, "set_lpx", vec![11_i32.into_val(env)]);
    runtime.invoke_contract(addr, "set_lpy", vec![22_i32.into_val(env)]);
    runtime.invoke_contract(addr, "set_lqx", vec![33_i32.into_val(env)]);
    runtime.invoke_contract(addr, "set_rpx", vec![44_i32.into_val(env)]);
    runtime.invoke_contract(addr, "set_ltag", vec![555_u64.into_val(env)]);
    runtime.invoke_contract(addr, "set_flag", vec![true.into_val(env)]);

    let checks: &[(&str, Val)] = &[
        ("get_lpx", 11_i32.into_val(env)),
        ("get_lpy", 22_i32.into_val(env)),
        ("get_lqx", 33_i32.into_val(env)),
        ("get_rpx", 44_i32.into_val(env)),
        ("get_ltag", 555_u64.into_val(env)),
        ("get_flag", true.into_val(env)),
    ];
    for (getter, expected) in checks {
        let got = runtime.invoke_contract(addr, getter, vec![]);
        assert!(expected.shallow_eq(&got), "{getter} lost its value");
    }
}

#[test]
fn very_complex_nested_struct_leaves() {
    let runtime = build_solidity(VERY_COMPLEX_SRC, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    runtime.invoke_contract(addr, "set_nla_lat", vec![1001_i32.into_val(env)]);
    runtime.invoke_contract(addr, "set_nla_lon", vec![1002_i32.into_val(env)]);
    runtime.invoke_contract(addr, "set_nla_id", vec![2001_u64.into_val(env)]);
    runtime.invoke_contract(addr, "set_nla_active", vec![true.into_val(env)]);
    runtime.invoke_contract(addr, "set_nlb_lat", vec![1003_i32.into_val(env)]);
    runtime.invoke_contract(addr, "set_nl_weight", vec![3001_i32.into_val(env)]);
    runtime.invoke_contract(addr, "set_nra_lat", vec![1004_i32.into_val(env)]);
    runtime.invoke_contract(addr, "set_n_rootid", vec![4001_u64.into_val(env)]);
    runtime.invoke_contract(addr, "set_sla_lon", vec![1005_i32.into_val(env)]);
    runtime.invoke_contract(addr, "set_planted", vec![true.into_val(env)]);

    let checks: &[(&str, Val)] = &[
        ("get_nla_lat", 1001_i32.into_val(env)),
        ("get_nla_lon", 1002_i32.into_val(env)),
        ("get_nla_id", 2001_u64.into_val(env)),
        ("get_nla_active", true.into_val(env)),
        ("get_nlb_lat", 1003_i32.into_val(env)),
        ("get_nl_weight", 3001_i32.into_val(env)),
        ("get_nra_lat", 1004_i32.into_val(env)),
        ("get_n_rootid", 4001_u64.into_val(env)),
        ("get_sla_lon", 1005_i32.into_val(env)),
        ("get_planted", true.into_val(env)),
    ];
    for (getter, expected) in checks {
        let got = runtime.invoke_contract(addr, getter, vec![]);
        assert!(expected.shallow_eq(&got), "{getter} lost its value");
    }

    runtime.invoke_contract(addr, "set_nla_lat", vec![99_i32.into_val(env)]);
    runtime.invoke_contract(addr, "set_planted", vec![false.into_val(env)]);

    let checks: &[(&str, Val)] = &[
        ("get_nla_lat", 99_i32.into_val(env)),
        ("get_nla_lon", 1002_i32.into_val(env)),
        ("get_nla_id", 2001_u64.into_val(env)),
        ("get_nla_active", true.into_val(env)),
        ("get_nlb_lat", 1003_i32.into_val(env)),
        ("get_nl_weight", 3001_i32.into_val(env)),
        ("get_nra_lat", 1004_i32.into_val(env)),
        ("get_n_rootid", 4001_u64.into_val(env)),
        ("get_sla_lon", 1005_i32.into_val(env)),
        ("get_planted", false.into_val(env)),
    ];
    for (getter, expected) in checks {
        let got = runtime.invoke_contract(addr, getter, vec![]);
        assert!(expected.shallow_eq(&got), "{getter} lost its value");
    }
}

#[test]
fn flat_struct_init_then_mutate() {
    let src = r#"
        contract c {
            struct S1 { int32 a; int32 b; }
            S1 s1;
            function init() public { s1 = S1(5, 6); }
            function set_a(int32 v) public { s1.a = v; }
            function get_a() public view returns (int32) { return s1.a; }
            function get_b() public view returns (int32) { return s1.b; }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    runtime.invoke_contract(addr, "init", vec![]);
    let a: Val = 5_i32.into_val(env);
    let b: Val = 6_i32.into_val(env);
    assert!(a.shallow_eq(&runtime.invoke_contract(addr, "get_a", vec![])));
    assert!(b.shallow_eq(&runtime.invoke_contract(addr, "get_b", vec![])));

    runtime.invoke_contract(addr, "set_a", vec![50_i32.into_val(env)]);
    let a2: Val = 50_i32.into_val(env);
    assert!(a2.shallow_eq(&runtime.invoke_contract(addr, "get_a", vec![])));
    assert!(b.shallow_eq(&runtime.invoke_contract(addr, "get_b", vec![])));

    runtime.invoke_contract(addr, "set_a", vec![99_i32.into_val(env)]);
    let a3: Val = 99_i32.into_val(env);
    assert!(a3.shallow_eq(&runtime.invoke_contract(addr, "get_a", vec![])));
    assert!(b.shallow_eq(&runtime.invoke_contract(addr, "get_b", vec![])));
}

#[test]
fn nested_field_init_via_flat_substruct() {
    let src = r#"
        contract c {
            struct S1 { int32 a; int32 b; }
            struct S2 { S1 inner; int32 z; }
            S2 s2;
            function init_inner() public { s2.inner = S1(7, 8); }
            function set_z(int32 v) public { s2.z = v; }
            function get_a() public view returns (int32) { return s2.inner.a; }
            function get_b() public view returns (int32) { return s2.inner.b; }
            function get_z() public view returns (int32) { return s2.z; }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    runtime.invoke_contract(addr, "init_inner", vec![]);
    runtime.invoke_contract(addr, "set_z", vec![9_i32.into_val(env)]);

    let a: Val = 7_i32.into_val(env);
    let b: Val = 8_i32.into_val(env);
    let z: Val = 9_i32.into_val(env);
    assert!(a.shallow_eq(&runtime.invoke_contract(addr, "get_a", vec![])));
    assert!(b.shallow_eq(&runtime.invoke_contract(addr, "get_b", vec![])));
    assert!(z.shallow_eq(&runtime.invoke_contract(addr, "get_z", vec![])));
}

#[test]
fn nested_whole_literal_init() {
    let src = r#"
        contract c {
            struct S1 { int32 a; int32 b; }
            struct S2 { S1 inner; int32 z; }
            S2 s2;
            function init() public { s2 = S2(S1(5, 6), 7); }
            function get_a() public view returns (int32) { return s2.inner.a; }
            function get_b() public view returns (int32) { return s2.inner.b; }
            function get_z() public view returns (int32) { return s2.z; }
            // whole-struct load round-trip (exercises decode_struct_storage)
            function get_all() public view returns (S2 memory) { return s2; }
        }
    "#;
    let runtime = build_solidity(src, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    runtime.invoke_contract(addr, "init", vec![]);
    let five: Val = 5_i32.into_val(env);
    let six: Val = 6_i32.into_val(env);
    let seven: Val = 7_i32.into_val(env);
    assert!(five.shallow_eq(&runtime.invoke_contract(addr, "get_a", vec![])));
    assert!(six.shallow_eq(&runtime.invoke_contract(addr, "get_b", vec![])));
    assert!(seven.shallow_eq(&runtime.invoke_contract(addr, "get_z", vec![])));
    runtime.invoke_contract(addr, "get_all", vec![]);
}
