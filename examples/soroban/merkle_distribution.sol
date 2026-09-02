// SPDX-License-Identifier: Apache-2.0
// Mapping of https://github.com/stellar/soroban-examples/tree/main/merkle_distribution
pragma solidity ^0.8.20;

contract merkle_distribution {
    bytes32 instance rootHash;
    address instance tokenAddress;
    mapping(uint32 => bool) instance claimed;

    struct Receiver {
        uint32 index;
        address recipient;
        int128 amount;
    }

    constructor(
        bytes32 root_hash,
        address token,
        int128 funding_amount,
        address funding_source
    ) {
        rootHash = root_hash;
        tokenAddress = token;

        bytes payload = abi.encode(
            "transfer", funding_source, address(this), funding_amount
        );
        (bool ok, ) = token.call(payload);
        require(ok, "funding transfer failed");
    }

    function compute_root(bytes32 leaf, bytes32[] memory proof)
        public
        pure
        returns (bytes32)
    {
        bytes32 h = leaf;
        for (uint32 i = 0; i < proof.length; i++) {
            if (h < proof[i]) {
                h = sha256(bytes.concat(h, proof[i]));
            } else {
                h = sha256(bytes.concat(proof[i], h));
            }
        }
        return h;
    }

    function leaf_hash(uint32 index, address receiver, int128 amount)
        public
        pure
        returns (bytes32)
    {
        Receiver memory node = Receiver(index, receiver, amount);
        return sha256(to_xdr(node));
    }

    function claim(
        uint32 index,
        address receiver,
        int128 amount,
        bytes32[] memory proof
    ) public {
        require(!claimed[index], "AlreadyClaimed");

        bytes32 leaf = leaf_hash(index, receiver, amount);
        bytes32 root = compute_root(leaf, proof);
        require(root == rootHash, "InvalidProof");

        bytes payload = abi.encode(
            "transfer", address(this), receiver, amount
        );
        (bool ok, ) = tokenAddress.call(payload);
        require(ok, "payout transfer failed");

        claimed[index] = true;
    }
}
