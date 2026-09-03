// SPDX-License-Identifier: Apache-2.0

use std::ffi::OsStr;

use solang::{
    codegen::codegen,
    file_resolver::FileResolver,
    lir::{converter::Converter, printer::Printer},
    parse_and_resolve,
    sema::ast::Namespace,
    Target,
};

use crate::stringfy_lir;

fn new_file_resolver(src: &str) -> FileResolver {
    let mut cache = FileResolver::default();
    cache.set_file_contents("test.sol", src.to_string());
    cache
}

fn print_lir_str(src: &str, cfg_no: usize, target: Target) {
    let mut resolver = new_file_resolver(src);
    let mut ns: Namespace = parse_and_resolve(OsStr::new("test.sol"), &mut resolver, target);
    // print diagnostics
    if !ns.diagnostics.is_empty() {
        ns.print_diagnostics_in_plain(&resolver, false);
    }
    codegen(&mut ns, &Default::default());
    let contract = ns.contracts.first().unwrap();
    let cfg = contract.cfg.get(cfg_no).unwrap();

    let converter = Converter::new(&ns, cfg);
    let lir = converter.get_lir();

    let printer = Printer::new(&lir.vartable);

    let result = stringfy_lir!(printer, &lir);
    println!("{result}");
}

fn assert_lir_str_eq_by_name(src: &str, cfg_name: &str, expected: &str, target: Target) {
    let mut resolver = new_file_resolver(src);
    let mut ns: Namespace = parse_and_resolve(OsStr::new("test.sol"), &mut resolver, target);
    // print diagnostics
    if !ns.diagnostics.is_empty() {
        ns.print_diagnostics_in_plain(&resolver, false);
    }
    codegen(&mut ns, &Default::default());
    let contract = ns.contracts.first().unwrap();
    let cfg = contract
        .cfg
        .iter()
        .filter(|cfg| cfg.name == cfg_name)
        .next_back()
        .unwrap();

    let converter = Converter::new(&ns, cfg);
    let lir = converter.get_lir();

    let printer = Printer::new(&lir.vartable);

    let result = stringfy_lir!(printer, &lir);
    assert_eq!(result.trim(), expected);

    let re = regex::Regex::new(r"%temp\.ssa_ir\.\d+ =").unwrap();
    let mut temp_vars = Vec::new();
    for cap in re.captures_iter(result.as_str()) {
        temp_vars.push(cap[0].to_string());
    }
    // check if there are duplicated temp variables
    let mut temp_vars_clone = temp_vars.clone();
    temp_vars_clone.dedup();

    // assert length equal
    assert_eq!(temp_vars.len(), temp_vars_clone.len());
}

fn assert_lir_str_eq(src: &str, cfg_no: usize, expected: &str, target: Target) {
    let mut resolver = new_file_resolver(src);
    let mut ns: Namespace = parse_and_resolve(OsStr::new("test.sol"), &mut resolver, target);
    // print diagnostics
    if !ns.diagnostics.is_empty() {
        ns.print_diagnostics_in_plain(&resolver, false);
    }
    codegen(&mut ns, &Default::default());
    let contract = ns.contracts.first().unwrap();
    let cfg = contract.cfg.get(cfg_no).unwrap();

    let converter = Converter::new(&ns, cfg);
    let lir = converter.get_lir();

    let printer = Printer::new(&lir.vartable);

    let result = stringfy_lir!(printer, &lir);
    assert_eq!(result.trim(), expected);

    let re = regex::Regex::new(r"%temp\.ssa_ir\.\d+ =").unwrap();
    let mut temp_vars = Vec::new();
    for cap in re.captures_iter(result.as_str()) {
        temp_vars.push(cap[0].to_string());
    }
    // check if there are duplicated temp variables
    let mut temp_vars_clone = temp_vars.clone();
    temp_vars_clone.dedup();

    // assert length equal
    assert_eq!(temp_vars.len(), temp_vars_clone.len());
}

fn assert_solana_lir_str_eq(src: &str, cfg_no: usize, expected: &str) {
    assert_lir_str_eq(src, cfg_no, expected, Target::Solana);
}

fn assert_polkadot_lir_str_eq(src: &str, cfg_no: usize, expected: &str) {
    assert_lir_str_eq(src, cfg_no, expected, Target::default_polkadot());
}

#[test]
fn test_convert_lir() {
    let src = r#"
contract dynamicarray {
    function test() public pure {
        int64[] memory a = new int64[](3);
        a[0] = 1;
        a[1] = 2;
        a[2] = 3;
        a.push(4);

        assert(a.length == 4);
    }
}"#;

    print_lir_str(src, 0, Target::Solana);

    assert_solana_lir_str_eq(
        src,
        0,
        r#"public function sol#2 dynamicarray::dynamicarray::function::test ():
block#0 entry:
    uint32 %array_length.temp.1 = 3;
    ptr<int64[]> %a = alloc ptr<int64[]>[uint32(3)];
    uint32 %index.temp.3 = 0;
    bool %temp.ssa_ir.9 = uint32(0) (u)>= uint32(3);
    cbr bool(%temp.ssa_ir.9) block#1 else block#2;

block#1 out_of_bounds:
    assert_failure;

block#2 in_bounds:
    int64 %temp.2 = 1;
    ptr<int64> %temp.ssa_ir.10 = ptr<int64[]>(%a)[uint32(0)];
    store int64(1) to ptr<int64>(%temp.ssa_ir.10);
    uint32 %index.temp.5 = 1;
    bool %temp.ssa_ir.11 = uint32(1) (u)>= uint32(3);
    cbr bool(%temp.ssa_ir.11) block#3 else block#4;

block#3 out_of_bounds:
    assert_failure;

block#4 in_bounds:
    int64 %temp.4 = 2;
    ptr<int64> %temp.ssa_ir.12 = ptr<int64[]>(%a)[uint32(1)];
    store int64(2) to ptr<int64>(%temp.ssa_ir.12);
    uint32 %index.temp.7 = 2;
    bool %temp.ssa_ir.13 = uint32(2) (u)>= uint32(3);
    cbr bool(%temp.ssa_ir.13) block#5 else block#6;

block#5 out_of_bounds:
    assert_failure;

block#6 in_bounds:
    int64 %temp.6 = 3;
    ptr<int64> %temp.ssa_ir.14 = ptr<int64[]>(%a)[uint32(2)];
    store int64(3) to ptr<int64>(%temp.ssa_ir.14);
    int64 %temp.8 = push_mem ptr<int64[]>(%a) int64(4);
    uint32 %array_length.temp.1 = 4;
    bool %temp.ssa_ir.15 = uint32(4) == uint32(4);
    cbr bool(%temp.ssa_ir.15) block#7 else block#8;

block#7 noassert:
    return;

block#8 doassert:
    assert_failure;"#,
    );
}

#[test]
fn test_bool_exprs() {
    let cfg_no = 1;

    // read the example.sol file
    let src = r#"
		contract test {
			enum State {
				Running,
				Sleeping,
				Waiting,
				Stopped,
				Zombie,
				StateCount
			}
			State state;
			int32 pid;
			int32 constant first_pid = 1;
			constructor(int32 _pid) {
				pid = _pid;
			}
			function is_zombie_reaper() public view returns (bool) {
				return (pid == first_pid && state != State.Zombie);
			}
		}"#;

    assert_solana_lir_str_eq(
        src,
        cfg_no,
        r#"public function sol#3 test::test::function::is_zombie_reaper () returns (bool):
block#0 entry:
    int32 %temp.3 = load_storage uint32(20);
    bool %and.temp.4 = false;
    bool %temp.ssa_ir.6 = int32(%temp.3) == int32(1);
    cbr bool(%temp.ssa_ir.6) block#1 else block#2;

block#1 and_right_side:
    uint8 %temp.5 = load_storage uint32(16);
    bool %and.temp.4 = uint8(%temp.5) != uint8(4);
    br block#2;

block#2 and_end:
    return bool(%and.temp.4);"#,
    )
}

