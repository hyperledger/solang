// SPDX-License-Identifier: Apache-2.0

use std::ffi::CString;
use std::fs::File;
use std::io::{Read, Write};
use tempfile::tempdir;

pub fn link(input: &[u8], name: &str) -> Vec<u8> {
    let dir = tempdir().expect("failed to create temp directory for linking");

    let object_filename = dir.path().join(format!("{name}.o"));
    let res_filename = dir.path().join(format!("{name}.elf"));
    let linker_script_filename = dir.path().join("r55-bare-bones.x");

    File::create(&object_filename)
        .expect("failed to create object file")
        .write_all(input)
        .expect("failed to write object file");

    File::create(&linker_script_filename)
        .expect("failed to create linker script")
        .write_all(include_bytes!("riscv/r55-bare-bones.x"))
        .expect("failed to write linker script");

    let command_line = vec![
        CString::new("-T").unwrap(),
        CString::new(linker_script_filename.to_str().unwrap()).unwrap(),
        CString::new("--static").unwrap(),
        CString::new("-m").unwrap(),
        CString::new("elf64lriscv").unwrap(),
        CString::new(object_filename.to_str().unwrap()).unwrap(),
        CString::new("-o").unwrap(),
        CString::new(res_filename.to_str().unwrap()).unwrap(),
    ];

    assert!(!super::elf_linker(&command_line), "riscv linker failed");

    let mut output = Vec::new();
    File::open(&res_filename)
        .expect("output file should exist")
        .read_to_end(&mut output)
        .expect("failed to read output file");

    output
}
