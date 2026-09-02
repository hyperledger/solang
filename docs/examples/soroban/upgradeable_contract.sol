// SPDX-License-Identifier: Apache-2.0
// Mapping of https://github.com/stellar/soroban-examples/tree/main/upgradeable_contract
pragma solidity ^0.8.20;

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
