// SPDX-License-Identifier: Apache-2.0

//! The `analyze` subcommand.
//!
//! Analyzes a Soroban contract wasm using `sordec` (a Soroban wasm
//! analyzer/decoder). The input may be either an already-emitted `.wasm`, or a
//! `.sol` source file which is compiled to Soroban wasm first — the latter is a
//! convenience so a developer does not have to run `compile` themselves.

use crate::cli::AnalyzeCommand;
use std::{fs, process::exit};

pub fn analyze(args: &AnalyzeCommand) {
    let input = &args.input;

    let wasm: Vec<u8> = match input.extension().and_then(|e| e.to_str()) {
        Some("wasm") => match fs::read(input) {
            Ok(bytes) => bytes,
            Err(e) => {
                eprintln!("{}: error: {}", input.display(), e);
                exit(1);
            }
        },
        Some("sol") => {
            eprintln!(
                "{}: error: analyzing `.sol` sources is not yet implemented; \
                 compile to a Soroban `.wasm` first and pass that",
                input.display()
            );
            exit(1);
        }
        _ => {
            eprintln!(
                "{}: error: expected a `.wasm` or `.sol` file",
                input.display()
            );
            exit(1);
        }
    };

    let driver = sordec_driver::Driver::standard();
    match driver.run(&wasm) {
        Ok(output) => {
            eprintln!("{}: info: analyzed {} bytes", input.display(), wasm.len());
            print!("{}", output.wat);
        }
        Err(e) => {
            eprintln!("{}: error: {}", input.display(), e);
            exit(1);
        }
    }
}
