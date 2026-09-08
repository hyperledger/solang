// SPDX-License-Identifier: Apache-2.0

#![cfg(test)]

mod data_account;

use crate::sema::ast::{Expression, ExternalCallAccounts, Parameter, Statement, TryCatch, Type};
use crate::sema::yul::ast::InlineAssembly;
use crate::{parse_and_resolve, sema::ast, FileResolver, Target};
use solang_parser::pt::Loc;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

pub(crate) fn parse(src: &'static str) -> ast::Namespace {
    let mut cache = FileResolver::default();
    cache.set_file_contents("test.sol", src.to_string());

    let ns = parse_and_resolve(OsStr::new("test.sol"), &mut cache, Target::EVM);
    ns
}

#[test]
fn test_statement_reachable() {
    let loc = Loc::File(0, 1, 2);
    let test_cases: Vec<(Statement, bool)> = vec![
        (Statement::Underscore(loc), true),
        (
            Statement::Destructure(loc, vec![], Expression::BoolLiteral { loc, value: true }),
            true,
        ),
        (
            Statement::VariableDecl(
                loc,
                0,
                Parameter {
                    loc,
                    id: None,
                    ty: Type::Bool,
                    ty_loc: None,
                    indexed: false,
                    readonly: false,
                    infinite_size: false,
                    recursive: false,
                    annotation: None,
                },
                None,
            ),
            true,
        ),
        (
            Statement::Emit {
                loc,
                event_no: 0,
                event_loc: Loc::Builtin,
                args: vec![],
            },
            true,
        ),
        (
            Statement::Delete(
                loc,
                Type::Bool,
                Expression::BoolLiteral { loc, value: true },
            ),
            true,
        ),
        (Statement::Continue(loc), false),
        (Statement::Break(loc), false),
        (Statement::Return(loc, None), false),
        (
            Statement::If(
                loc,
                false,
                Expression::BoolLiteral { loc, value: false },
                vec![],
                vec![],
            ),
            false,
        ),
        (
            Statement::While(
                loc,
                true,
                Expression::BoolLiteral { loc, value: false },
                vec![],
            ),
            true,
        ),
        (
            Statement::DoWhile(
                loc,
                false,
                vec![],
                Expression::BoolLiteral { loc, value: true },
            ),
            false,
        ),
        (
            Statement::Expression(loc, true, Expression::BoolLiteral { loc, value: false }),
            true,
        ),
        (
            Statement::For {
                loc,
                reachable: false,
                init: vec![],
                cond: None,
                next: None,
                body: vec![],
            },
            false,
        ),
        (
            Statement::TryCatch(
                loc,
                true,
                TryCatch {
                    expr: Expression::BoolLiteral { loc, value: false },
                    returns: vec![],
                    ok_stmt: vec![],
                    errors: vec![],
                    catch_all: None,
                },
            ),
            true,
        ),
        (
            Statement::Assembly(
                InlineAssembly {
                    loc,
                    memory_safe: false,
                    body: vec![],
                    functions: std::ops::Range { start: 0, end: 0 },
                },
                false,
            ),
            false,
        ),
    ];

    for (test_case, expected) in test_cases {
        assert_eq!(test_case.reachable(), expected);
    }
}

