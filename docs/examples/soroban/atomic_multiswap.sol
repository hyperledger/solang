// SPDX-License-Identifier: Apache-2.0
// Mapping of https://github.com/stellar/soroban-examples/tree/main/atomic_multiswap
pragma solidity ^0.8.20;

contract atomic_multiswap {
    struct SwapSpec {
        address addr;
        int128 amount;
        int128 min_recv;
    }

    function multi_swap(
        address swap_contract,
        address token_a,
        address token_b,
        SwapSpec[] memory swaps_a,
        SwapSpec[] memory swaps_b
    ) public {
        bool[] memory used = new bool[](swaps_b.length);

        for (uint256 i = 0; i < swaps_a.length; i++) {
            SwapSpec memory acc_a = swaps_a[i];
            for (uint256 j = 0; j < swaps_b.length; j++) {
                if (used[j]) {
                    continue;
                }
                SwapSpec memory acc_b = swaps_b[j];
                if (
                    acc_a.amount >= acc_b.min_recv &&
                    acc_a.min_recv <= acc_b.amount
                ) {
                    bytes memory payload = abi.encode(
                        "swap",
                        acc_a.addr,
                        acc_b.addr,
                        token_a,
                        token_b,
                        acc_a.amount,
                        acc_a.min_recv,
                        acc_b.amount,
                        acc_b.min_recv
                    );
                    swap_contract.call(payload);
                    used[j] = true;
                    break;
                }
            }
        }
    }
}
