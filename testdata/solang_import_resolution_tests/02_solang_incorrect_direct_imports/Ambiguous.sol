// SPDX-License-Identifier: BUSL-1.1

pragma solidity ^0.8.0;


contract Ambiguous {

    function identity(uint256 x) pure external returns (uint256) {
      return x;
    }
}