#[test]
fn constant_overflow_checks() {
    let file = r#"
    contract test_contract {
        function test_params(uint8 usesa, int8 sesa) public returns(uint8) {
            return usesa + uint8(sesa);
        }

        function test_add(int8 input) public returns (uint8) {
            // value 133 does not fit into type int8.
            int8 add_ovf = 127 + 6;

            // negative value -1 does not fit into type uint8. Cannot implicitly convert signed literal to unsigned type.
            uint8 negative = 3 - 4;

            // value 133 does not fit into type int8.
            int8 mixed = 126 + 7 + input;

            // negative value -1 does not fit into type uint8. Cannot implicitly convert signed literal to unsigned type.
            return 1 - 2;
        }

        function test_mul(int8 input) public {
            // value 726 does not fit into type int8.
            int8 mul_ovf = 127 * 6;

            // value 882 does not fit into type int8.
            int8 mixed = 126 * 7 * input;
        }

        function test_shift(int8 input) public {
            // value 128 does not fit into type int8.
            // warning: left shift by 7 may overflow the final result.
            int8 mul_ovf = 1 << 7;

            // value 128 does not fit into type int8.
            // warning: left shift by 7 may overflow the final result
            int8 mixed = (1 << 7) + input;
        }

        function test_call() public {
            // negative value -1 does not fit into type uint8. Cannot implicitly convert signed literal to unsigned type.
            // value 129 does not fit into type int8.
            test_params(1 - 2, 127 + 2);

            // negative value -1 does not fit into type uint8. Cannot implicitly convert signed literal to unsigned type.
            // value 129 does not fit into type int8.
            test_params({usesa: 1 - 2, sesa: 127 + 2});
        }

        function test_builtin (bytes input) public{

            // value 4294967296 does not fit into type uint32.
            int16 sesa = input.readInt16LE(4294967296);
        }

        function test_for_loop () public {
            for (int8 i = 125 + 5; i < 300 ; i++) {
            }
        }

        function composite(int8 a, bytes input) public{

            uint8 sesa = 500- 400 + test_params(100+200, 0) + (200+101) + input.readUint8(4294967296);
            int8 seas = (120 + 120) + a + (120 + 125);

            // no diagnostic
            uint8 b = 255 - 255/5 ;

            // value 260 does not fit into type uint8.
            uint8 shift_r = (120 >> 2) + 230;

            // value 261 does not fit into type uint8.
            uint8 mod_test = 254 + (500%17);

            // value 269 does not fit into type uint8.
            uint8 bb = 320 - (255/5) ;

            // left shift by 7 may overflow the final result
            uint8 shift_warning = (1 << 9) - 300;

            int8 bitwise_or = (250 | 5) - 150;

            // value 155 does not fit into type int8.
            int8 bitwise_or_ovf = (250 | 5) - 100;

            uint8 bitwise_and = 1000 & 5 ;

            // value 262 does not fit into type uint8.
            uint8 bitwise_and_ovf = (1000 & 255) + 30 ;

            uint8 bitwise_xor = 1000 ^ 256;

            // divide by zero
            uint8 div_zero= 3 / (1-1);

            // divide by zero
            uint8 div_zeroo = (300-50) % 0;

            // shift by negative number not allowed.
            uint8 shift_left_neg = 120 << -1;
            uint8 shift_right_neg = 120 >> -1;

            // power by -1 is not allowed.
            uint8 pow = 12 ** -1;

            // large shift not allowed
            int x = 1 >> 14676683207225698178084221555689649093015162623576402558976;

        }
    }

        "#;
    let ns = parse(file);
    let errors = ns.diagnostics.errors();
    let warnings = ns.diagnostics.warnings();

    assert_eq!(errors[0].message, "value 133 does not fit into type int8.");
    assert_eq!(errors[1].message, "negative value -1 does not fit into type uint8. Cannot implicitly convert signed literal to unsigned type.");
    assert_eq!(errors[2].message, "value 133 does not fit into type int8.");
    assert_eq!(errors[3].message, "negative value -1 does not fit into type uint8. Cannot implicitly convert signed literal to unsigned type.");
    assert_eq!(errors[4].message, "value 762 does not fit into type int8.");
    assert_eq!(errors[5].message, "value 882 does not fit into type int8.");
    assert_eq!(errors[6].message, "value 128 does not fit into type int8.");
    assert_eq!(errors[7].message, "value 128 does not fit into type int8.");
    assert_eq!(errors[8].message, "negative value -1 does not fit into type uint8. Cannot implicitly convert signed literal to unsigned type.");
    assert_eq!(errors[9].message, "value 129 does not fit into type int8.");
    assert_eq!(errors[10].message, "negative value -1 does not fit into type uint8. Cannot implicitly convert signed literal to unsigned type.");
    assert_eq!(errors[11].message, "value 129 does not fit into type int8.");
    assert_eq!(
        errors[12].message,
        "value 4294967296 does not fit into type uint32."
    );
    assert_eq!(errors[13].message, "value 130 does not fit into type int8.");
    assert_eq!(
        errors[14].message,
        "value 300 does not fit into type uint8."
    );
    assert_eq!(
        errors[15].message,
        "value 301 does not fit into type uint8."
    );
    assert_eq!(
        errors[16].message,
        "value 4294967296 does not fit into type uint32."
    );
    assert_eq!(errors[17].message, "value 240 does not fit into type int8.");
    assert_eq!(errors[18].message, "value 245 does not fit into type int8.");
    assert_eq!(
        errors[19].message,
        "value 260 does not fit into type uint8."
    );
    assert_eq!(
        errors[20].message,
        "value 261 does not fit into type uint8."
    );
    assert_eq!(
        errors[21].message,
        "value 269 does not fit into type uint8."
    );
    assert_eq!(errors[22].message, "value 155 does not fit into type int8.");
    assert_eq!(
        errors[23].message,
        "value 262 does not fit into type uint8."
    );

    assert_eq!(
        errors[24].message,
        "value 744 does not fit into type uint8."
    );
    assert_eq!(errors[25].message, "divide by zero");
    assert_eq!(errors[26].message, "divide by zero");
    assert_eq!(errors[27].message, "left shift by -1 is not possible");
    assert_eq!(errors[28].message, "right shift by -1 is not possible");
    assert_eq!(errors[29].message, "power by -1 is not possible");
    assert_eq!(errors[30].message, "right shift by 14676683207225698178084221555689649093015162623576402558976 is not possible");

    assert_eq!(errors.len(), 31);

    assert_eq!(warnings.len(), 0);
}

