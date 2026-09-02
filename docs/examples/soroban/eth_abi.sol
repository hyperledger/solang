// SPDX-License-Identifier: Apache-2.0
// Mapping of https://github.com/stellar/soroban-examples/tree/main/eth_abi
pragma solidity ^0.8.20;

contract EthAbi {
    struct Input {
        bytes32 a;
        uint256 b;
        uint256 c;
    }

    struct Output {
        bytes32 a;
        uint256 r;
    }

    function exec(bytes memory input) public pure returns (bytes memory) {
        Input memory i = abi.decode(input, (Input));
        return abi.encode(Output(i.a, i.b + i.c));
    }

    function run(
        bytes32 a,
        uint256 b,
        uint256 c
    ) public pure returns (Output memory) {
        bytes memory input = abi.encode(Input(a, b, c));
        bytes memory output = exec(input);
        return abi.decode(output, (Output));
    }
}
