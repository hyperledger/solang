// SPDX-License-Identifier: Apache-2.0

use crate::build_solidity;
use soroban_sdk::{contracttype, IntoVal, TryFromVal, Val, Vec as SVec};

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub v: i32,
}

#[test]
fn nested_u64_roundtrip_and_sum() {
    let runtime = build_solidity(
        r#"
        contract c {
            function echo(uint64[][] memory a) public pure returns (uint64[][] memory) {
                return a;
            }

            function total(uint64[][] memory a) public pure returns (uint64) {
                uint64 s = 0;
                for (uint32 i = 0; i < a.length; i++) {
                    for (uint32 j = 0; j < a[i].length; j++) {
                        s += a[i][j];
                    }
                }
                return s;
            }

            function build() public pure returns (uint64[][] memory) {
                uint64[][] memory out = new uint64[][](3);
                out[0] = new uint64[](1);
                out[0][0] = 7;
                out[1] = new uint64[](0);
                out[2] = new uint64[](2);
                out[2][0] = 8;
                out[2][1] = 9;
                return out;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let input: SVec<SVec<u64>> = soroban_sdk::vec![
        env,
        soroban_sdk::vec![env, 1_u64, 2, 3],
        soroban_sdk::vec![env],
        soroban_sdk::vec![env, 10_u64, 20],
    ];

    let res = runtime.invoke_contract(addr, "echo", vec![input.clone().into_val(env)]);
    assert_eq!(SVec::<SVec<u64>>::try_from_val(env, &res).unwrap(), input);

    let res = runtime.invoke_contract(addr, "total", vec![input.into_val(env)]);
    let expected: soroban_sdk::Val = 36_u64.into_val(env);
    assert!(expected.shallow_eq(&res));

    let res = runtime.invoke_contract(addr, "build", vec![]);
    let got = SVec::<SVec<u64>>::try_from_val(env, &res).unwrap();
    assert_eq!(
        got,
        soroban_sdk::vec![
            env,
            soroban_sdk::vec![env, 7_u64],
            soroban_sdk::vec![env],
            soroban_sdk::vec![env, 8_u64, 9],
        ]
    );
}

#[test]
fn nested_i32_roundtrip() {
    let runtime = build_solidity(
        r#"
        contract c {
            function echo(int32[][] memory a) public pure returns (int32[][] memory) {
                return a;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let input: SVec<SVec<i32>> = soroban_sdk::vec![
        env,
        soroban_sdk::vec![env, -1_i32, 0, 1],
        soroban_sdk::vec![env, i32::MIN, i32::MAX],
    ];
    let res = runtime.invoke_contract(addr, "echo", vec![input.clone().into_val(env)]);
    assert_eq!(SVec::<SVec<i32>>::try_from_val(env, &res).unwrap(), input);
}

#[test]
fn storage_nested_u64_set_get() {
    let src = r#"
        contract c {
            uint64[][] stored;

            function set(uint64[][] memory a) public {
                stored = a;
            }

            function get() public view returns (uint64[][] memory) {
                return stored;
            }

            function total() public view returns (uint64) {
                uint64 s = 0;
                for (uint32 i = 0; i < stored.length; i++) {
                    for (uint32 j = 0; j < stored[i].length; j++) {
                        s += stored[i][j];
                    }
                }
                return s;
            }

            // overwrite the whole nested storage array with a freshly built one
            function reset() public returns (uint64[][] memory) {
                uint64[][] memory tmp = new uint64[][](2);
                tmp[0] = new uint64[](2);
                tmp[0][0] = 100;
                tmp[0][1] = 200;
                tmp[1] = new uint64[](0);
                stored = tmp;
                return stored;
            }
        }
    "#;

    let runtime = build_solidity(src, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let input: SVec<SVec<u64>> = soroban_sdk::vec![
        env,
        soroban_sdk::vec![env, 1_u64, 2, 3],
        soroban_sdk::vec![env],
        soroban_sdk::vec![env, 10_u64, 20],
    ];

    runtime.invoke_contract(addr, "set", vec![input.clone().into_val(env)]);

    let res = runtime.invoke_contract(addr, "get", vec![]);
    assert_eq!(SVec::<SVec<u64>>::try_from_val(env, &res).unwrap(), input);

    let expected: Val = 36_u64.into_val(env);
    let res = runtime.invoke_contract(addr, "total", vec![]);
    assert!(expected.shallow_eq(&res));

    let res = runtime.invoke_contract(addr, "reset", vec![]);
    let expected: SVec<SVec<u64>> = soroban_sdk::vec![
        env,
        soroban_sdk::vec![env, 100_u64, 200],
        soroban_sdk::vec![env],
    ];
    assert_eq!(
        SVec::<SVec<u64>>::try_from_val(env, &res).unwrap(),
        expected
    );

    let res = runtime.invoke_contract(addr, "get", vec![]);
    assert_eq!(
        SVec::<SVec<u64>>::try_from_val(env, &res).unwrap(),
        expected
    );
}

#[test]
fn nested_struct_array_roundtrip() {
    let runtime = build_solidity(
        r#"
        contract c {
            struct Cell { int32 v; }
            function echo(Cell[][] memory a) public pure returns (Cell[][] memory) {
                return a;
            }
        }
        "#,
        |_| {},
    );

    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let input: SVec<SVec<Cell>> = soroban_sdk::vec![
        env,
        soroban_sdk::vec![env, Cell { v: -5 }, Cell { v: 6 }],
        soroban_sdk::vec![env],
        soroban_sdk::vec![env, Cell { v: 42 }],
    ];
    let res = runtime.invoke_contract(addr, "echo", vec![input.clone().into_val(env)]);
    assert_eq!(SVec::<SVec<Cell>>::try_from_val(env, &res).unwrap(), input);
}