#[test]
fn test_types() {
    let file = r#"
    contract test_contract {
        function test_types32(bytes input) public {
            // value 2147483648 does not fit into type int32.
            int32 add_ovf = 2147483647 + 1;

            // value 2147483648 does not fit into type int32.
            int32 add_normal = 2147483647 + 0;

            // value 2147483648 does not fit into type int32.
            int32 mixed = 2147483647 + 1 + input.readInt32LE(2);
        }

        function test_types64(bytes input) public {
            // value 9223372036854775808 does not fit into type int64.
            int64 add_ovf = 9223372036854775807 + 1;

            int64 add_normal = 9223372036854775807;

            // value 9223372036854775808 does not fit into type int64.
            int64 mixed = 9223372036854775807 + 1 + input.readInt64LE(2);

            // value 18446744073709551616 does not fit into type uint64.
            uint64 pow_ovf = 2**64;

            uint64 normal_pow = (2**64) - 1;
        }

        function test_types_128_256(bytes input) public {
            while (true) {
                // value 340282366920938463463374607431768211456 does not fit into type uint64.
                uint128 ovf = 2**128;
                uint128 normal = 2**128 - 1;
            }
            uint128[] arr;
            // negative value -1 does not fit into type uint32. Cannot implicitly convert signed literal to unsigned type.
            // value 340282366920938463463374607431768211456 does not fit into type uint128.
            uint128 access = arr[1 - 2] + 1 + (2**128);
            // value 3000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000 does not fit into type uint256.
            uint256 num = 3e255;

            uint256 num_2 = 115792089237316195423570985008687907853269984665640564039457584007913129639935 *
                    4;
        }

        function foo() public {
            uint16 x = 0;
            x += 450000;

            for (uint16 i = 0; i < (2**32); i += 65546) {}

            uint8 y = 0;
            y *= 120 + 250;
            y -= 500;
            y /= 300 + 200 - 200 + y;
        }
    }

        "#;
    let ns = parse(file);
    let errors = ns.diagnostics.errors();

    assert_eq!(
        errors[0].message,
        "value 2147483648 does not fit into type int32."
    );
    assert_eq!(
        errors[1].message,
        "value 2147483648 does not fit into type int32."
    );
    assert_eq!(
        errors[2].message,
        "value 9223372036854775808 does not fit into type int64."
    );
    assert_eq!(
        errors[3].message,
        "value 9223372036854775808 does not fit into type int64."
    );
    assert_eq!(
        errors[4].message,
        "value 18446744073709551616 does not fit into type uint64."
    );
    assert_eq!(
        errors[5].message,
        "value 340282366920938463463374607431768211456 does not fit into type uint128."
    );
    assert_eq!(errors[6].message, "negative value -1 does not fit into type uint32. Cannot implicitly convert signed literal to unsigned type.");
    assert_eq!(
        errors[7].message,
        "value 340282366920938463463374607431768211456 does not fit into type uint128."
    );

    assert_eq!(errors[8].message, "value 3000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000 does not fit into type uint256.");

    assert_eq!(errors[9].message, "value 463168356949264781694283940034751631413079938662562256157830336031652518559740 does not fit into type uint256.");
    assert_eq!(
        errors[10].message,
        "value 450000 does not fit into type uint16."
    );
    assert_eq!(
        errors[11].message,
        "value 65546 does not fit into type uint16."
    );
    assert_eq!(
        errors[12].message,
        "value 370 does not fit into type uint8."
    );
    assert_eq!(
        errors[13].message,
        "value 500 does not fit into type uint8."
    );
    assert_eq!(
        errors[14].message,
        "value 300 does not fit into type uint8."
    );
    assert_eq!(errors.len(), 15);
}

