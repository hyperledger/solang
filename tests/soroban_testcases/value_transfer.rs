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

// Regression test for https://github.com/hyperledger-solang/solang/issues/1909
// Previously this would panic in codegen (`Found IntValue` at
// `src/emit/instructions.rs:989`) while lowering `Instr::ValueTransfer`. Soroban
// has no native value-transfer model, so the call should be rejected at sema
// time with an actionable diagnostic.
#[test]
fn address_transfer_is_rejected_on_soroban() {
    let ns = compile_soroban(
        r#"
        contract C {
            function run() external payable {
                payable(address(this)).transfer(uint256(0));
            }
        }
        "#,
    );

    let errors = ns
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.level == Level::Error)
        .collect::<Vec<_>>();

    assert!(
        errors
            .iter()
            .any(|diagnostic| diagnostic.message
                == "method 'transfer' is not available on Soroban. Soroban contracts \
                    do not have a native value-transfer model; move assets through \
                    the Stellar Asset Contract (SAC) or the token interface instead."),
        "expected a Soroban-specific rejection of `transfer`, got: {errors:?}",
    );
}

#[test]
fn address_send_is_rejected_on_soroban() {
    let ns = compile_soroban(
        r#"
        contract C {
            function run() external payable {
                payable(address(this)).send(uint256(0));
            }
        }
        "#,
    );

    let errors = ns
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.level == Level::Error)
        .collect::<Vec<_>>();

    assert!(
        errors
            .iter()
            .any(|diagnostic| diagnostic.message
                == "method 'send' is not available on Soroban. Soroban contracts \
                    do not have a native value-transfer model; move assets through \
                    the Stellar Asset Contract (SAC) or the token interface instead."),
        "expected a Soroban-specific rejection of `send`, got: {errors:?}",
    );
}
