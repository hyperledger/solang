// SPDX-License-Identifier: Apache-2.0

use assert_cmd::cargo_bin_cmd;
use rayon::prelude::*;
use std::ffi::OsString;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::{fs, path::PathBuf};

#[test]
fn solidity_testcases() {
    run_test_for_path("./tests/codegen_testcases/solidity/");
}

#[test]
fn yul_testcases() {
    run_test_for_path("./tests/codegen_testcases/yul/")
}

fn run_test_for_path(path: &str) {
    let mut tests = Vec::new();

    let ext = OsString::from("sol");
    for entry in fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path();

        if path.is_file() && path.extension() == Some(&ext) {
            tests.push(path);
        }
    }

    tests.into_par_iter().for_each(testcase);
}

#[derive(Debug)]
#[allow(unused)]
enum Test {
    Check(usize, String),
    CheckAbsent(usize, String),
    NotCheck(usize, String),
    Fail(usize, String),
    Rewind(usize),
}

fn testcase(path: PathBuf) {
    // find the args to run.
    println!("testcase: {}", path.display());

    let file = File::open(&path).unwrap();
    let reader = BufReader::new(file);
    let mut command_line: Option<String> = None;
    let mut checks = Vec::new();
    let mut fails = Vec::new();
    let mut read_from = None;

    for (line_no, line) in reader.lines().enumerate() {
        let mut line = line.unwrap();
        line = line.trim().parse().unwrap();
        // The first line should be a command line (excluding "solang compile") after // RUN:
        if let Some(args) = line.strip_prefix("// RUN: ") {
            assert_eq!(command_line, None);

            command_line = Some(String::from(args));

        // Read the contents of a file, e.g. the llvm-ir output of // RUN: --emit llvm-ir
        // rather than the stdout of the command
        } else if let Some(check) = line.strip_prefix("// READ:") {
            read_from = Some(check.trim().to_string());
        // Read more input until you find a line that contains the needle // CHECK: needle
        } else if let Some(check) = line.strip_prefix("// CHECK:") {
            checks.push(Test::Check(line_no, check.trim().to_string()));
        //
        } else if let Some(fail) = line.strip_prefix("// FAIL:") {
            fails.push(Test::Fail(line_no, fail.trim().to_string()));
        // Ensure that the following line in the input does not match
        } else if let Some(not_check) = line.strip_prefix("// NOT-CHECK:") {
            checks.push(Test::NotCheck(line_no, not_check.trim().to_string()));
        // Check the output from here until the end of the file does not contain the needle
        } else if let Some(check_absent) = line.strip_prefix("// CHECK-ABSENT:") {
            checks.push(Test::CheckAbsent(line_no, check_absent.trim().to_string()));
        // Go back to the beginning and find the needle from there, like // CHECK: but from
        // the beginning of the file.
        } else if let Some(check) = line.strip_prefix("// BEGIN-CHECK:") {
            checks.push(Test::Rewind(line_no));
            checks.push(Test::Check(line_no, check.trim().to_string()));
        }
    }

    let args = command_line.expect("cannot find RUN: line");
    assert_ne!(checks.len() + fails.len(), 0);

    let mut cmd = cargo_bin_cmd!("solang");
    let assert = cmd
        .arg("compile")
        .args(args.split_whitespace())
        .arg(format!("{}", path.canonicalize().unwrap().display()))
        .assert();

    let output = assert.get_output();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let mut current_check = 0;
    let mut current_fail = 0;
    let mut current_line = 0;
    let contents = if let Some(file) = read_from {
        fs::read_to_string(file).unwrap()
    } else {
        stdout.to_string()
    };
    let lines: Vec<&str> = contents.split('\n').chain(stderr.split('\n')).collect();

    while current_line < lines.len() {
        let line = lines[current_line];

        match checks.get(current_check) {
            Some(Test::Check(_, needle)) => {
                if line.contains(needle) {
                    current_check += 1;
                }
            }
            Some(Test::NotCheck(_, needle)) => {
                if !line.contains(needle) {
                    current_check += 1;
                    // We should not advance line during a not check
                    current_line -= 1;
                }
            }
            Some(Test::CheckAbsent(_, needle)) => {
                for line in lines.iter().skip(current_line) {
                    if line.contains(needle) {
                        panic!(
                            "FOUND CHECK-ABSENT: {:?}, {}",
                            checks[current_check],
                            path.display()
                        );
                    }
                }
                current_check += 1;
            }
            Some(Test::Rewind(_)) => {
                current_line = 0;
                current_check += 1;
                continue;
            }
            _ => (),
        }

        if let Some(Test::Fail(_, needle)) = fails.get(current_fail) {
            if line.contains(needle) {
                current_fail += 1;
            }
        }

        current_line += 1;
    }

    if current_check < checks.len() {
        println!("{stderr}");
        println!("OUTPUT: \n===8<===8<===\n{stdout}===8<===8<===\n");

        panic!(
            "NOT FOUND CHECK: {:?}, {}",
            checks[current_check],
            path.display()
        );
    } else if current_fail < fails.len() {
        println!("STDERR: \n===8<===8<===\n{stderr}===8<===8<===\n");

        panic!(
            "NOT FOUND FAIL: {:?}, {}",
            fails[current_fail],
            path.display()
        );
    }
}
