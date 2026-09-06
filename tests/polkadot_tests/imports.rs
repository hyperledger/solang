// SPDX-License-Identifier: Apache-2.0

use solang::file_resolver::FileResolver;
use solang::Target;
use solang_parser::pt::Loc;
use std::ffi::OsStr;

#[test]
fn enum_import() {
    let mut cache = FileResolver::default();

    cache.set_file_contents(
        "a.sol",
        r#"
        import "b.sol";

        abstract contract foo {
            enum_b bar;
        }
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "b.sol",
        r#"
        enum enum_b { b1 }
        "#
        .to_string(),
    );

    let ns = solang::parse_and_resolve(OsStr::new("a.sol"), &mut cache, Target::default_polkadot());

    assert!(!ns.diagnostics.any_errors());

    let mut cache = FileResolver::default();

    cache.set_file_contents(
        "a.sol",
        r#"
        import { enum_b } from "b.sol";

        abstract contract foo {
            enum_b bar;
        }
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "b.sol",
        r#"
        enum enum_b { b1 }
        "#
        .to_string(),
    );

    let ns = solang::parse_and_resolve(OsStr::new("a.sol"), &mut cache, Target::default_polkadot());

    assert!(!ns.diagnostics.any_errors());

    let mut cache = FileResolver::default();

    cache.set_file_contents(
        "a.sol",
        r#"
        import { enum_b as foobar } from "b.sol";

        abstract contract foo {
            foobar bar;
        }
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "b.sol",
        r#"
        enum enum_b { b1 }
        "#
        .to_string(),
    );

    let ns = solang::parse_and_resolve(OsStr::new("a.sol"), &mut cache, Target::default_polkadot());

    assert!(!ns.diagnostics.any_errors());

    let mut cache = FileResolver::default();

    cache.set_file_contents(
        "a.sol",
        r#"
        import { enum_c } from "b.sol";
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "b.sol",
        r#"
        enum enum_b { b1 }
        "#
        .to_string(),
    );

    let ns = solang::parse_and_resolve(OsStr::new("a.sol"), &mut cache, Target::default_polkadot());

    assert_eq!(
        ns.diagnostics.first_error(),
        "import 'b.sol' does not export 'enum_c'"
    );

    // from has special handling to avoid making it a keyword
    let mut cache = FileResolver::default();

    cache.set_file_contents(
        "a.sol",
        r#"
        import { enum_c } frum "b.sol";
        "#
        .to_string(),
    );

    let ns = solang::parse_and_resolve(OsStr::new("a.sol"), &mut cache, Target::default_polkadot());

    assert_eq!(
        ns.diagnostics.first_error(),
        "'frum' found where 'from' expected"
    );

    let mut cache = FileResolver::default();

    cache.set_file_contents(
        "a.sol",
        r#"
        import * as foo frum "b.sol";
        "#
        .to_string(),
    );

    let ns = solang::parse_and_resolve(OsStr::new("a.sol"), &mut cache, Target::default_polkadot());

    assert_eq!(
        ns.diagnostics.first_error(),
        "'frum' found where 'from' expected"
    );
}

#[test]
fn builtin_import_definition_note_uses_import_location() {
    let mut cache = FileResolver::default();

    cache.set_file_contents(
        "a.sol",
        r#"
        import "polkadot";

        contract Foo {
            function chain_extension() public pure {}
        }
        "#
        .to_string(),
    );

    let ns = solang::parse_and_resolve(OsStr::new("a.sol"), &mut cache, Target::default_polkadot());

    assert_eq!(
        ns.diagnostics.first_error(),
        "chain_extension is already defined as a builtin function"
    );

    let diagnostics = ns.diagnostics.errors();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].notes.len(), 1);
    assert_eq!(
        diagnostics[0].notes[0].message,
        "location of previous definition"
    );
    assert!(matches!(diagnostics[0].notes[0].loc, Loc::File(..)));
}

#[test]
fn struct_import() {
    let mut cache = FileResolver::default();

    cache.set_file_contents(
        "a.sol",
        r#"
        import "b.sol";

        struct foo {
            struct_a bar;
        }
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "b.sol",
        r#"
        struct struct_a { uint32 f1; }
        "#
        .to_string(),
    );

    let ns = solang::parse_and_resolve(OsStr::new("a.sol"), &mut cache, Target::default_polkadot());

    assert!(!ns.diagnostics.any_errors());

    let mut cache = FileResolver::default();

    cache.set_file_contents(
        "a.sol",
        r#"
        import { struct_a as not_struct_a } from "b.sol";

        struct foo {
            struct_a bar;
        }
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "b.sol",
        r#"
        struct struct_a { uint32 f1; }
        "#
        .to_string(),
    );

    let ns = solang::parse_and_resolve(OsStr::new("a.sol"), &mut cache, Target::default_polkadot());

    assert_eq!(ns.diagnostics.first_error(), "type 'struct_a' not found");
}