#[test]
fn test_cast() {
    let cfg_no = 0;

    // read the example.sol file
    let src = r#"
		contract test {
			int32 constant first_pid = 1;
			function systemd_pid() public pure returns (uint32) {
				return uint32(first_pid);
			}
		}"#;

    assert_solana_lir_str_eq(
        src,
        cfg_no,
        r#"public function sol#2 test::test::function::systemd_pid () returns (uint32):
block#0 entry:
    uint32 %temp.ssa_ir.1 = (cast int32(1) to uint32);
    return uint32(%temp.ssa_ir.1);"#,
    )
}

#[test]
fn test_arithmetic_exprs() {
    let cfg_no = 0;

    // read the example.sol file
    let src = r#"
		contract test {
			function celcius2fahrenheit(int32 celcius) pure public returns (int32) {
				int32 fahrenheit = celcius * 9 / 5 + 32;
				return fahrenheit;
			}
		}"#;

    assert_solana_lir_str_eq(
        src,
        cfg_no,
        r#"public function sol#2 test::test::function::celcius2fahrenheit__int32 (int32) returns (int32):
block#0 entry:
    int32 %celcius = int32(arg#0);
    int32 %temp.ssa_ir.4 = int32(%celcius) * int32(9);
    int32 %temp.ssa_ir.3 = int32(%temp.ssa_ir.4) / int32(5);
    int32 %fahrenheit = int32(%temp.ssa_ir.3) + int32(32);
    return int32(%fahrenheit);"#,
    )
}

#[test]
fn test_arithmetic_exprs_1() {
    let cfg_no = 0;

    // read the example.sol file
    let src = r#"
		contract test {
			function byte8reverse(bytes8 input) public pure returns (bytes8 out) {
				out = ((input << 56) & hex"ff00_0000_0000_0000") |
						((input << 40) & hex"00ff_0000_0000_0000") |
						((input << 24) & hex"0000_ff00_0000_0000") |
						((input <<  8) & hex"0000_00ff_0000_0000") |
						((input >>  8) & hex"0000_0000_ff00_0000") |
						((input >> 24) & hex"0000_0000_00ff_0000") |
						((input >> 40) & hex"0000_0000_0000_ff00") |
						((input >> 56) & hex"0000_0000_0000_00ff");
			}
		}"#;

    assert_solana_lir_str_eq(
        src,
        cfg_no,
        r#"public function sol#2 test::test::function::byte8reverse__bytes8 (bytes8) returns (bytes8):
block#0 entry:
    bytes8 %input = bytes8(arg#0);
    bytes8 %out = bytes8 hex"00_00_00_00_00_00_00_00";
    bytes8 %temp.ssa_ir.9 = bytes8(%input) << bytes8(56);
    bytes8 %temp.ssa_ir.10 = bytes8 hex"ff_00_00_00_00_00_00_00";
    bytes8 %temp.ssa_ir.8 = bytes8(%temp.ssa_ir.9) & bytes8(%temp.ssa_ir.10);
    bytes8 %temp.ssa_ir.12 = bytes8(%input) << bytes8(40);
    bytes8 %temp.ssa_ir.13 = bytes8 hex"00_ff_00_00_00_00_00_00";
    bytes8 %temp.ssa_ir.11 = bytes8(%temp.ssa_ir.12) & bytes8(%temp.ssa_ir.13);
    bytes8 %temp.ssa_ir.7 = bytes8(%temp.ssa_ir.8) | bytes8(%temp.ssa_ir.11);
    bytes8 %temp.ssa_ir.15 = bytes8(%input) << bytes8(24);
    bytes8 %temp.ssa_ir.16 = bytes8 hex"00_00_ff_00_00_00_00_00";
    bytes8 %temp.ssa_ir.14 = bytes8(%temp.ssa_ir.15) & bytes8(%temp.ssa_ir.16);
    bytes8 %temp.ssa_ir.6 = bytes8(%temp.ssa_ir.7) | bytes8(%temp.ssa_ir.14);
    bytes8 %temp.ssa_ir.18 = bytes8(%input) << bytes8(8);
    bytes8 %temp.ssa_ir.19 = bytes8 hex"00_00_00_ff_00_00_00_00";
    bytes8 %temp.ssa_ir.17 = bytes8(%temp.ssa_ir.18) & bytes8(%temp.ssa_ir.19);
    bytes8 %temp.ssa_ir.5 = bytes8(%temp.ssa_ir.6) | bytes8(%temp.ssa_ir.17);
    bytes8 %temp.ssa_ir.21 = bytes8(%input) (u)>> bytes8(8);
    bytes8 %temp.ssa_ir.22 = bytes8 hex"00_00_00_00_ff_00_00_00";
    bytes8 %temp.ssa_ir.20 = bytes8(%temp.ssa_ir.21) & bytes8(%temp.ssa_ir.22);
    bytes8 %temp.ssa_ir.4 = bytes8(%temp.ssa_ir.5) | bytes8(%temp.ssa_ir.20);
    bytes8 %temp.ssa_ir.24 = bytes8(%input) (u)>> bytes8(24);
    bytes8 %temp.ssa_ir.25 = bytes8 hex"00_00_00_00_00_ff_00_00";
    bytes8 %temp.ssa_ir.23 = bytes8(%temp.ssa_ir.24) & bytes8(%temp.ssa_ir.25);
    bytes8 %temp.ssa_ir.3 = bytes8(%temp.ssa_ir.4) | bytes8(%temp.ssa_ir.23);
    bytes8 %temp.ssa_ir.27 = bytes8(%input) (u)>> bytes8(40);
    bytes8 %temp.ssa_ir.28 = bytes8 hex"00_00_00_00_00_00_ff_00";
    bytes8 %temp.ssa_ir.26 = bytes8(%temp.ssa_ir.27) & bytes8(%temp.ssa_ir.28);
    bytes8 %temp.ssa_ir.2 = bytes8(%temp.ssa_ir.3) | bytes8(%temp.ssa_ir.26);
    bytes8 %temp.ssa_ir.30 = bytes8(%input) (u)>> bytes8(56);
    bytes8 %temp.ssa_ir.31 = bytes8 hex"00_00_00_00_00_00_00_ff";
    bytes8 %temp.ssa_ir.29 = bytes8(%temp.ssa_ir.30) & bytes8(%temp.ssa_ir.31);
    bytes8 %out = bytes8(%temp.ssa_ir.2) | bytes8(%temp.ssa_ir.29);
    return bytes8(%out);"#,
    )
}

#[test]
fn test_for_loop() {
    let cfg_no = 0;

    // read the example.sol file
    let src = r#"
		contract test {
            enum State {
                Running,
                Sleeping,
                Waiting,
                Stopped,
                Zombie,
                StateCount
            }
            function get_pid_state(uint64 _pid) pure private returns (State) {
                uint64 n = 8;
                for (uint16 i = 1; i < 10; ++i) {
                    if ((i % 3) == 0) {
                        n *= _pid / uint64(i);
                    } else {
                        n /= 3;
                    }
                }
        
                return State(n % uint64(State.StateCount));
            }
		}"#;

    assert_solana_lir_str_eq(
        src,
        cfg_no,
        r#"private function sol#2 test::test::function::get_pid_state__uint64 (uint64) returns (uint8):
block#0 entry:
    uint64 %_pid = uint64(arg#0);
    uint64 %n = 8;
    uint16 %i = 1;
    br block#2;

block#1 body:
    uint16 %temp.ssa_ir.6 = uint16(%i) (u)% uint16(3);
    bool %temp.ssa_ir.5 = uint16(%temp.ssa_ir.6) == uint16(0);
    cbr bool(%temp.ssa_ir.5) block#5 else block#6;

block#2 cond:
    bool %temp.ssa_ir.7 = uint16(%i) (u)< uint16(10);
    cbr bool(%temp.ssa_ir.7) block#1 else block#4;

block#3 next:
    uint16 %temp.4 = uint16(%i) + uint16(1);
    uint16 %i = uint16(%temp.4);
    br block#2;

block#4 endfor:
    uint64 %temp.ssa_ir.9 = uint64(%n) (u)% uint64(5);
    uint8 %temp.ssa_ir.8 = (trunc uint64(%temp.ssa_ir.9) to uint8);
    return uint8(%temp.ssa_ir.8);

block#5 then:
    uint64 %temp.ssa_ir.11 = (zext uint16(%i) to uint64);
    uint64 %temp.ssa_ir.10 = uint64(%_pid) (u)/ uint64(%temp.ssa_ir.11);
    uint64 %n = uint64(%n) * uint64(%temp.ssa_ir.10);
    br block#7;

block#6 else:
    uint64 %n = uint64(%n) (u)/ uint64(3);
    br block#7;

block#7 endif:
    br block#3;"#,
    )
}

#[test]
fn test_nested_if_blocks() {
    let cfg_no = 0;

    // read the example.sol file
    let src = r#"
		contract test {
            enum suit { club, diamonds, hearts, spades }
            enum value { two, three, four, five, six, seven, eight, nine, ten, jack, queen, king, ace }
            struct card {
                value v;
                suit s;
            }
			function score_card(card memory c) public pure returns (uint32 score) {
                if (c.s == suit.hearts) {
                    if (c.v == value.ace) {
                        score = 14;
                    }
                    if (c.v == value.king) {
                        score = 13;
                    }
                    if (c.v == value.queen) {
                        score = 12;
                    }
                    if (c.v == value.jack) {
                        score = 11;
                    }
                }
                // all others score 0
            }
		}"#;

    assert_solana_lir_str_eq(
        src,
        cfg_no,
        r#"public function sol#2 test::test::function::score_card__test.card (ptr<struct.0>) returns (uint32):
block#0 entry:
    ptr<struct.0> %c = ptr<struct.0>(arg#0);
    uint32 %score = 0;
    ptr<uint8> %temp.ssa_ir.4 = access ptr<struct.0>(%c) member 1;
    uint8 %temp.ssa_ir.3 = *ptr<uint8>(%temp.ssa_ir.4);
    bool %temp.ssa_ir.2 = uint8(%temp.ssa_ir.3) == uint8(2);
    cbr bool(%temp.ssa_ir.2) block#1 else block#2;

block#1 then:
    ptr<uint8> %temp.ssa_ir.7 = access ptr<struct.0>(%c) member 0;
    uint8 %temp.ssa_ir.6 = *ptr<uint8>(%temp.ssa_ir.7);
    bool %temp.ssa_ir.5 = uint8(%temp.ssa_ir.6) == uint8(12);
    cbr bool(%temp.ssa_ir.5) block#3 else block#4;

block#2 endif:
    return uint32(%score);

block#3 then:
    uint32 %score = 14;
    br block#4;

block#4 endif:
    ptr<uint8> %temp.ssa_ir.10 = access ptr<struct.0>(%c) member 0;
    uint8 %temp.ssa_ir.9 = *ptr<uint8>(%temp.ssa_ir.10);
    bool %temp.ssa_ir.8 = uint8(%temp.ssa_ir.9) == uint8(11);
    cbr bool(%temp.ssa_ir.8) block#5 else block#6;

block#5 then:
    uint32 %score = 13;
    br block#6;

block#6 endif:
    ptr<uint8> %temp.ssa_ir.13 = access ptr<struct.0>(%c) member 0;
    uint8 %temp.ssa_ir.12 = *ptr<uint8>(%temp.ssa_ir.13);
    bool %temp.ssa_ir.11 = uint8(%temp.ssa_ir.12) == uint8(10);
    cbr bool(%temp.ssa_ir.11) block#7 else block#8;

block#7 then:
    uint32 %score = 12;
    br block#8;

block#8 endif:
    ptr<uint8> %temp.ssa_ir.16 = access ptr<struct.0>(%c) member 0;
    uint8 %temp.ssa_ir.15 = *ptr<uint8>(%temp.ssa_ir.16);
    bool %temp.ssa_ir.14 = uint8(%temp.ssa_ir.15) == uint8(9);
    cbr bool(%temp.ssa_ir.14) block#9 else block#10;

block#9 then:
    uint32 %score = 11;
    br block#10;

block#10 endif:
    br block#2;"#,
    )
}

#[test]
fn test_init_struct() {
    let cfg_no = 0;

    // read the example.sol file
    let src = r#"
		contract test {
            enum suit { club, diamonds, hearts, spades }
            enum value { two, three, four, five, six, seven, eight, nine, ten, jack, queen, king, ace }
            struct card {
                value v;
                suit s;
            }
			function ace_of_spaces() public pure returns (card memory) {
                return card({s: suit.spades, v: value.ace });
            }
		}"#;

    assert_solana_lir_str_eq(
        src,
        cfg_no,
        r#"public function sol#2 test::test::function::ace_of_spaces () returns (ptr<struct.0>):
block#0 entry:
    ptr<struct.0> %temp.ssa_ir.1 = struct { uint8(12), uint8(3) };
    return ptr<struct.0>(%temp.ssa_ir.1);"#,
    )
}

#[test]
fn test_account_access() {
    let cfg_no = 0;

    // read the example.sol file
    let src = r#"
contract Foo {
    @account(oneAccount)
    @signer(mySigner)
    @mutableAccount(otherAccount)
    @mutableSigner(otherSigner)
    function bar() external returns (uint64) {
        assert(tx.accounts.mySigner.is_signer);
        assert(tx.accounts.otherSigner.is_signer);
        assert(tx.accounts.otherSigner.is_writable);
        assert(tx.accounts.otherAccount.is_writable);

        tx.accounts.otherAccount.data[0] = 0xca;
        tx.accounts.otherSigner.data[1] = 0xfe;

        return tx.accounts.oneAccount.lamports;
    }
}"#;

    assert_solana_lir_str_eq(
        src,
        cfg_no,
        r#"public function sol#2 Foo::Foo::function::bar () returns (uint64):
block#0 entry:
    ptr<struct.SolAccountInfo[]> %temp.ssa_ir.12 = builtin: Accounts();
    ptr<struct.SolAccountInfo> %temp.1 = ptr<struct.SolAccountInfo[]>(%temp.ssa_ir.12)[uint32(1)];
    ptr<bool> %temp.ssa_ir.14 = access ptr<struct.SolAccountInfo>(%temp.1) member 5;
    bool %temp.ssa_ir.13 = *ptr<bool>(%temp.ssa_ir.14);
    cbr bool(%temp.ssa_ir.13) block#1 else block#2;

block#1 noassert:
    ptr<struct.SolAccountInfo[]> %temp.ssa_ir.15 = builtin: Accounts();
    ptr<struct.SolAccountInfo> %temp.2 = ptr<struct.SolAccountInfo[]>(%temp.ssa_ir.15)[uint32(3)];
    ptr<bool> %temp.ssa_ir.17 = access ptr<struct.SolAccountInfo>(%temp.2) member 5;
    bool %temp.ssa_ir.16 = *ptr<bool>(%temp.ssa_ir.17);
    cbr bool(%temp.ssa_ir.16) block#3 else block#4;

block#2 doassert:
    assert_failure;

block#3 noassert:
    ptr<struct.SolAccountInfo[]> %temp.ssa_ir.18 = builtin: Accounts();
    ptr<struct.SolAccountInfo> %temp.3 = ptr<struct.SolAccountInfo[]>(%temp.ssa_ir.18)[uint32(3)];
    ptr<bool> %temp.ssa_ir.20 = access ptr<struct.SolAccountInfo>(%temp.3) member 6;
    bool %temp.ssa_ir.19 = *ptr<bool>(%temp.ssa_ir.20);
    cbr bool(%temp.ssa_ir.19) block#5 else block#6;

block#4 doassert:
    assert_failure;

block#5 noassert:
    ptr<struct.SolAccountInfo[]> %temp.ssa_ir.21 = builtin: Accounts();
    ptr<struct.SolAccountInfo> %temp.4 = ptr<struct.SolAccountInfo[]>(%temp.ssa_ir.21)[uint32(2)];
    ptr<bool> %temp.ssa_ir.23 = access ptr<struct.SolAccountInfo>(%temp.4) member 6;
    bool %temp.ssa_ir.22 = *ptr<bool>(%temp.ssa_ir.23);
    cbr bool(%temp.ssa_ir.22) block#7 else block#8;

block#6 doassert:
    assert_failure;

block#7 noassert:
    ptr<struct.SolAccountInfo[]> %temp.ssa_ir.24 = builtin: Accounts();
    ptr<struct.SolAccountInfo> %temp.6 = ptr<struct.SolAccountInfo[]>(%temp.ssa_ir.24)[uint32(2)];
    uint32 %index.temp.7 = 0;
    ptr<slice<bytes1>> %temp.ssa_ir.28 = access ptr<struct.SolAccountInfo>(%temp.6) member 2;
    ptr<slice<bytes1>> %temp.ssa_ir.27 = *ptr<slice<bytes1>>(%temp.ssa_ir.28);
    uint32 %temp.ssa_ir.26 = builtin: ArrayLength(ptr<slice<bytes1>>(%temp.ssa_ir.27));
    bool %temp.ssa_ir.25 = uint32(0) (u)>= uint32(%temp.ssa_ir.26);
    cbr bool(%temp.ssa_ir.25) block#9 else block#10;

block#8 doassert:
    assert_failure;

block#9 out_of_bounds:
    assert_failure;

block#10 in_bounds:
    bytes1 %temp.5 = 202;
    ptr<slice<bytes1>> %temp.ssa_ir.31 = access ptr<struct.SolAccountInfo>(%temp.6) member 2;
    ptr<slice<bytes1>> %temp.ssa_ir.30 = *ptr<slice<bytes1>>(%temp.ssa_ir.31);
    ptr<bytes1> %temp.ssa_ir.29 = ptr<slice<bytes1>>(%temp.ssa_ir.30)[uint32(%index.temp.7)];
    store bytes1(202) to ptr<bytes1>(%temp.ssa_ir.29);
    ptr<struct.SolAccountInfo[]> %temp.ssa_ir.32 = builtin: Accounts();
    ptr<struct.SolAccountInfo> %temp.9 = ptr<struct.SolAccountInfo[]>(%temp.ssa_ir.32)[uint32(3)];
    uint32 %index.temp.10 = 1;
    ptr<slice<bytes1>> %temp.ssa_ir.36 = access ptr<struct.SolAccountInfo>(%temp.9) member 2;
    ptr<slice<bytes1>> %temp.ssa_ir.35 = *ptr<slice<bytes1>>(%temp.ssa_ir.36);
    uint32 %temp.ssa_ir.34 = builtin: ArrayLength(ptr<slice<bytes1>>(%temp.ssa_ir.35));
    bool %temp.ssa_ir.33 = uint32(1) (u)>= uint32(%temp.ssa_ir.34);
    cbr bool(%temp.ssa_ir.33) block#11 else block#12;

block#11 out_of_bounds:
    assert_failure;

block#12 in_bounds:
    bytes1 %temp.8 = 254;
    ptr<slice<bytes1>> %temp.ssa_ir.39 = access ptr<struct.SolAccountInfo>(%temp.9) member 2;
    ptr<slice<bytes1>> %temp.ssa_ir.38 = *ptr<slice<bytes1>>(%temp.ssa_ir.39);
    ptr<bytes1> %temp.ssa_ir.37 = ptr<slice<bytes1>>(%temp.ssa_ir.38)[uint32(%index.temp.10)];
    store bytes1(254) to ptr<bytes1>(%temp.ssa_ir.37);
    ptr<struct.SolAccountInfo[]> %temp.ssa_ir.40 = builtin: Accounts();
    ptr<struct.SolAccountInfo> %temp.11 = ptr<struct.SolAccountInfo[]>(%temp.ssa_ir.40)[uint32(0)];
    ptr<ptr<uint64>> %temp.ssa_ir.43 = access ptr<struct.SolAccountInfo>(%temp.11) member 1;
    ptr<uint64> %temp.ssa_ir.42 = *ptr<ptr<uint64>>(%temp.ssa_ir.43);
    uint64 %temp.ssa_ir.41 = *ptr<uint64>(%temp.ssa_ir.42);
    return uint64(%temp.ssa_ir.41);"#,
    )
}

#[test]
fn test_assertion_using_require() {
    let src = r#"contract Test {
        function test(int32 num) public {
            require(num > 10, "sesa");
        }
    }"#;

    assert_polkadot_lir_str_eq(
        src,
        0,
        r#"public function sol#4 Test::Test::function::test__int32 (int32):
block#0 entry:
    int32 %num = int32(arg#0);
    bool %temp.ssa_ir.1 = int32(%num) > int32(10);
    cbr bool(%temp.ssa_ir.1) block#1 else block#2;

block#1 noassert:
    return;

block#2 doassert:
    ptr<slice<bytes1>> %temp.ssa_ir.2 = alloc ptr<slice<bytes1>>[uint32(9)] {08, c3, 79, a0, 10, 73, 65, 73, 61};
    assert_failure ptr<slice<bytes1>>(%temp.ssa_ir.2);"#,
    );
}

#[test]
fn test_call_1() {
    let src = r#"contract Test {
        function test(int32 num) public {
            check(num);
        }

        function check(int32 num) pure internal {
            require(num > 10, "sesa");
        }
    }"#;

    assert_polkadot_lir_str_eq(
        src,
        0,
        r#"public function sol#4 Test::Test::function::test__int32 (int32):
block#0 entry:
    int32 %num = int32(arg#0);
     = call function#1(int32(%num));
    return;"#,
    )
}

#[test]
fn test_return_data_and_return_code() {
    let src = r#"contract Test {
        function test() public {
        }
    }"#;

    assert_lir_str_eq_by_name(
        src,
        "polkadot_call_dispatch",
        r#"private function none polkadot_call_dispatch (ptr<uint8>, uint32, uint128, ptr<uint32>):
block#0 entry:
    uint32 %input_len.temp.4 = uint32(arg#1);
    uint128 %value.temp.5 = uint128(arg#2);
    ptr<uint8> %input_ptr.temp.6 = ptr<uint8>(arg#0);
    bool %temp.ssa_ir.8 = uint32(%input_len.temp.4) (u)< uint32(4);
    cbr bool(%temp.ssa_ir.8) block#2 else block#1;

block#1 start_dispatch:
    uint32 %selector.temp.7 = builtin: ReadFromBuffer(ptr<uint8>(%input_ptr.temp.6), uint32(0));
    uint32 %temp.ssa_ir.9 = uint32(arg#3);
    store uint32(%selector.temp.7) to uint32(%temp.ssa_ir.9);
    switch uint32(%selector.temp.7):
    case:    uint32(1845340408) => block#3
    default: block#2;

block#2 fb_or_recv:
    return_code "function selector invalid";

block#3 func_0_dispatch:
    bool %temp.ssa_ir.10 = uint128(%value.temp.5) (u)> uint128(0);
    cbr bool(%temp.ssa_ir.10) block#4 else block#5;

block#4 func_0_got_value:
    assert_failure;

block#5 func_0_no_value:
     = call function#0();
    ptr<struct.vector<uint8>> %temp.ssa_ir.11 = alloc ptr<struct.vector<uint8>>[uint32(0)];
    return_data ptr<struct.vector<uint8>>(%temp.ssa_ir.11) of length uint32(0);"#,
        Target::default_polkadot(),
    )
}

#[test]
fn test_value_transfer() {
    let src = r#"contract Test {
        function transfer(address payable addr, uint128 amount) public {
            addr.transfer(amount);
        }
    }"#;

    // Should be Polkadot
    assert_polkadot_lir_str_eq(
        src,
        0,
        r#"public function sol#4 Test::Test::function::transfer__address_uint128 (uint8[32], uint128):
block#0 entry:
    uint8[32] %addr = uint8[32](arg#0);
    uint128 %amount = uint128(arg#1);
    uint128 %temp.ssa_ir.3 = (cast uint128(%amount) to uint128);
    uint32 %success.temp.2 = value_transfer uint128(%temp.ssa_ir.3) to uint8[32](%addr);
    bool %temp.ssa_ir.4 = uint32(%success.temp.2) == uint32(0);
    cbr bool(%temp.ssa_ir.4) block#1 else block#2;

block#1 transfer_success:
    return;

block#2 transfer_fail:
    assert_failure;"#,
    )
}

#[test]
fn test_array_type_dynamic_storage() {
    let cfg_no = 0;

    // read the example.sol file
    let src = r#"contract s {
        int64[] a;
    
        function test() public {
            // push takes a single argument with the item to be added
            a.push(128);
            // push with no arguments adds 0
            a.push();
            // now we have two elements in our array, 128 and 0
            assert(a.length == 2);
            a[0] |= 64;
            // pop removes the last element
            a.pop();
            // you can assign the return value of pop
            int64 v = a.pop();
            assert(v == 192);
        }
    }"#;

    assert_solana_lir_str_eq(
        src,
        cfg_no,
        r#"public function sol#2 s::s::function::test ():
block#0 entry:
    int64 %temp.1 = push_storage uint32(16) int64(128);
    int64 %temp.2 = push_storage uint32(16) int64(0);
    uint32 %temp.ssa_ir.9 = storage_arr_len(uint32(16));
    bool %temp.ssa_ir.8 = uint32(%temp.ssa_ir.9) == uint32(2);
    cbr bool(%temp.ssa_ir.8) block#1 else block#2;

block#1 noassert:
    uint256 %index.temp.3 = 0;
    uint32 %temp.ssa_ir.12 = storage_arr_len(uint32(16));
    uint256 %temp.ssa_ir.11 = (zext uint32(%temp.ssa_ir.12) to uint256);
    bool %temp.ssa_ir.10 = uint256(0) (u)>= uint256(%temp.ssa_ir.11);
    cbr bool(%temp.ssa_ir.10) block#3 else block#4;

block#2 doassert:
    assert_failure;

block#3 out_of_bounds:
    assert_failure;

block#4 in_bounds:
    uint32 %temp.ssa_ir.14 = (trunc uint256(%index.temp.3) to uint32);
    storage_ptr<int64> %temp.ssa_ir.13 = uint32(16)[uint32(%temp.ssa_ir.14)];
    int64 %temp.4 = load_storage storage_ptr<int64>(%temp.ssa_ir.13);
    uint256 %index.temp.6 = 0;
    uint32 %temp.ssa_ir.17 = storage_arr_len(uint32(16));
    uint256 %temp.ssa_ir.16 = (zext uint32(%temp.ssa_ir.17) to uint256);
    bool %temp.ssa_ir.15 = uint256(0) (u)>= uint256(%temp.ssa_ir.16);
    cbr bool(%temp.ssa_ir.15) block#5 else block#6;

block#5 out_of_bounds:
    assert_failure;

block#6 in_bounds:
    int64 %temp.5 = int64(%temp.4) | int64(64);
    uint32 %temp.ssa_ir.19 = (trunc uint256(%index.temp.6) to uint32);
    storage_ptr<int64> %temp.ssa_ir.18 = uint32(16)[uint32(%temp.ssa_ir.19)];
    set_storage storage_ptr<int64>(%temp.ssa_ir.18) int64(%temp.5);
    pop_storage uint32(16);
    int64 %temp.7 = pop_storage uint32(16);
    int64 %v = int64(%temp.7);
    bool %temp.ssa_ir.20 = int64(%v) == int64(192);
    cbr bool(%temp.ssa_ir.20) block#7 else block#8;

block#7 noassert:
    return;

block#8 doassert:
    assert_failure;"#,
    )
}

#[test]
fn test_switch() {
    let src = r#"contract foo {
        function test(uint x) public pure {
            uint256 yy=0;
            assembly {
                let y := 5
                switch and(x, 3)
                    case 0 {
                        y := 5
                        x := 5
                    }
                    case 1 {
                        y := 7
                        x := 9
                    }
                    case 3 {
                        y := 10
                        x := 80
                    }
            }
        }
    }"#;

    assert_solana_lir_str_eq(
        src,
        0,
        r#"public function sol#2 foo::foo::function::test__uint256 (uint256):
block#0 entry:
    uint256 %x = uint256(arg#0);
    uint256 %y = 5;
    uint256 %temp.ssa_ir.3 = uint256(%x) & uint256(3);
    switch uint256(%temp.ssa_ir.3):
    case:    uint256(0) => block#2, 
    case:    uint256(1) => block#3, 
    case:    uint256(3) => block#4
    default: block#1;

block#1 end_switch:
    return;

block#2 case_0:
    uint256 %y = 5;
    uint256 %x = 5;
    br block#1;

block#3 case_1:
    uint256 %y = 7;
    uint256 %x = 9;
    br block#1;

block#4 case_2:
    uint256 %y = 10;
    uint256 %x = 80;
    br block#1;"#,
    )
}

#[test]
fn test_keccak256() {
    let src = r#"contract b {
        struct user {
            bool exists;
            address addr;
        }
        mapping(string => user) users;
        function add(string name, address addr) public {
            // This construction is not recommended, because it requires two hash calculations.
            // See the tip below.
            users[name].exists = true;
            users[name].addr = addr;
        }
    }"#;

    assert_polkadot_lir_str_eq(
        src,
        0,
        r#"public function sol#4 b::b::function::add__string_address (ptr<struct.vector<uint8>>, uint8[32]):
block#0 entry:
    ptr<struct.vector<uint8>> %name = ptr<struct.vector<uint8>>(arg#0);
    uint8[32] %addr = uint8[32](arg#1);
    bool %temp.2 = true;
    storage_ptr<mapping(struct.vector<uint8> => struct.0)> %temp.ssa_ir.5 = keccak256(uint256(0), ptr<struct.vector<uint8>>(%name));
    uint256 %temp.ssa_ir.4 = storage_ptr<mapping(struct.vector<uint8> => struct.0)>(%temp.ssa_ir.5) (of)+ uint256(0);
    set_storage uint256(%temp.ssa_ir.4) true;
    uint8[32] %temp.3 = uint8[32](arg#1);
    storage_ptr<mapping(struct.vector<uint8> => struct.0)> %temp.ssa_ir.7 = keccak256(uint256(0), ptr<struct.vector<uint8>>(%name));
    uint256 %temp.ssa_ir.6 = storage_ptr<mapping(struct.vector<uint8> => struct.0)>(%temp.ssa_ir.7) (of)+ uint256(1);
    set_storage uint256(%temp.ssa_ir.6) uint8[32](%temp.3);
    return;"#,
    )
}

#[test]
fn test_internal_function_cfg() {
    let src = r#"contract A {
      function foo(uint a) internal returns (uint) {
          return a+2;
      }

      function bar(uint b) public returns (uint) {
        function (uint) returns (uint) fPtr = foo;
        return fPtr(b);
      }
    }"#;

    assert_polkadot_lir_str_eq(
        src,
        1,
        r#"public function sol#5 A::A::function::bar__uint256 (uint256) returns (uint256):
block#0 entry:
    uint256 %b = uint256(arg#0);
    ptr<function (uint256) returns (uint256)> %temp.ssa_ir.6 = function#0;
    ptr<function (uint256) returns (uint256)> %fPtr = (cast ptr<function (uint256) returns (uint256)>(%temp.ssa_ir.6) to ptr<function (uint256) returns (uint256)>);
    uint256 %.temp.5 = call ptr<function (uint256) returns (uint256)>(%fPtr)(uint256(%b));
    return uint256(%.temp.5);"#,
    )
}

#[test]
fn test_sign_ext() {
    let src = r#"contract Test {
        function test(int32 a) public returns (int128) {
            return int128(a);
        }
    }"#;

    assert_solana_lir_str_eq(
        src,
        0,
        r#"public function sol#2 Test::Test::function::test__int32 (int32) returns (int128):
block#0 entry:
    int32 %a = int32(arg#0);
    int128 %temp.ssa_ir.2 = (sext int32(%a) to int128);
    return int128(%temp.ssa_ir.2);"#,
    )
}

#[test]
fn test_string_compare() {
    let src = r#"contract Test {
        function test(string a, string b) public returns (bool) {
            return a == b;
        }
    }"#;

    assert_solana_lir_str_eq(
        src,
        0,
        r#"public function sol#2 Test::Test::function::test__string_string (ptr<struct.vector<uint8>>, ptr<struct.vector<uint8>>) returns (bool):
block#0 entry:
    ptr<struct.vector<uint8>> %a = ptr<struct.vector<uint8>>(arg#0);
    ptr<struct.vector<uint8>> %b = ptr<struct.vector<uint8>>(arg#1);
    bool %temp.ssa_ir.3 = strcmp(ptr<struct.vector<uint8>>(%a), ptr<struct.vector<uint8>>(%b));
    return bool(%temp.ssa_ir.3);"#,
    )
}

#[test]
fn test_const_array() {
    let src = r#"contract Test {
        uint32[5] constant arr = [1, 2, 3, 4, 5];
        function test() public returns (uint32) {
            return arr[0];
        }
    }"#;

    assert_solana_lir_str_eq(
        src,
        0,
        r#"public function sol#2 Test::Test::function::test () returns (uint32):
block#0 entry:
    uint32 %index.temp.1 = 0;
    bool %temp.ssa_ir.2 = uint32(0) (u)>= uint32(5);
    cbr bool(%temp.ssa_ir.2) block#1 else block#2;

block#1 out_of_bounds:
    assert_failure;

block#2 in_bounds:
    ptr<uint32[5]> %temp.ssa_ir.5 = const ptr<uint32[5]> [uint32(1), uint32(2), uint32(3), uint32(4), uint32(5)];
    ptr<uint32> %temp.ssa_ir.4 = ptr<uint32[5]>(%temp.ssa_ir.5)[uint32(0)];
    uint32 %temp.ssa_ir.3 = *ptr<uint32>(%temp.ssa_ir.4);
    return uint32(%temp.ssa_ir.3);"#,
    )
}

#[test]
fn test_account_meta() {
    let src = r#"import 'solana';
    contract creator {
        @mutableSigner(data_account_to_initialize)
        @mutableSigner(payer)
        function create_with_metas() external {
            AccountMeta[3] metas = [
                AccountMeta({
                    pubkey: tx.accounts.data_account_to_initialize.key,
                    is_signer: true, 
                    is_writable: true}),
                AccountMeta({
                    pubkey: tx.accounts.payer.key,
                    is_signer: true,
                    is_writable: true}),
                AccountMeta({
                    pubkey: address"11111111111111111111111111111111",
                    is_writable: false,
                    is_signer: false})
            ];
            Child.new{accounts: metas}();        
            Child.use_metas{accounts: []}();
        }
    }
    @program_id("Chi1d5XD6nTAp2EyaNGqMxZzUjh6NvhXRxbGHP3D1RaT")
    contract Child {
        @payer(payer)
        constructor() {
            print("In child constructor");
        }
        function use_metas() pure public {
            print("I am using metas");
        }
    }"#;

    assert_solana_lir_str_eq(
        src,
        0,
        r#"public function sol#2 creator::creator::function::create_with_metas ():
block#0 entry:
    ptr<struct.SolAccountInfo[]> %temp.ssa_ir.14 = builtin: Accounts();
    ptr<struct.SolAccountInfo> %temp.10 = ptr<struct.SolAccountInfo[]>(%temp.ssa_ir.14)[uint32(0)];
    ptr<struct.SolAccountInfo[]> %temp.ssa_ir.15 = builtin: Accounts();
    ptr<struct.SolAccountInfo> %temp.11 = ptr<struct.SolAccountInfo[]>(%temp.ssa_ir.15)[uint32(1)];
    ptr<ptr<uint8[32]>> %temp.ssa_ir.18 = access ptr<struct.SolAccountInfo>(%temp.10) member 0;
    ptr<uint8[32]> %temp.ssa_ir.17 = *ptr<ptr<uint8[32]>>(%temp.ssa_ir.18);
    ptr<struct.SolAccountMeta> %temp.ssa_ir.16 = struct { ptr<uint8[32]>(%temp.ssa_ir.17), true, true };
    ptr<ptr<uint8[32]>> %temp.ssa_ir.21 = access ptr<struct.SolAccountInfo>(%temp.11) member 0;
    ptr<uint8[32]> %temp.ssa_ir.20 = *ptr<ptr<uint8[32]>>(%temp.ssa_ir.21);
    ptr<struct.SolAccountMeta> %temp.ssa_ir.19 = struct { ptr<uint8[32]>(%temp.ssa_ir.20), true, true };
    ptr<uint8[32]> %temp.ssa_ir.23 = &uint8[32](0);
    ptr<struct.SolAccountMeta> %temp.ssa_ir.22 = struct { ptr<uint8[32]>(%temp.ssa_ir.23), false, false };
    ptr<struct.SolAccountMeta[3]> %metas = ptr<struct.SolAccountMeta[3]> [ptr<struct.SolAccountMeta>(%temp.ssa_ir.16), ptr<struct.SolAccountMeta>(%temp.ssa_ir.19), ptr<struct.SolAccountMeta>(%temp.ssa_ir.22)];
    ptr<struct.vector<uint8>> %abi_encoded.temp.12 = alloc ptr<struct.vector<uint8>>[uint32(8)];
    bytes8 %temp.ssa_ir.24 = bytes8 hex"87_2c_cd_c6_19_01_48_bc";
    write_buf ptr<struct.vector<uint8>>(%abi_encoded.temp.12) offset:uint32(0) value:bytes8(%temp.ssa_ir.24);
    _ = call_ext [regular] address:uint8[32](78642644713358252795404932596995255556623171005675782810573618728006773308276) payload:ptr<struct.vector<uint8>>(%abi_encoded.temp.12) value:uint64(0) gas:uint64(0) accounts:ptr<struct.SolAccountMeta[3]>(%metas) seeds:_ contract_no:1, function_no:3 flags:_;
    ptr<struct.vector<uint8>> %abi_encoded.temp.13 = alloc ptr<struct.vector<uint8>>[uint32(8)];
    bytes8 %temp.ssa_ir.25 = bytes8 hex"97_f8_3c_a2_18_9f_26_9d";
    write_buf ptr<struct.vector<uint8>>(%abi_encoded.temp.13) offset:uint32(0) value:bytes8(%temp.ssa_ir.25);
    _ = call_ext [regular] address:uint8[32](78642644713358252795404932596995255556623171005675782810573618728006773308276) payload:ptr<struct.vector<uint8>>(%abi_encoded.temp.13) value:uint64(0) gas:uint64(0) accounts:none seeds:_ contract_no:1, function_no:4 flags:_;
    return;"#,
    )
}

#[test]
fn test_constructor() {
    let src = r#"
    contract B {
      A aa;
      function test(uint a) public {
          aa = new A(a);
      }
    }
    contract A {
      uint public a;
      constructor(uint b) {
        a = b;
      }
    }
    "#;

    assert_polkadot_lir_str_eq(
        src,
        0,
        r#"public function sol#4 B::B::function::test__uint256 (uint256):
block#0 entry:
    uint256 %a = uint256(arg#0);
    ptr<struct.vector<uint8>> %abi_encoded.temp.18 = alloc ptr<struct.vector<uint8>>[uint32(36)];
    uint32 %temp.ssa_ir.20 = uint32 hex"58_16_c4_25";
    write_buf ptr<struct.vector<uint8>>(%abi_encoded.temp.18) offset:uint32(0) value:uint32(%temp.ssa_ir.20);
    write_buf ptr<struct.vector<uint8>>(%abi_encoded.temp.18) offset:uint32(4) value:uint256(%a);
    uint32 %success.temp.17, uint8[32] %temp.16 = constructor(no: 6, contract_no:1) salt:_ value:_ gas:uint64(0) address:_ seeds:_ encoded-buffer:ptr<struct.vector<uint8>>(%abi_encoded.temp.18) accounts:absent
    switch uint32(%success.temp.17):
    case:    uint32(0) => block#1, 
    case:    uint32(2) => block#2
    default: block#3;

block#1 ret_success:
    uint8[32] %temp.19 = uint8[32](%temp.16);
    set_storage uint256(0) uint8[32](%temp.19);
    return;

block#2 ret_bubble:
    ptr<struct.vector<uint8>> %temp.ssa_ir.21 = (extern_call_ret_data);
    assert_failure ptr<struct.vector<uint8>>(%temp.ssa_ir.21);

block#3 ret_no_data:
    assert_failure;"#,
    )
}

#[test]
fn test_external_fn() {
    let src = r#"contract Testing {
        function testExternalFunction(
            bytes memory buffer
        ) public view returns (bytes8, address) {
            function(uint8) external returns (int8) fPtr = abi.decode(
                buffer,
                (function(uint8) external returns (int8))
            );
            return (fPtr.selector, fPtr.address);
        }
    }
    "#;

    assert_solana_lir_str_eq(
        src,
        0,
        r#"public function sol#2 Testing::Testing::function::testExternalFunction__bytes (ptr<struct.vector<uint8>>) returns (bytes8, uint8[32]):
block#0 entry:
    ptr<struct.vector<uint8>> %buffer = ptr<struct.vector<uint8>>(arg#0);
    uint32 %temp.4 = builtin: ArrayLength(ptr<struct.vector<uint8>>(%buffer));
    bool %temp.ssa_ir.5 = uint32(40) (u)<= uint32(%temp.4);
    cbr bool(%temp.ssa_ir.5) block#1 else block#2;

block#1 inbounds:
    bool %temp.ssa_ir.6 = uint32(40) (u)< uint32(%temp.4);
    cbr bool(%temp.ssa_ir.6) block#3 else block#4;

block#2 out_of_bounds:
    assert_failure;

block#3 not_all_bytes_read:
    assert_failure;

block#4 buffer_read:
    uint64 %temp.ssa_ir.9 = builtin: ReadFromBuffer(ptr<struct.vector<uint8>>(%buffer), uint32(0));
    uint8[32] %temp.ssa_ir.10 = builtin: ReadFromBuffer(ptr<struct.vector<uint8>>(%buffer), uint32(8));
    ptr<struct.ExternalFunction> %temp.ssa_ir.8 = struct { uint64(%temp.ssa_ir.9), uint8[32](%temp.ssa_ir.10) };
    ptr<struct.ExternalFunction> %temp.ssa_ir.7 = (cast ptr<struct.ExternalFunction>(%temp.ssa_ir.8) to ptr<struct.ExternalFunction>);
    ptr<struct.ExternalFunction> %fPtr = (cast ptr<struct.ExternalFunction>(%temp.ssa_ir.7) to ptr<struct.ExternalFunction>);
    ptr<uint64> %temp.ssa_ir.13 = access ptr<struct.ExternalFunction>(%fPtr) member 0;
    uint64 %temp.ssa_ir.12 = *ptr<uint64>(%temp.ssa_ir.13);
    bytes8 %temp.ssa_ir.11 = (cast uint64(%temp.ssa_ir.12) to bytes8);
    ptr<uint8[32]> %temp.ssa_ir.15 = access ptr<struct.ExternalFunction>(%fPtr) member 1;
    uint8[32] %temp.ssa_ir.14 = *ptr<uint8[32]>(%temp.ssa_ir.15);
    return bytes8(%temp.ssa_ir.11), uint8[32](%temp.ssa_ir.14);"#,
    )
}

#[test]
fn test_push_pop_mem() {
    let src = r#"
    contract foo {
        struct s {
            int32 f1;
            bool f2;
        }
        function test() public {
            s[] bar = new s[](0);
            s memory n = bar.push();
            bar.pop();
        }
    }
    "#;

    assert_solana_lir_str_eq(
        src,
        0,
        r#"public function sol#2 foo::foo::function::test ():
block#0 entry:
    uint32 %array_length.temp.2 = 0;
    ptr<struct.0[]> %bar = alloc ptr<struct.0[]>[uint32(0)];
    ptr<struct.0> %temp.ssa_ir.5 = struct {  };
    ptr<struct.0> %temp.3 = push_mem ptr<struct.0[]>(%bar) ptr<struct.0>(%temp.ssa_ir.5);
    uint32 %array_length.temp.2 = 1;
    ptr<struct.0> %temp.4 = pop_mem ptr<struct.0[]>(%bar);
    uint32 %array_length.temp.2 = 0;
    return;"#,
    )
}

#[test]
fn test_math() {
    let src = r#"contract store {
        enum enum_bar { bar1, bar2, bar3, bar4 }
        uint64 u64;
        uint32 u32;
        int16 i16;
        int256 i256;
        uint256 u256;
        string str;
        bytes bs = hex"b00b1e";
        bytes4 fixedbytes;
        enum_bar bar;
        function do_ops() public {
            unchecked {
                // u64 will overflow to 1
                u64 += 2;
                u32 &= 0xffff;
                // another overflow
                i16 += 1;
                i256 ^= 1;
                u256 *= 600;
                str = "";
                bs[1] = 0xff;
                // make upper case
                fixedbytes |= 0x20202020;
                bar = enum_bar.bar4;
            }
        }
    }"#;

    assert_solana_lir_str_eq(
        src,
        0,
        r#"public function sol#2 store::store::function::do_ops ():
block#0 entry:
    uint64 %temp.0 = load_storage uint32(16);
    uint64 %temp.1 = uint64(%temp.0) (of)+ uint64(2);
    set_storage uint32(16) uint64(%temp.1);
    uint32 %temp.2 = load_storage uint32(24);
    uint32 %temp.3 = uint32(%temp.2) & uint32(65535);
    set_storage uint32(24) uint32(%temp.3);
    int16 %temp.4 = load_storage uint32(28);
    int16 %temp.5 = int16(%temp.4) (of)+ int16(1);
    set_storage uint32(28) int16(%temp.5);
    int256 %temp.6 = load_storage uint32(32);
    int256 %temp.7 = int256(%temp.6) ^ int256(1);
    set_storage uint32(32) int256(%temp.7);
    uint256 %temp.8 = load_storage uint32(64);
    uint64 %temp.ssa_ir.16 = (trunc uint256(%temp.8) to uint64);
    uint64 %temp.ssa_ir.15 = uint64(%temp.ssa_ir.16) (of)* uint64(600);
    uint256 %temp.9 = (zext uint64(%temp.ssa_ir.15) to uint256);
    set_storage uint32(64) uint256(%temp.9);
    ptr<struct.vector<uint8>> %temp.10 = alloc ptr<slice<bytes1>>[uint32(0)] {};
    set_storage uint32(96) ptr<struct.vector<uint8>>(%temp.10);
    bytes1 %temp.11 = 255;
    set_storage_bytes uint32(100) offset:uint32(1) value:bytes1(255);
    bytes4 %temp.12 = load_storage uint32(104);
    bytes4 %temp.13 = bytes4(%temp.12) | bytes4(538976288);
    set_storage uint32(104) bytes4(%temp.13);
    uint8 %temp.14 = 3;
    set_storage uint32(108) uint8(3);
    return;"#,
    )
}

#[test]
fn test_byte_cast() {
    let src = r#"contract Cast {
        function test(uint256 num) public returns (bytes) {
            bytes smol_buf = new bytes(num);
            bytes32 b32 = bytes32(smol_buf);
            return b32;
        }
    }"#;

    assert_solana_lir_str_eq(
        src,
        0,
        r#"public function sol#2 Cast::Cast::function::test__uint256 (uint256) returns (ptr<struct.vector<uint8>>):
block#0 entry:
    uint256 %num = uint256(arg#0);
    uint256 %value.temp.4 = uint256(arg#0);
    bool %temp.ssa_ir.5 = uint256(%value.temp.4) (u)>= uint256(4294967296);
    cbr bool(%temp.ssa_ir.5) block#1 else block#2;

block#1 out_of_bounds:
    assert_failure;

block#2 in_bounds:
    uint32 %temp.ssa_ir.6 = (trunc uint256(%value.temp.4) to uint32);
    ptr<struct.vector<uint8>> %smol_buf = alloc ptr<struct.vector<uint8>>[uint32(%temp.ssa_ir.6)];
    bytes32 %b32 = (cast ptr<struct.vector<uint8>>(%smol_buf) to bytes32);
    ptr<struct.vector<uint8>> %temp.ssa_ir.7 = (cast bytes32(%b32) to ptr<struct.vector<uint8>>);
    return ptr<struct.vector<uint8>>(%temp.ssa_ir.7);"#,
    )
}

#[test]
fn test_signed_modulo() {
    let src = r#"contract SignedModulo {
        function test(int256 a, int256 b) public returns (int256) {
            int256 c = a % b;
            return c;
        }
    }"#;

    assert_solana_lir_str_eq(
        src,
        0,
        r#"public function sol#2 SignedModulo::SignedModulo::function::test__int256_int256 (int256, int256) returns (int256):
block#0 entry:
    int256 %a = int256(arg#0);
    int256 %b = int256(arg#1);
    int256 %c = int256(%a) % int256(%b);
    return int256(%c);"#,
    )
}

#[test]
fn test_compare() {
    let src = r#"contract example {
        int16 stored;
    
        function func(int256 x) public {
            if (x < type(int16).min || x > type(int16).max) {
                revert("value will not fit");
            }
    
            stored = int16(x);
        }
    }
    "#;

    assert_solana_lir_str_eq(
        src,
        0,
        r#"public function sol#2 example::example::function::func__int256 (int256):
block#0 entry:
    int256 %x = int256(arg#0);
    bool %or.temp.1 = true;
    bool %temp.ssa_ir.3 = int256(%x) < int256(-32768);
    cbr bool(%temp.ssa_ir.3) block#2 else block#1;

block#1 or_right_side:
    bool %or.temp.1 = int256(%x) > int256(32767);
    br block#2;

block#2 or_end:
    cbr bool(%or.temp.1) block#3 else block#4;

block#3 then:
    assert_failure;

block#4 endif:
    int16 %temp.2 = (trunc int256(%x) to int16);
    set_storage uint32(16) int16(%temp.2);
    return;"#,
    )
}

#[test]
fn test_array() {
    let src = r#"
    contract C {
        function testVec() public pure returns (uint32) {
            uint32[3] vec = [1, 2, 3];
            return vec.length;
        }
    }"#;

    assert_solana_lir_str_eq(
        src,
        0,
        r#"public function sol#2 C::C::function::testVec () returns (uint32):
block#0 entry:
    ptr<uint32[3]> %vec = ptr<uint32[3]> [uint32(1), uint32(2), uint32(3)];
    return uint32(3);"#,
    )
}

#[test]
fn test_clear_storage() {
    let src = r#"
    struct S {
        function() e;
    }

    contract C {
        S s;
        function test() public {
            delete s.e;
        }
    }"#;

    assert_solana_lir_str_eq(
        src,
        0,
        r#"public function sol#2 C::C::function::test ():
block#0 entry:
    clear_storage uint32(64);
    return;"#,
    )
}

#[test]
fn test_call_ext() {
    let src = r#"
    contract adult {
        function test(address id) external {
            hatchling.new{program_id: id}("luna");
        }
    }
    contract hatchling {
        string name;
        constructor(string id) payable {
            name = id;
        }
    }
    "#;

    assert_solana_lir_str_eq(
        src,
        0,
        r#"public function sol#2 adult::adult::function::test__address (uint8[32]):
block#0 entry:
    uint8[32] %id = uint8[32](arg#0);
    ptr<struct.vector<uint8>> %temp.ssa_ir.15 = alloc ptr<struct.vector<uint8>>[uint32(4)] {6c, 75, 6e, 61};
    uint32 %temp.ssa_ir.14 = builtin: ArrayLength(ptr<struct.vector<uint8>>(%temp.ssa_ir.15));
    uint32 %temp.ssa_ir.13 = uint32(%temp.ssa_ir.14) + uint32(4);
    uint32 %temp.ssa_ir.12 = uint32(8) + uint32(%temp.ssa_ir.13);
    ptr<struct.vector<uint8>> %abi_encoded.temp.10 = alloc ptr<struct.vector<uint8>>[uint32(%temp.ssa_ir.12)];
    bytes8 %temp.ssa_ir.16 = bytes8 hex"87_2c_cd_c6_19_01_48_bc";
    write_buf ptr<struct.vector<uint8>>(%abi_encoded.temp.10) offset:uint32(0) value:bytes8(%temp.ssa_ir.16);
    ptr<struct.vector<uint8>> %temp.ssa_ir.17 = alloc ptr<struct.vector<uint8>>[uint32(4)] {6c, 75, 6e, 61};
    uint32 %temp.11 = builtin: ArrayLength(ptr<struct.vector<uint8>>(%temp.ssa_ir.17));
    write_buf ptr<struct.vector<uint8>>(%abi_encoded.temp.10) offset:uint32(8) value:uint32(%temp.11);
    ptr<struct.vector<uint8>> %temp.ssa_ir.18 = alloc ptr<struct.vector<uint8>>[uint32(4)] {6c, 75, 6e, 61};
    ptr<uint8> %temp.ssa_ir.19 = ptr_add(ptr<struct.vector<uint8>>(%abi_encoded.temp.10), uint32(12));
    memcopy ptr<struct.vector<uint8>>(%temp.ssa_ir.18) to ptr<uint8>(%temp.ssa_ir.19) for uint32(%temp.11) bytes;
    ptr<struct.SolAccountInfo[]> %temp.ssa_ir.25 = builtin: Accounts();
    ptr<struct.SolAccountInfo> %temp.ssa_ir.24 = ptr<struct.SolAccountInfo[]>(%temp.ssa_ir.25)[uint32(1)];
    ptr<ptr<uint8[32]>> %temp.ssa_ir.23 = access ptr<struct.SolAccountInfo>(%temp.ssa_ir.24) member 0;
    ptr<uint8[32]> %temp.ssa_ir.22 = *ptr<ptr<uint8[32]>>(%temp.ssa_ir.23);
    ptr<struct.SolAccountMeta> %temp.ssa_ir.21 = struct { ptr<uint8[32]>(%temp.ssa_ir.22), true, false };
    ptr<struct.SolAccountMeta[1]> %temp.ssa_ir.20 = ptr<struct.SolAccountMeta[1]> [ptr<struct.SolAccountMeta>(%temp.ssa_ir.21)];
    _ = call_ext [regular] address:uint8[32](%id) payload:ptr<struct.vector<uint8>>(%abi_encoded.temp.10) value:uint64(0) gas:uint64(0) accounts:ptr<struct.SolAccountMeta[1]>(%temp.ssa_ir.20) seeds:_ contract_no:1, function_no:3 flags:_;
    return;"#,
    )
}

#[test]
fn test_emit_event() {
    let src = r#"
    contract mytokenEvent {
        event Debugging(int b);
    
        function test() public {
            emit Debugging(1);
        }
    }"#;

    assert_solana_lir_str_eq(
        src,
        0,
        r#"public function sol#2 mytokenEvent::mytokenEvent::function::test ():
block#0 entry:
    ptr<struct.vector<uint8>> %abi_encoded.temp.0 = alloc ptr<struct.vector<uint8>>[uint32(40)];
    bytes8 %temp.ssa_ir.1 = bytes8 hex"cc_c9_89_03_bd_da_d5_98";
    write_buf ptr<struct.vector<uint8>>(%abi_encoded.temp.0) offset:uint32(0) value:bytes8(%temp.ssa_ir.1);
    write_buf ptr<struct.vector<uint8>>(%abi_encoded.temp.0) offset:uint32(8) value:int256(1);
    emit event#0 to topics[], data: ptr<struct.vector<uint8>>(%abi_encoded.temp.0);
    return;"#,
    )
}

#[test]
fn test_fmt_string_and_print() {
    let src = r#"contract Test {
        function test() public {
            print("Number: {}".format(123));
        }
    }"#;

    assert_solana_lir_str_eq(
        src,
        0,
        r#"public function sol#2 Test::Test::function::test ():
block#0 entry:
    ptr<struct.vector<uint8>> %temp.ssa_ir.2 = ptr<struct.vector<uint8>> hex"4e_75_6d_62_65_72_3a_20";
    ptr<struct.vector<uint8>> %temp.ssa_ir.1 = fmt_str(ptr<struct.vector<uint8>>(%temp.ssa_ir.2), uint8(123));
    print ptr<struct.vector<uint8>>(%temp.ssa_ir.1);
    return;"#,
    )
}

#[test]
fn test_bitewise_not() {
    let src = r#"contract Test {
        function byte_wise_not(bytes14 a) public pure returns (bytes14) {
            return ~a;
        }
    }"#;

    assert_solana_lir_str_eq(
        src,
        0,
        r#"public function sol#2 Test::Test::function::byte_wise_not__bytes14 (bytes14) returns (bytes14):
block#0 entry:
    bytes14 %a = bytes14(arg#0);
    bytes14 %temp.ssa_ir.2 = ~bytes14(%a);
    return bytes14(%temp.ssa_ir.2);"#,
    )
}
