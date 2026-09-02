// SPDX-License-Identifier: Apache-2.0

use crate::build_solidity;
use soroban_sdk::{contracttype, Bytes, FromVal, IntoVal, U256};

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Output {
    pub a: Bytes,
    pub r: U256,
}

const CONTRACT: &str = r#"
    contract EthAbi {
        struct Input  { bytes32 a; uint256 b; uint256 c; }
        struct Output { bytes32 a; uint256 r; }

        function exec(bytes memory input) public pure returns (bytes memory) {
            Input memory i = abi.decode(input, (Input));
            return abi.encode(Output(i.a, i.b + i.c));
        }

        function run(bytes32 a, uint256 b, uint256 c) public pure returns (Output memory) {
            bytes memory input = abi.encode(Input(a, b, c));
            bytes memory output = exec(input);
            return abi.decode(output, (Output));
        }
    }
"#;

#[test]
fn example_eth_abi_run_adds_and_preserves() {
    let runtime = build_solidity(CONTRACT, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let bytes: [u8; 32] = core::array::from_fn(|i| (i as u8).wrapping_mul(7));
    let a = Bytes::from_array(env, &bytes);
    let b: u128 = 100_000_000_000_000_000_000;
    let c: u128 = 50_000_000_000_000_000_000;

    let o = Output::from_val(
        env,
        &runtime.invoke_contract(
            addr,
            "run",
            vec![
                a.clone().into_val(env),
                U256::from_u128(env, b).into_val(env),
                U256::from_u128(env, c).into_val(env),
            ],
        ),
    );
    assert_eq!(o.a, a);
    assert_eq!(o.r, U256::from_u128(env, b + c));
}

#[test]
fn example_eth_abi_run_large_uint256() {
    let runtime = build_solidity(CONTRACT, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let a = Bytes::from_array(env, &[0xABu8; 32]);
    let b: u128 = 1_000_000_000_000_000_000_000_000_000_000;
    let c: u128 = 2_000_000_000_000_000_000_000_000_000_000;

    let o = Output::from_val(
        env,
        &runtime.invoke_contract(
            addr,
            "run",
            vec![
                a.clone().into_val(env),
                U256::from_u128(env, b).into_val(env),
                U256::from_u128(env, c).into_val(env),
            ],
        ),
    );
    assert_eq!(o.a, a);
    assert_eq!(o.r, U256::from_u128(env, b + c));
}

#[test]
fn example_eth_abi_run_zero() {
    let runtime = build_solidity(CONTRACT, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    let a = Bytes::from_array(env, &[0u8; 32]);
    let o = Output::from_val(
        env,
        &runtime.invoke_contract(
            addr,
            "run",
            vec![
                a.clone().into_val(env),
                U256::from_u128(env, 0).into_val(env),
                U256::from_u128(env, 0).into_val(env),
            ],
        ),
    );
    assert_eq!(o.a, a);
    assert_eq!(o.r, U256::from_u128(env, 0));
}