#[test]
fn contract_import() {
    let mut cache = FileResolver::default();

    cache.set_file_contents(
        "a.sol",
        r#"
        import "b.sol";

        contract a {
            function go() public {
                b x = new b();

                assert(x.test() == 102);
            }
        }
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "b.sol",
        r#"
        contract b {
            function test() public returns (uint32) {
                return 102;
            }
        }
        "#
        .to_string(),
    );

    let ns = solang::parse_and_resolve(OsStr::new("a.sol"), &mut cache, Target::default_polkadot());

    assert!(!ns.diagnostics.any_errors());

    // lets try a importing an import
    let mut cache = FileResolver::default();

    cache.set_file_contents(
        "a.sol",
        r#"
        import "b.sol";

        contract a {
            function go() public {
                c x = new c();

                assert(x.test() == 102);
            }
        }
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "b.sol",
        r#"
        import "c.sol";
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "c.sol",
        r#"
        contract c {
            function test() public returns (uint32) {
                return 102;
            }
        }
        "#
        .to_string(),
    );

    let ns = solang::parse_and_resolve(OsStr::new("a.sol"), &mut cache, Target::default_polkadot());

    assert!(!ns.diagnostics.any_errors());

    // now let's rename an import in a chain
    let mut cache = FileResolver::default();

    cache.set_file_contents(
        "a.sol",
        r#"
        import "b.sol";

        contract a {
            function go() public {
                mr_c x = new mr_c();

                assert(x.test() == 102);
            }
        }
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "b.sol",
        r#"
        import { c as mr_c } from "c.sol";
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "c.sol",
        r#"
        contract c {
            function test() public returns (uint32) {
                return 102;
            }
        }
        "#
        .to_string(),
    );

    let ns = solang::parse_and_resolve(OsStr::new("a.sol"), &mut cache, Target::default_polkadot());

    assert!(!ns.diagnostics.any_errors());
}

#[test]
fn circular_import() {
    let mut cache = FileResolver::default();

    cache.set_file_contents(
        "self.sol",
        r#"
        import { foo } from "self.sol";

        enum foo { foo1, foo2 }

        contract c {
            foo public f1;
        }
        "#
        .to_string(),
    );

    let ns = solang::parse_and_resolve(
        OsStr::new("self.sol"),
        &mut cache,
        Target::default_polkadot(),
    );

    assert!(!ns.diagnostics.any_errors());

    let mut cache = FileResolver::default();

    cache.set_file_contents(
        "a.sol",
        r#"
        import "b.sol";

        enum enum_a { f1, f2 }

        contract a {
            function go() public {
                b x = new b();

                assert(x.test() == 102);
            }
        }
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "b.sol",
        r#"
        import "a.sol";
        contract b {
            function test() public returns (uint32) {
                return 102;
            }

            function test2() public returns (enum_a) {
                return enum_a.f1;
            }
        }
        "#
        .to_string(),
    );

    let ns = solang::parse_and_resolve(OsStr::new("a.sol"), &mut cache, Target::default_polkadot());

    assert!(!ns.diagnostics.any_errors());
}

