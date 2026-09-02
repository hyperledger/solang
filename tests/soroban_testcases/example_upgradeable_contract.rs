// SPDX-License-Identifier: Apache-2.0

use crate::{build_wasm, SorobanEnv};
use soroban_sdk::{testutils::Address as _, Address, Bytes, BytesN, IntoVal, Val};

const V1_SRC: &str = r#"
contract UpgradeableContract {
    address instance admin;

    constructor(address _admin) {
        admin = _admin;
    }

    function version() public pure returns (uint32) {
        return 1;
    }

    function upgrade(bytes32 new_wasm_hash) public {
        admin.requireAuth();
        updateCurrentContractWasm(new_wasm_hash);
    }
}
"#;

const V2_SRC: &str = r#"
contract UpgradeableContract {
    address instance admin;

    constructor(address _admin) {
        admin = _admin;
    }

    function version() public pure returns (uint32) {
        return 2;
    }

    function upgrade(bytes32 new_wasm_hash) public {
        admin.requireAuth();
        updateCurrentContractWasm(new_wasm_hash);
    }
}
"#;

fn deploy_v1() -> (SorobanEnv, Address, Address) {
    let mut runtime = SorobanEnv::new();
    runtime.env.mock_all_auths();
    let admin = Address::generate(&runtime.env);
    let addr = runtime.deploy_contract_with_args(V1_SRC, (admin.clone(),));
    (runtime, addr, admin)
}

#[test]
fn example_upgradeable_contract_initial_version_is_one() {
    let (runtime, addr, _admin) = deploy_v1();

    let ret = runtime.invoke_contract(&addr, "version", vec![]);
    let expected: Val = 1_u32.into_val(&runtime.env);
    assert!(expected.shallow_eq(&ret));
}

#[test]
fn example_upgradeable_contract_upgrade_switches_to_v2() {
    let (runtime, addr, _admin) = deploy_v1();

    let before = runtime.invoke_contract(&addr, "version", vec![]);
    let expected_1: Val = 1_u32.into_val(&runtime.env);
    assert!(expected_1.shallow_eq(&before));

    let v2_wasm = build_wasm(V2_SRC).0;
    let v2_bytes = Bytes::from_slice(&runtime.env, &v2_wasm);
    let new_hash: BytesN<32> = runtime.env.deployer().upload_contract_wasm(v2_bytes);

    runtime.invoke_contract(&addr, "upgrade", vec![new_hash.into_val(&runtime.env)]);

    let after = runtime.invoke_contract(&addr, "version", vec![]);
    let expected_2: Val = 2_u32.into_val(&runtime.env);
    assert!(expected_2.shallow_eq(&after));
}
