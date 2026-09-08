// SPDX-License-Identifier: Apache-2.0

mod bpf;
mod polkadot_wasm;
mod riscv;
mod soroban_wasm;
use crate::Target;
use once_cell::sync::Lazy;
use std::ffi::CString;
use std::sync::Mutex;

static LINKER_MUTEX: Lazy<Mutex<i32>> = Lazy::new(|| Mutex::new(0i32));

/// Take an object file and turn it into a final linked binary ready for deployment
pub fn link(input: &[u8], name: &str, target: Target) -> Vec<u8> {
    // The lld linker is totally not thread-safe; it uses many globals
    // We should fix this one day
    let _lock = LINKER_MUTEX.lock().unwrap();

    match target {
        Target::Solana => bpf::link(input, name),
        Target::Soroban => soroban_wasm::link(input, name),
        Target::Riscv => riscv::link(input, name),
        Target::Polkadot {
            address_length: _,
            value_length: _,
        } => polkadot_wasm::link(input, name),
        _ => panic!("linker not implemented for target {target:?}"),
    }
}

extern "C" {
    fn LLDELFLink(args: *const *const libc::c_char, size: libc::size_t) -> libc::c_int;
}

pub fn elf_linker(args: &[CString]) -> bool {
    let mut command_line: Vec<*const libc::c_char> = Vec::with_capacity(args.len() + 1);

    let executable_name = CString::new("ld.lld").unwrap();

    command_line.push(executable_name.as_ptr());

    for arg in args {
        command_line.push(arg.as_ptr());
    }

    unsafe { LLDELFLink(command_line.as_ptr(), command_line.len()) == 0 }
}

extern "C" {
    fn LLDWasmLink(args: *const *const libc::c_char, size: libc::size_t) -> libc::c_int;
}

pub fn wasm_linker(args: &[CString]) -> bool {
    let mut command_line: Vec<*const libc::c_char> = Vec::with_capacity(args.len() + 1);

    let executable_name = CString::new("wasm-ld").unwrap();

    command_line.push(executable_name.as_ptr());

    for arg in args {
        command_line.push(arg.as_ptr());
    }

    unsafe { LLDWasmLink(command_line.as_ptr(), command_line.len()) == 0 }
}