#[test]
fn import_symbol() {
    // import struct via import symbol
    let mut cache = FileResolver::default();

    cache.set_file_contents(
        "a.sol",
        r#"
        import "b.sol" as foo;

        contract a {
            function go(foo.b_struct x) public returns (uint32) {
                return x.f1;
            }
        }
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "b.sol",
        r#"
        struct b_struct {
            uint32 f1;
        }
        "#
        .to_string(),
    );

    let ns = solang::parse_and_resolve(OsStr::new("a.sol"), &mut cache, Target::default_polkadot());

    assert!(!ns.diagnostics.any_errors());

    // import contract via import symbol
    let mut cache = FileResolver::default();

    cache.set_file_contents(
        "a.sol",
        r#"
        import "b.sol" as foo;

        contract a {
            function go() public returns (uint32) {
                foo.b x = new foo.b();

                return x.test();
            }
        }
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "b.sol",
        r#"
        contract b {
            function test() public returns (uint32) {
                return 102;
            }
        }
        "#
        .to_string(),
    );

    let ns = solang::parse_and_resolve(OsStr::new("a.sol"), &mut cache, Target::default_polkadot());

    assert!(!ns.diagnostics.any_errors());

    // import enum in contract via import symbol
    let mut cache = FileResolver::default();

    cache.set_file_contents(
        "a.sol",
        r#"
        import "b.sol" as foo;

        contract a {
            function go(foo.b.c x) public {
                assert(x == foo.b.c.c2);
            }
        }
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "b.sol",
        r#"
        contract b {
            enum c { c1, c2 }

            function test() public returns (uint32) {
                return 102;
            }
        }
        "#
        .to_string(),
    );

    let ns = solang::parse_and_resolve(OsStr::new("a.sol"), &mut cache, Target::default_polkadot());

    assert!(!ns.diagnostics.any_errors());

    // import struct in contract via import symbol chain
    let mut cache = FileResolver::default();

    cache.set_file_contents(
        "a.sol",
        r#"
        import "b.sol" as foo;

        contract a {
            function go(foo.bar.c.k x) public returns (int32) {
                return x.f1;
            }
        }
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "b.sol",
        r#"
        import "c.sol" as bar;
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "c.sol",
        r#"
        contract c {
            struct k {
                int32 f1;
            }

            function test() public returns (uint32) {
                return 102;
            }
        }
        "#
        .to_string(),
    );

    let ns = solang::parse_and_resolve(OsStr::new("a.sol"), &mut cache, Target::default_polkadot());

    assert!(!ns.diagnostics.any_errors());
}

#[test]
fn enum_import_chain() {
    // import struct in contract via import symbol chain
    let mut cache = FileResolver::default();

    cache.set_file_contents(
        "a.sol",
        r#"
        import "b.sol" as foo;

        contract a {
            function go(foo.c_import.d_import.d.enum_d x) public returns (bool) {
                return foo.c_import.d_import.d.enum_d.d2 == x;
            }
        }
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "b.sol",
        r#"
        import "c.sol" as c_import;
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "c.sol",
        r#"
        import "d.sol" as d_import;
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "d.sol",
        r#"
        abstract contract d {
            enum enum_d { d1, d2, d3 }
        }
        "#
        .to_string(),
    );

    let ns = solang::parse_and_resolve(OsStr::new("a.sol"), &mut cache, Target::default_polkadot());

    assert!(!ns.diagnostics.any_errors());

    // now with error
    let mut cache = FileResolver::default();

    cache.set_file_contents(
        "a.sol",
        r#"
        import "b.sol" as foo;

        contract a {
            function go(foo.c_import.d_import.d.enum_d x) public returns (bool) {
                return foo.c_import.d_import.d.enum_d.d4 == x;
            }
        }
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "b.sol",
        r#"
        import "c.sol" as c_import;
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "c.sol",
        r#"
        import "d.sol" as d_import;
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "d.sol",
        r#"
        abstract contract d {
            enum enum_d { d1, d2, d3 }
        }
        "#
        .to_string(),
    );

    let ns = solang::parse_and_resolve(OsStr::new("a.sol"), &mut cache, Target::default_polkadot());

    assert_eq!(
        ns.diagnostics.first_error(),
        "enum d.enum_d does not have value d4"
    );
}

#[test]
fn import_base_dir() {
    // if a imports x/b.sol then when x/b.sol imports, it should use x/ as a base
    let mut cache = FileResolver::default();

    cache.set_file_contents(
        "a.sol",
        r#"
        import "x/b.sol";

        contract a {
            function go() public {
                c x = new c();

                assert(x.test() == 102);
            }
        }
         "#
        .to_string(),
    );

    cache.set_file_contents(
        "x/b.sol",
        r#"
        import "x/c.sol";
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "x/c.sol",
        r#"
        contract c {
            function test() public returns (uint32) {
                return 102;
            }
        }
        "#
        .to_string(),
    );

    let ns = solang::parse_and_resolve(OsStr::new("a.sol"), &mut cache, Target::default_polkadot());

    assert!(!ns.diagnostics.any_errors());
}

#[test]
fn event_resolve() {
    let mut cache = FileResolver::default();

    cache.set_file_contents(
        "IThing.sol",
        r#"
        // SPDX-License-Identifier: MIT
        pragma solidity ^0.8.13;

        interface IThing {
            event Executed();

            function run() external;
        }
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "Test.sol",
        r#"
        // SPDX-License-Identifier: MIT
        pragma solidity ^0.8.13;

        import {IThing} from "IThing.sol";

        contract Thing is IThing {
            function run() external {
                emit Executed();
            }
        }"#
        .to_string(),
    );

    let ns = solang::parse_and_resolve(
        OsStr::new("Test.sol"),
        &mut cache,
        Target::default_polkadot(),
    );

    assert!(!ns.diagnostics.any_errors());
}