#[test]
fn try_catch_solana() {
    let file = r#"
@program_id("GTW14QhXXafodyHp6RTaoVKKUgG4G2YgVYg2dBfp6FK4")
contract aborting {
    function abort() public returns (int32, bool) {
        revert("bar");
    }
}

contract runner {
    function test() external pure {

        try aborting.abort() returns (int32 a, bool b) {
            // call succeeded; return values are in a and b
        }
        catch Error(string x) {
            if (x == "bar") {
                // "bar" reason code was provided through revert() or require()
            }
        }
        catch (bytes raw) {
            // if no error string could decoding, we end up here with the raw data
        }
    }
}
    "#;

    let mut cache = FileResolver::default();
    cache.set_file_contents("test.sol", file.to_string());

    let ns = parse_and_resolve(OsStr::new("test.sol"), &mut cache, Target::Solana);

    assert_eq!(ns.diagnostics.len(), 3);
    assert!(ns.diagnostics.contains_message("found contract 'runner'"));
    assert!(ns.diagnostics.contains_message("found contract 'aborting'"));
    assert!(ns.diagnostics.contains_message("The try-catch statement is not \
     supported on Solana. Please, go to \
     https://solang.readthedocs.io/en/latest/language/statements.html#try-catch-statement for more information"));
}

#[test]
fn solana_discriminator_type() {
    let src = r#"
    contract test {
    function foo() public pure returns (int32) {
        return -3;
    }

    function testA() public returns (uint32) {
        function () external returns (int32) fptr = this.foo;
        return foo.selector;
    }

    function testB() public returns (bytes4) {
        function () external returns (int32) fptr = this.foo;
        return foo.selector;
    }
}
    "#;

    let mut cache = FileResolver::default();
    cache.set_file_contents("test.sol", src.to_string());

    let ns = parse_and_resolve(OsStr::new("test.sol"), &mut cache, Target::Solana);

    assert_eq!(ns.diagnostics.len(), 5);
    assert!(ns.diagnostics.contains_message("found contract 'test'"));
    assert!(ns.diagnostics.contains_message(
        "function selector needs an integer of at least 64 bits to avoid being truncated"
    ));
    assert!(ns
        .diagnostics
        .contains_message("implicit conversion to uint32 from bytes8 not allowed"));

    assert!(ns
        .diagnostics
        .contains_message("function selector should only be casted to bytes8 or larger"));
    assert!(ns
        .diagnostics
        .contains_message("implicit conversion would truncate from bytes8 to bytes4"));
}

