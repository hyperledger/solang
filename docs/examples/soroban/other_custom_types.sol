// SPDX-License-Identifier: Apache-2.0
// Mapping of https://github.com/stellar/soroban-examples/tree/main/other_custom_types
pragma solidity ^0.8.20;

contract other_custom_types {
    struct Test {
        uint32 a;
        bool b;
        string c;
    }

    enum SimpleEnum {
        First,
        Second,
        Third
    }

    enum RoyalCard {
        Jack,
        Queen,
        King
    }

    struct TupleStruct {
        Test test;
        SimpleEnum simple;
    }

    struct ComplexStruct {
        address admin;
        uint64 a64;
        uint32[] assets_vec;
        SimpleEnum base_asset;
        uint32 a32;
        uint32 b32;
        uint32 c32;
        RoyalCard complex_enum3;
    }

    event AuthEvent(address indexed hello, string world);

    uint32 persistent count;

    function hello(string memory v) public pure returns (string memory) {
        return v;
    }

    function auth(address addr, string memory world) public returns (address) {
        addr.requireAuth();
        emit AuthEvent(addr, world);
        return addr;
    }

    function get_count() public view returns (uint32) {
        return count;
    }

    function inc() public returns (uint32) {
        count += 1;
        return count;
    }

    function woid() public pure {}

    function u32_fail_on_even(uint32 v) public pure returns (uint32) {
        require(v % 2 == 1, "NumberMustBeOdd");
        return v;
    }

    function u32_(uint32 v) public pure returns (uint32) {
        return v;
    }

    function i32_(int32 v) public pure returns (int32) {
        return v;
    }

    function i64_(int64 v) public pure returns (int64) {
        return v;
    }

    function strukt_hel(Test memory t) public pure returns (string[] memory) {
        string[] memory res = new string[](2);
        res[0] = "Hello";
        res[1] = t.c;
        return res;
    }

    function strukt(Test memory t) public pure returns (Test memory) {
        return t;
    }

    function simple(SimpleEnum v) public pure returns (SimpleEnum) {
        return v;
    }

    function addresse(address v) public pure returns (address) {
        return v;
    }

    function bytes_(bytes memory v) public pure returns (bytes memory) {
        return v;
    }

    function bytes_n(bytes9 v) public pure returns (bytes9) {
        return v;
    }

    function card(RoyalCard v) public pure returns (RoyalCard) {
        return v;
    }

    function boolean(bool v) public pure returns (bool) {
        return v;
    }

    function not(bool v) public pure returns (bool) {
        return !v;
    }

    function i128(int128 v) public pure returns (int128) {
        return v;
    }

    function u128(uint128 v) public pure returns (uint128) {
        return v;
    }

    function multi_args(uint32 a, bool b) public pure returns (uint32) {
        return b ? a : 0;
    }

    function vec(uint32[] memory v) public pure returns (uint32[] memory) {
        return v;
    }

    function u256(uint256 v) public pure returns (uint256) {
        return v;
    }

    function i256(int256 v) public pure returns (int256) {
        return v;
    }

    function string_(string memory v) public pure returns (string memory) {
        return v;
    }

    function tuple_strukt(TupleStruct memory t) public pure returns (TupleStruct memory) {
        return t;
    }

    function complex_struct(ComplexStruct memory config) public pure returns (ComplexStruct memory) {
        return config;
    }
}
