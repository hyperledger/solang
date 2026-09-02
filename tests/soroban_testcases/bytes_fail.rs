// SPDX-License-Identifier: Apache-2.0

use solang::codegen::Options;
use solang::file_resolver::FileResolver;
use solang::sema::ast::Namespace;
use solang::{compile, Target};
use solang_parser::diagnostics::Level;
use std::ffi::OsStr;

fn compile_soroban(src: &str) -> Namespace {
    let tmp_file = OsStr::new("test.sol");
    let mut cache = FileResolver::default();
    cache.set_file_contents(tmp_file.to_str().unwrap(), src.to_string());
    let opt = inkwell::OptimizationLevel::Default;

    let (_, ns) = compile(
        tmp_file,
        &mut cache,
        Target::Soroban,
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

#[test]
fn fixed_bytes_index_assignment_is_rejected() {
    let ns = compile_soroban(
        r#"contract T {
    function bad(bytes4 b) public pure returns (bytes4) {
        b[0] = 0x01;
        return b;
    }
}"#,
    );

    let errors = ns
        .diagnostics
        .iter()
        .filter(|d| d.level == Level::Error)
        .collect::<Vec<_>>();

    assert!(
        !errors.is_empty(),
        "assigning to bytesN[i] must be a sema error"
    );
}

#[test]
fn bytesn_array_abi_is_supported() {
    let ns = compile_soroban(
        r#"contract T {
    function f(bytes32[] memory data) public pure returns (uint64) {
        return uint64(data.length);
    }
}"#,
    );

    let errors = ns
        .diagnostics
        .iter()
        .filter(|d| d.level == Level::Error)
        .collect::<Vec<_>>();

    assert!(
        errors.is_empty(),
        "bytes32[] memory param must be accepted, got: {errors:?}"
    );
}

#[test]
fn bytes_array_abi_is_supported() {
    let ns = compile_soroban(
        r#"contract T {
    function f(bytes[] memory data) public pure returns (uint64) {
        return uint64(data.length);
    }
}"#,
    );

    let errors = ns
        .diagnostics
        .iter()
        .filter(|d| d.level == Level::Error)
        .collect::<Vec<_>>();

    assert!(
        errors.is_empty(),
        "bytes[] memory param must be accepted, got: {errors:?}"
    );
}