#[test]
fn dynamic_account_metas() {
    let src = r#"
    import 'solana';

contract creator {
    function create_child_with_meta(address child, address payer) public {
        AccountMeta[] metas = new AccountMeta[](2);

        metas[0] = AccountMeta({pubkey: child, is_signer: false, is_writable: false});
        metas[1] = AccountMeta({pubkey: payer, is_signer: true, is_writable: true});

        Child.new{accounts: metas}(payer);

        Child.say_hello{accounts: []}();
    }
}

@program_id("Chi1d5XD6nTAp2EyaNGqMxZzUjh6NvhXRxbGHP3D1RaT")
contract Child {
    @payer(payer)
    @space(511 + 7)
    constructor(address payer) {
        print("In child constructor");
    }

    function say_hello() pure public {
        print("Hello there");
    }
}
    "#;
    let mut cache = FileResolver::default();
    cache.set_file_contents("test.sol", src.to_string());

    let ns = parse_and_resolve(OsStr::new("test.sol"), &mut cache, Target::Solana);

    let errors = ns.diagnostics.errors();
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "dynamic array is not supported for the 'accounts' argument"
    );
}

#[test]
fn no_address_and_no_metas() {
    let src = r#"
    import 'solana';

contract creator {
    function create_child_with_meta(address child) public {
        Child.new();
        Child.say_hello();
    }
}

@program_id("Chi1d5XD6nTAp2EyaNGqMxZzUjh6NvhXRxbGHP3D1RaT")
contract Child {
    @payer(payer)
    @space(511 + 7)
    constructor() {
        print("In child constructor");
    }

    function say_hello() pure public {
        print("Hello there");
    }
}
    "#;
    let mut cache = FileResolver::default();
    cache.set_file_contents("test.sol", src.to_string());

    let ns = parse_and_resolve(OsStr::new("test.sol"), &mut cache, Target::Solana);

    let errors = ns.diagnostics.errors();
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "accounts are required for calling a contract. You can either provide the accounts with the {accounts: ...} call argument or change this function's visibility to external"
    );
}

#[test]
fn state_initializer_contract_call_requires_accounts() {
    let src = r#"
    contract C {
        uint public x = this.foo();

        function foo() external pure returns (uint) {
            return 1;
        }
    }
    "#;
    let mut cache = FileResolver::default();
    cache.set_file_contents("test.sol", src.to_string());

    let ns = parse_and_resolve(OsStr::new("test.sol"), &mut cache, Target::Solana);

    let errors = ns.diagnostics.errors();
    assert!(errors.is_empty());

    let contract = &ns.contracts[0];
    let foo_no = *contract
        .functions
        .iter()
        .find(|function_no| ns.functions[**function_no].name == "foo")
        .unwrap();

    let initializer = contract.variables[0].initializer.as_ref().unwrap();

    match initializer {
        Expression::ExternalFunctionCall {
            returns,
            function,
            args,
            call_args,
            ..
        } => {
            assert_eq!(returns, &vec![Type::Uint(256)]);
            assert!(args.is_empty());
            assert!(matches!(
                call_args.accounts,
                ExternalCallAccounts::AbsentArgument
            ));

            assert!(matches!(
                function.as_ref(),
                Expression::ExternalFunction { function_no, .. } if *function_no == foo_no
            ));
        }
        _ => panic!("unexpected initializer expression: {initializer:?}"),
    }
}

#[test]
fn get_import_map() {
    let mut cache = FileResolver::default();
    let map = OsString::from("@openzepellin");
    let example_sol_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .canonicalize()
        .unwrap();

    cache.add_import_map(map.clone(), example_sol_path.clone());

    let retrieved = cache.get_import_map(&map);
    assert_eq!(Some(&example_sol_path), retrieved);
}

#[test]
fn get_import_path() {
    let mut cache = FileResolver::default();
    let examples = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .canonicalize()
        .unwrap();

    let bad_path = PathBuf::from("/IDontExist.sol");

    cache.add_import_path(&examples);
    cache.add_import_path(&bad_path);

    let ns = parse_and_resolve(OsStr::new("example.sol"), &mut cache, Target::EVM);

    let file = ns.files.first();
    assert!(file.is_some());
    if let Some(file) = file {
        let import_path = cache.get_import_path(file.import_no.unwrap());
        assert_eq!(Some(&(None, examples.clone())), import_path);
    }

    let ns = parse_and_resolve(OsStr::new("incrementer.sol"), &mut cache, Target::EVM);
    let file = ns.files.first();
    assert!(file.is_some());
    if let Some(file) = file {
        let import_path = cache.get_import_path(file.import_no.unwrap());
        assert_eq!(Some(&(None, examples.clone())), import_path);
    }
}
