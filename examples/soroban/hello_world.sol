// SPDX-License-Identifier: Apache-2.0
// Mapping of https://github.com/stellar/soroban-examples/tree/main/hello_world
pragma solidity ^0.8.20;

contract hello_world {
    // Mirrors the upstream Soroban `hello` function, which takes a name and
    // returns the vector ["Hello", <name>].
    function hello(string memory to) public pure returns (string[] memory) {
        string[] memory result = new string[](2);
        result[0] = "Hello";
        result[1] = to;
        return result;
    }
}
