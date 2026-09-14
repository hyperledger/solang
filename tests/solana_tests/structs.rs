// SPDX-License-Identifier: Apache-2.0

use crate::borsh_encoding::BorshToken;
use crate::build_solidity;
use num_bigint::BigInt;

#[test]
fn struct_as_reference() {
    let mut vm = build_solidity(
        r#"
        contract caller {
    struct AB {
        uint64 a;
        uint64 b;
    }

    function try_ref(AB[] vec) public pure returns (AB[]) {
        AB ref = vec[1];
        // This is a reference to the array, not a copy.
        ref.a += 3;
        return vec;
    }
}
        "#,
    );

    let data_account = vm.initialize_data_account();
    vm.function("new")
        .accounts(vec![("dataAccount", data_account)])
        .call();
    let input = BorshToken::Array(vec![
        BorshToken::Tuple(vec![
            BorshToken::Uint {
                width: 64,
                value: BigInt::from(1u8),
            },
            BorshToken::Uint {
                width: 64,
                value: BigInt::from(2u8),
            },
        ]),
        BorshToken::Tuple(vec![
            BorshToken::Uint {
                width: 64,
                value: BigInt::from(1u8),
            },
            BorshToken::Uint {
                width: 64,
                value: BigInt::from(2u8),
            },
        ]),
    ]);

    let res = vm.function("try_ref").arguments(&[input]).call().unwrap();
    assert_eq!(
        res,
        BorshToken::Array(vec![
            BorshToken::Tuple(vec![
                BorshToken::Uint {
                    width: 64,
                    value: BigInt::from(1u8),
                },
                BorshToken::Uint {
                    width: 64,
                    value: BigInt::from(2u8),
                },
            ]),
            BorshToken::Tuple(vec![
                BorshToken::Uint {
                    width: 64,
                    value: BigInt::from(4u8),
                },
                BorshToken::Uint {
                    width: 64,
                    value: BigInt::from(2u8),
                },
            ]),
        ])
    );
}

#[test]
fn user_defined_type_in_struct() {
    let mut vm = build_solidity(
        r#"
        type C is address;
        struct S { C c; }
        contract T {
            S s;
            function set_c(C c) public {
                s.c = c;
            }
            function get_c() public view returns (C) {
                return s.c;
            }
        }
        "#,
    );

    let data_account = vm.initialize_data_account();
    vm.function("new")
        .accounts(vec![("dataAccount", data_account)])
        .call();

    let address = [0x42; 32];
    vm.function("set_c")
        .arguments(&[BorshToken::Address(address)])
        .accounts(vec![("dataAccount", data_account)])
        .call();

    let res = vm
        .function("get_c")
        .accounts(vec![("dataAccount", data_account)])
        .call()
        .unwrap();
    assert_eq!(res, BorshToken::Address(address));
}
