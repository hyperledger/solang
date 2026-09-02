// SPDX-License-Identifier: Apache-2.0

use solang::codegen::Options;
use solang::file_resolver::FileResolver;
use solang::sema::ast::Namespace;
use solang::{compile, Target};
use solang_parser::diagnostics::Level;
use std::ffi::OsStr;

fn compile_target(src: &str, target: Target) -> Namespace {
    let tmp_file = OsStr::new("test.sol");
    let mut cache = FileResolver::default();
    cache.set_file_contents(tmp_file.to_str().unwrap(), src.to_string());
    let opt = inkwell::OptimizationLevel::Default;

    let (_, ns) = compile(
        tmp_file,
        &mut cache,
        target,
        &Options {
            opt_level: opt.into(),
            log_runtime_errors: true,
            log_prints: true,
            #[cfg(feature = "wasm_opt")]
            wasm_opt: Some(contract_build::OptimizationPasses::Z),
            soroban_version: None,
            ..Default::default()
        },
        std::vec!["unknown".to_string()],
        "0.0.1",
    );

    ns
}

fn compile_soroban(src: &str) -> Namespace {
    compile_target(src, Target::Soroban)
}

#[test]
fn arrays_accepted_multiple_returns_rejected() {
    let ns = compile_soroban(
        r#"contract test {
    struct Item {
        uint64 value;
    }

    function struct_array(Item[] memory data) public returns (uint64) {
        return uint64(data.length);
    }

    function struct_param(Item memory item) public returns (uint64) {
        return item.value;
    }

    function struct_return() public returns (Item memory) {
        return Item({ value: 1 });
    }

    function array_return() public returns (uint64[] memory) {
        uint64[] memory values = new uint64[](1);
        values[0] = 1;
        return values;
    }

    function multiple_returns() public returns (uint64, uint64) {
        return (1, 2);
    }
}"#,
    );

    let errors = ns
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.level == Level::Error)
        .collect::<Vec<_>>();

    assert!(!errors.iter().any(|diagnostic| diagnostic.message
        == "type 'uint64[] memory' is not supported as a Soroban external function return value"));
    assert!(!errors.iter().any(|diagnostic| diagnostic.message
        == "type 'struct test.Item[] memory' is not supported as a Soroban external function parameter"));

    assert_eq!(errors.len(), 1);
    assert!(errors.iter().any(|diagnostic| diagnostic.message
        == "Soroban external functions can return at most one value"));
}

#[test]
fn struct_public_accessor_is_rejected() {
    let ns = compile_soroban(
        r#"contract test {
    struct Pair { uint64 first; uint64 second; }
    Pair public pair;
}"#,
    );

    let errors = ns
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.level == Level::Error)
        .collect::<Vec<_>>();

    assert_eq!(errors.len(), 1);
    assert!(errors.iter().any(|diagnostic| diagnostic.message
        == "type 'struct test.Pair' is not supported as a Soroban public variable accessor return value"));
}

#[test]
fn unsupported_event_parameters_are_rejected() {
    let ns = compile_soroban(
        r#"contract test {
    event Struct(Item data);
    struct Item { uint64 value; }
    function go() public { emit Struct(Item({ value: 1 })); }
}"#,
    );

    let errors = ns
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.level == Level::Error)
        .collect::<Vec<_>>();

    assert_eq!(errors.len(), 1);
    assert!(errors.iter().any(|diagnostic| diagnostic.message
        == "type 'struct test.Item' is not supported as a Soroban event parameter"));
}
