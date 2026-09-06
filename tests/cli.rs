// SPDX-License-Identifier: Apache-2.0

use assert_cmd::cargo_bin_cmd;
use serde_json::Value;
use std::fs::File;
use tempfile::TempDir;

#[test]
fn create_output_dir() {
    let mut cmd = cargo_bin_cmd!("solang");

    let tmp = TempDir::new_in("tests").unwrap();

    let test1 = tmp.path().join("test1");

    cmd.args([
        "compile",
        "examples/solana/flipper.sol",
        "--target",
        "solana",
        "--output",
    ])
    .arg(test1.clone())
    .assert()
    .success();

    File::open(test1.join("flipper.json")).expect("should exist");
    File::open(test1.join("flipper.so")).expect("should exist");

    let mut cmd = cargo_bin_cmd!("solang");

    let test2 = tmp.path().join("test2");
    let test2_meta = tmp.path().join("test2_meta");

    let assert = cmd
        .args([
            "compile",
            "examples/solana/flipper.sol",
            "--target",
            "solana",
            "--contract",
            "flipper",
            "--contract-authors",
            "itchy and cratchy",
            "--output",
        ])
        .arg(test2.clone())
        .arg("--output-meta")
        .arg(test2_meta.clone())
        .assert()
        .success();

    File::open(test2.join("flipper.so")).expect("should exist");
    File::open(test2_meta.join("flipper.json")).expect("should exist");

    let output = assert.get_output();

    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "warning: the `authors` flag will be ignored for Solana target\n"
    );

    let mut cmd = cargo_bin_cmd!("solang");

    cmd.args([
        "compile",
        "examples/solana/flipper.sol",
        "--target",
        "solana",
        "--output",
        "examples/solana/flipper.sol",
    ])
    .assert()
    .failure();

    let mut cmd = cargo_bin_cmd!("solang");

    let test3 = tmp.path().join("test3");

    cmd.args([
        "compile",
        "examples/solana/flipper.sol",
        "--target",
        "solana",
        "--contract",
        "flapper,flipper", // not just flipper
        "--output",
    ])
    .arg(test3.clone())
    .assert()
    .failure();

    // nothing should have been created because flapper does not exist
    assert!(!test3.exists());
}

#[test]
fn basic_compilation_from_toml() {
    let mut new_cmd = cargo_bin_cmd!("solang");
    let tmp = TempDir::new_in("tests").unwrap();

    let solana_test = tmp.path().join("solana_test");

    //solang new --target solana
    new_cmd
        .arg("new")
        .arg(solana_test.clone())
        .args(["--target", "solana"])
        .assert()
        .success();
    File::open(solana_test.join("flipper.sol")).expect("should exist");
    File::open(solana_test.join("solang.toml")).expect("should exist");

    // compile flipper using config file
    let mut compile_cmd = cargo_bin_cmd!("solang");

    compile_cmd
        .args(["compile"])
        .current_dir(solana_test)
        .assert()
        .success();

    let polkadot_test = tmp.path().join("polkadot_test");
    let _new_cmd = cargo_bin_cmd!("solang")
        .arg("new")
        .arg(polkadot_test.clone())
        .args(["--target", "solana"])
        .assert()
        .success();

    compile_cmd.current_dir(polkadot_test).assert().success();
}

#[test]
fn standard_json_reads_sources_from_stdin() {
    let input = r#"{
        "language": "Solidity",
        "sources": {
            "input.sol": {
                "content": "contract flipper { function flip() public {} }"
            }
        }
    }"#;

    let assert = cargo_bin_cmd!("solang")
        .args(["compile", "--target", "polkadot", "--standard-json"])
        .write_stdin(input)
        .assert()
        .success();

    let output = String::from_utf8_lossy(&assert.get_output().stdout);
    let json: Value = serde_json::from_str(&output).unwrap();

    assert_eq!(json["target"], "Polkadot");
    assert!(json["contracts"]["input.sol"]["flipper"].is_object());
    assert_eq!(json["errors"].as_array().unwrap().len(), 0);
}
