// SPDX-License-Identifier: Apache-2.0

use crate::build_solidity;
use soroban_sdk::{contracttype, vec as svec, Bytes, FromVal, IntoVal, String, Vec};

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inner {
    pub x: i64,
    pub y: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outer {
    pub inner: Inner,
    pub tag: u32,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct C3 {
    pub v: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct B3 {
    pub c: C3,
    pub k: u32,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct A3 {
    pub b: B3,
    pub z: i32,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Info {
    pub name: String,
    pub data: Bytes,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Wrapper {
    pub info: Info,
    pub id: u32,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct P {
    pub x: i64,
    pub y: i64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seg {
    pub a: P,
    pub b: P,
}

#[test]
fn multi_field_nested_roundtrip() {
    let runtime = build_solidity(
        r#"
        contract test {
            struct Inner { int64 x; uint64 y; }
            struct Outer { Inner inner; uint32 tag; }

            function make() public pure returns (Outer memory) {
                return Outer(Inner(-7, 8), 100);
            }
            function echo(Outer memory o) public pure returns (Outer memory) {
                return o;
            }
            function get_x(Outer memory o) public pure returns (int64) {
                return o.inner.x;
            }
            function bump(Outer memory o) public pure returns (Outer memory) {
                o.inner.x = o.inner.x + 1;
                o.inner.y = o.inner.y + 2;
                o.tag = o.tag + 3;
                return o;
            }
            function via_local(Outer memory o) public pure returns (Outer memory) {
                Inner memory i = o.inner;
                return Outer(i, o.tag);
            }
        }
        "#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let res = runtime.invoke_contract(addr, "make", vec![]);
    assert_eq!(
        Outer::from_val(env, &res),
        Outer {
            inner: Inner { x: -7, y: 8 },
            tag: 100,
        }
    );

    let input = Outer {
        inner: Inner {
            x: -1_234_567,
            y: 0x0123_4567_89AB_CDEF,
        },
        tag: 42,
    };
    let res = runtime.invoke_contract(addr, "echo", vec![input.clone().into_val(env)]);
    assert_eq!(Outer::from_val(env, &res), input);

    let res = runtime.invoke_contract(addr, "get_x", vec![input.clone().into_val(env)]);
    assert_eq!(i64::from_val(env, &res), -1_234_567);

    let res = runtime.invoke_contract(addr, "bump", vec![input.clone().into_val(env)]);
    assert_eq!(
        Outer::from_val(env, &res),
        Outer {
            inner: Inner {
                x: -1_234_566,
                y: 0x0123_4567_89AB_CDF1,
            },
            tag: 45,
        }
    );

    let res = runtime.invoke_contract(addr, "via_local", vec![input.clone().into_val(env)]);
    assert_eq!(Outer::from_val(env, &res), input);
}

#[test]
fn three_level_nested_roundtrip() {
    let runtime = build_solidity(
        r#"
        contract test {
            struct C { uint64 v; }
            struct B { C c; uint32 k; }
            struct A { B b; int32 z; }

            function make() public pure returns (A memory) {
                return A(B(C(42), 7), -3);
            }
            function echo(A memory a) public pure returns (A memory) {
                return a;
            }
            function deep(A memory a) public pure returns (uint64) {
                return a.b.c.v;
            }
            function bump_deep(A memory a) public pure returns (A memory) {
                a.b.c.v = a.b.c.v + 100;
                return a;
            }
        }
        "#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let res = runtime.invoke_contract(addr, "make", vec![]);
    assert_eq!(
        A3::from_val(env, &res),
        A3 {
            b: B3 {
                c: C3 { v: 42 },
                k: 7,
            },
            z: -3,
        }
    );

    let input = A3 {
        b: B3 {
            c: C3 { v: 9_999_999_999 },
            k: 123,
        },
        z: -777,
    };
    let res = runtime.invoke_contract(addr, "echo", vec![input.clone().into_val(env)]);
    assert_eq!(A3::from_val(env, &res), input);

    let res = runtime.invoke_contract(addr, "deep", vec![input.clone().into_val(env)]);
    assert_eq!(u64::from_val(env, &res), 9_999_999_999);

    let res = runtime.invoke_contract(addr, "bump_deep", vec![input.clone().into_val(env)]);
    assert_eq!(
        A3::from_val(env, &res),
        A3 {
            b: B3 {
                c: C3 { v: 10_000_000_099 },
                k: 123,
            },
            z: -777,
        }
    );
}

#[test]
fn nested_struct_with_dynamic_fields() {
    let runtime = build_solidity(
        r#"
        contract test {
            struct Info { string name; bytes data; }
            struct Wrapper { Info info; uint32 id; }

            function make() public pure returns (Wrapper memory) {
                return Wrapper(Info("hi", hex"0102"), 5);
            }
            function echo(Wrapper memory w) public pure returns (Wrapper memory) {
                return w;
            }
            function rename(Wrapper memory w, string memory n) public pure returns (Wrapper memory) {
                w.info.name = n;
                return w;
            }
            function grow(Wrapper memory w) public pure returns (Wrapper memory) {
                bytes memory b = w.info.data;
                b.push(0xFF);
                w.info.data = b;
                return w;
            }
            function name_len(Wrapper memory w) public pure returns (uint64) {
                return uint64(bytes(w.info.name).length);
            }
        }
        "#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    let s = |t: &str| String::from_str(env, t);
    let b = |bytes: &[u8]| Bytes::from_slice(env, bytes);

    let res = runtime.invoke_contract(addr, "make", vec![]);
    assert_eq!(
        Wrapper::from_val(env, &res),
        Wrapper {
            info: Info {
                name: s("hi"),
                data: b(&[0x01, 0x02]),
            },
            id: 5,
        }
    );

    let input = Wrapper {
        info: Info {
            name: s("Solang"),
            data: b(&[0xAA, 0xBB, 0xCC]),
        },
        id: 77,
    };
    let res = runtime.invoke_contract(addr, "echo", vec![input.clone().into_val(env)]);
    assert_eq!(Wrapper::from_val(env, &res), input);

    let res = runtime.invoke_contract(addr, "name_len", vec![input.clone().into_val(env)]);
    assert_eq!(u64::from_val(env, &res), 6);

    let res = runtime.invoke_contract(
        addr,
        "rename",
        vec![input.clone().into_val(env), s("Stellar").into_val(env)],
    );
    assert_eq!(
        Wrapper::from_val(env, &res),
        Wrapper {
            info: Info {
                name: s("Stellar"),
                data: b(&[0xAA, 0xBB, 0xCC]),
            },
            id: 77,
        }
    );

    let res = runtime.invoke_contract(addr, "grow", vec![input.into_val(env)]);
    assert_eq!(
        Wrapper::from_val(env, &res),
        Wrapper {
            info: Info {
                name: s("Solang"),
                data: b(&[0xAA, 0xBB, 0xCC, 0xFF]),
            },
            id: 77,
        }
    );
}

#[test]
fn sibling_nested_structs() {
    let runtime = build_solidity(
        r#"
        contract test {
            struct P { int64 x; int64 y; }
            struct Seg { P a; P b; }

            function make() public pure returns (Seg memory) {
                return Seg(P(0, 0), P(3, 4));
            }
            function echo(Seg memory s) public pure returns (Seg memory) {
                return s;
            }
            function swap(Seg memory s) public pure returns (Seg memory) {
                int64 ax = s.a.x;
                int64 ay = s.a.y;
                s.a.x = s.b.x;
                s.a.y = s.b.y;
                s.b.x = ax;
                s.b.y = ay;
                return s;
            }
            function dx(Seg memory s) public pure returns (int64) {
                return s.b.x - s.a.x;
            }
        }
        "#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let res = runtime.invoke_contract(addr, "make", vec![]);
    assert_eq!(
        Seg::from_val(env, &res),
        Seg {
            a: P { x: 0, y: 0 },
            b: P { x: 3, y: 4 },
        }
    );

    let input = Seg {
        a: P { x: -10, y: -20 },
        b: P { x: 100, y: 200 },
    };
    let res = runtime.invoke_contract(addr, "echo", vec![input.clone().into_val(env)]);
    assert_eq!(Seg::from_val(env, &res), input);

    let res = runtime.invoke_contract(addr, "dx", vec![input.clone().into_val(env)]);
    assert_eq!(i64::from_val(env, &res), 110);

    let res = runtime.invoke_contract(addr, "swap", vec![input.into_val(env)]);
    assert_eq!(
        Seg::from_val(env, &res),
        Seg {
            a: P { x: 100, y: 200 },
            b: P { x: -10, y: -20 },
        }
    );
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SArr {
    pub xs: Vec<u64>,
    pub tag: u32,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct D4 {
    pub v: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct C4 {
    pub d: D4,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct B4 {
    pub c: C4,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct A4 {
    pub b: B4,
}

#[test]
fn array_field_in_struct() {
    let runtime = build_solidity(
        r#"
        contract test {
            struct S { uint64[] xs; uint32 tag; }

            function make() public pure returns (S memory) {
                uint64[] memory a = new uint64[](3);
                a[0] = 10; a[1] = 20; a[2] = 30;
                return S(a, 7);
            }
            function echo(S memory s) public pure returns (S memory) {
                return s;
            }
            function len(S memory s) public pure returns (uint32) {
                return uint32(s.xs.length);
            }
            function at(S memory s, uint32 i) public pure returns (uint64) {
                return s.xs[i];
            }
            function sum(S memory s) public pure returns (uint64) {
                uint64 t = 0;
                for (uint32 i = 0; i < s.xs.length; i++) {
                    t += s.xs[i];
                }
                return t;
            }
            function bump(S memory s) public pure returns (S memory) {
                for (uint32 i = 0; i < s.xs.length; i++) {
                    s.xs[i] = s.xs[i] + 1;
                }
                s.tag = s.tag + 100;
                return s;
            }
        }
        "#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let res = runtime.invoke_contract(addr, "make", vec![]);
    assert_eq!(
        SArr::from_val(env, &res),
        SArr {
            xs: svec![env, 10, 20, 30],
            tag: 7,
        }
    );

    let input = SArr {
        xs: svec![env, 100, 200, 300, 400],
        tag: 42,
    };
    let res = runtime.invoke_contract(addr, "echo", vec![input.clone().into_val(env)]);
    assert_eq!(SArr::from_val(env, &res), input);

    let res = runtime.invoke_contract(addr, "len", vec![input.clone().into_val(env)]);
    assert_eq!(u32::from_val(env, &res), 4);

    let res = runtime.invoke_contract(
        addr,
        "at",
        vec![input.clone().into_val(env), 2u32.into_val(env)],
    );
    assert_eq!(u64::from_val(env, &res), 300);

    let res = runtime.invoke_contract(addr, "sum", vec![input.clone().into_val(env)]);
    assert_eq!(u64::from_val(env, &res), 1000);

    let res = runtime.invoke_contract(addr, "bump", vec![input.into_val(env)]);
    assert_eq!(
        SArr::from_val(env, &res),
        SArr {
            xs: svec![env, 101, 201, 301, 401],
            tag: 142,
        }
    );
}

#[test]
fn four_level_nested_roundtrip() {
    let runtime = build_solidity(
        r#"
        contract test {
            struct D { uint64 v; }
            struct C { D d; }
            struct B { C c; }
            struct A { B b; }

            function make() public pure returns (A memory) {
                return A(B(C(D(42))));
            }
            function echo(A memory a) public pure returns (A memory) {
                return a;
            }
            function deep(A memory a) public pure returns (uint64) {
                return a.b.c.d.v;
            }
            function bump_deep(A memory a) public pure returns (A memory) {
                a.b.c.d.v = a.b.c.d.v + 1000;
                return a;
            }
        }
        "#,
        |_| {},
    );
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let deepest = |v: u64| A4 {
        b: B4 {
            c: C4 { d: D4 { v } },
        },
    };

    let res = runtime.invoke_contract(addr, "make", vec![]);
    assert_eq!(A4::from_val(env, &res), deepest(42));

    let input = deepest(9_876_543_210);
    let res = runtime.invoke_contract(addr, "echo", vec![input.clone().into_val(env)]);
    assert_eq!(A4::from_val(env, &res), input);

    let res = runtime.invoke_contract(addr, "deep", vec![input.clone().into_val(env)]);
    assert_eq!(u64::from_val(env, &res), 9_876_543_210);

    let res = runtime.invoke_contract(addr, "bump_deep", vec![input.into_val(env)]);
    assert_eq!(A4::from_val(env, &res), deepest(9_876_544_210));
}
