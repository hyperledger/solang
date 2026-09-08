// SPDX-License-Identifier: Apache-2.0

//! Object code generation for the RISC-V target.
//!
//! Solang links against a purpose-built LLVM that only enables the
//! WebAssembly and SBF backends, so no in-process `TargetMachine` can be
//! created for RISC-V. Until that LLVM build gains the RISC-V backend, the
//! bitcode is handed to an external `llc` instead. Everything else, including
//! linking, still goes through the in-tree LLD.
//!
//! Set `SOLANG_RISCV_LLC` to select a specific `llc`.

use inkwell::module::Module;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

/// `llc` binaries to try, in order, when `SOLANG_RISCV_LLC` is not set.
const LLC_CANDIDATES: [&str; 4] = ["llc-19", "llc-18", "llc-17", "llc"];

/// Compile `module` to a RISC-V ELF object file.
pub(crate) fn object_from_module(module: &Module, assembly: bool) -> Result<Vec<u8>, String> {
    let dir = tempdir().map_err(|e| e.to_string())?;
    let bitcode = dir.path().join("contract.bc");
    let output = dir
        .path()
        .join(if assembly { "contract.s" } else { "contract.o" });

    if !module.write_bitcode_to_path(&bitcode) {
        return Err("failed to write RISC-V bitcode".into());
    }

    let llc = find_llc()?;

    let result = Command::new(&llc)
        .args([
            "-mtriple=riscv64-unknown-none-elf",
            "-mattr=+m,+a,+c",
            // r55 loads contracts at 0x80300000; medlow's absolute addressing
            // cannot reach that, so use medany's PC-relative sequences.
            "-code-model=medium",
            "-O2",
            if assembly {
                "-filetype=asm"
            } else {
                "-filetype=obj"
            },
        ])
        .arg(&bitcode)
        .arg("-o")
        .arg(&output)
        .output()
        .map_err(|e| format!("failed to run {llc}: {e}"))?;

    if !result.status.success() {
        return Err(format!(
            "{llc} failed: {}",
            String::from_utf8_lossy(&result.stderr)
        ));
    }

    fs::read(&output).map_err(|e| e.to_string())
}

/// Locate an `llc` that has the RISC-V backend enabled.
fn find_llc() -> Result<String, String> {
    if let Ok(llc) = std::env::var("SOLANG_RISCV_LLC") {
        return Ok(llc);
    }

    LLC_CANDIDATES
        .iter()
        .find(|llc| has_riscv_backend(llc))
        .map(|llc| llc.to_string())
        .ok_or_else(|| {
            format!(
                "no llc with a RISC-V backend found (tried {}); \
                 set SOLANG_RISCV_LLC to point at one",
                LLC_CANDIDATES.join(", ")
            )
        })
}

fn has_riscv_backend(llc: &str) -> bool {
    Command::new(llc)
        .arg("--version")
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).contains("riscv64"))
        .unwrap_or(false)
}
