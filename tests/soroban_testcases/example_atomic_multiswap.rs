// SPDX-License-Identifier: Apache-2.0

use crate::SorobanEnv;
use soroban_sdk::{contracttype, testutils::Address as _, Address, FromVal, IntoVal, Vec};

const TOKEN_SRC: &str = r#"
contract token {
    address public admin;
    mapping(address => int128) public balances;

    constructor(address _admin) {
        admin = _admin;
    }

    function mint(address to, int128 amount) public {
        require(amount >= 0, "Amount must be non-negative");
        admin.requireAuth();
        balances[to] = balances[to] + amount;
    }

    function transfer(address from, address to, int128 amount) public {
        require(amount >= 0, "Amount must be non-negative");
        from.requireAuth();
        require(balances[from] >= amount, "Insufficient balance");
        balances[from] = balances[from] - amount;
        balances[to] = balances[to] + amount;
    }

    function balance(address addr) public view returns (int128) {
        return balances[addr];
    }
}
"#;

const ATOMIC_SWAP_SRC: &str = r#"
contract atomic_swap {
    function swap(
        address a,
        address b,
        address token_a,
        address token_b,
        int128 amount_a,
        int128 min_b_for_a,
        int128 amount_b,
        int128 min_a_for_b
    ) public {
        require(amount_b >= min_b_for_a, "not enough token B for token A");
        require(amount_a >= min_a_for_b, "not enough token A for token B");

        a.requireAuth();
        b.requireAuth();

        move_token(token_a, a, b, amount_a, min_a_for_b);
        move_token(token_b, b, a, amount_b, min_b_for_a);
    }

    function move_token(
        address token,
        address from,
        address to,
        int128 max_spend_amount,
        int128 transfer_amount
    ) internal {
        address contract_address = address(this);

        bytes payload = abi.encode("transfer", from, contract_address, max_spend_amount);
        (bool success, bytes returndata) = token.call(payload);

        payload = abi.encode("transfer", contract_address, to, transfer_amount);
        (success, returndata) = token.call(payload);

        payload = abi.encode(
            "transfer",
            contract_address,
            from,
            max_spend_amount - transfer_amount
        );
        (success, returndata) = token.call(payload);
    }
}
"#;

const ATOMIC_MULTISWAP_SRC: &str = r#"
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
                if (acc_a.amount >= acc_b.min_recv && acc_a.min_recv <= acc_b.amount) {
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
"#;

#[contracttype]
#[derive(Clone)]
struct SwapSpec {
    addr: Address,
    amount: i128,
    min_recv: i128,
}

fn deploy_token(runtime: &mut SorobanEnv) -> Address {
    let admin = Address::generate(&runtime.env);
    runtime.deploy_contract_with_args(TOKEN_SRC, (admin,))
}

fn mint(runtime: &SorobanEnv, token: &Address, to: &Address, amount: i128) {
    runtime.invoke_contract(
        token,
        "mint",
        vec![
            to.clone().into_val(&runtime.env),
            amount.into_val(&runtime.env),
        ],
    );
}

fn balance(runtime: &SorobanEnv, token: &Address, owner: &Address) -> i128 {
    let val = runtime.invoke_contract(token, "balance", vec![owner.clone().into_val(&runtime.env)]);
    i128::from_val(&runtime.env, &val)
}

#[test]
fn atomic_multiswap_matches_and_clears_swaps() {
    let mut runtime = SorobanEnv::new();

    let token_a = deploy_token(&mut runtime);
    let token_b = deploy_token(&mut runtime);
    let swap = runtime.deploy_contract(ATOMIC_SWAP_SRC);
    let multiswap = runtime.deploy_contract(ATOMIC_MULTISWAP_SRC);

    runtime.env.mock_all_auths_allowing_non_root_auth();

    let a0 = Address::generate(&runtime.env);
    let a1 = Address::generate(&runtime.env);
    let a2 = Address::generate(&runtime.env);
    let b0 = Address::generate(&runtime.env);
    let b1 = Address::generate(&runtime.env);
    let b2 = Address::generate(&runtime.env);

    mint(&runtime, &token_a, &a0, 2000);
    mint(&runtime, &token_a, &a1, 3000);
    mint(&runtime, &token_a, &a2, 4000);
    mint(&runtime, &token_b, &b0, 300);
    mint(&runtime, &token_b, &b1, 295);
    mint(&runtime, &token_b, &b2, 400);

    let swaps_a = Vec::from_array(
        &runtime.env,
        [
            SwapSpec {
                addr: a0.clone(),
                amount: 2000,
                min_recv: 290,
            },
            SwapSpec {
                addr: a1.clone(),
                amount: 3000,
                min_recv: 350,
            },
            SwapSpec {
                addr: a2.clone(),
                amount: 4000,
                min_recv: 301,
            },
        ],
    );
    let swaps_b = Vec::from_array(
        &runtime.env,
        [
            SwapSpec {
                addr: b0.clone(),
                amount: 300,
                min_recv: 2100,
            },
            SwapSpec {
                addr: b1.clone(),
                amount: 295,
                min_recv: 1950,
            },
            SwapSpec {
                addr: b2.clone(),
                amount: 400,
                min_recv: 2900,
            },
        ],
    );

    runtime.invoke_contract(
        &multiswap,
        "multi_swap",
        vec![
            swap.clone().into_val(&runtime.env),
            token_a.clone().into_val(&runtime.env),
            token_b.clone().into_val(&runtime.env),
            swaps_a.into_val(&runtime.env),
            swaps_b.into_val(&runtime.env),
        ],
    );

    // Matches: a0<->b1 and a1<->b2. a2 and b0 stay untouched.
    assert_eq!(balance(&runtime, &token_a, &a0), 50);
    assert_eq!(balance(&runtime, &token_a, &a1), 100);
    assert_eq!(balance(&runtime, &token_a, &a2), 4000);
    assert_eq!(balance(&runtime, &token_a, &b0), 0);
    assert_eq!(balance(&runtime, &token_a, &b1), 1950);
    assert_eq!(balance(&runtime, &token_a, &b2), 2900);

    assert_eq!(balance(&runtime, &token_b, &a0), 290);
    assert_eq!(balance(&runtime, &token_b, &a1), 350);
    assert_eq!(balance(&runtime, &token_b, &a2), 0);
    assert_eq!(balance(&runtime, &token_b, &b0), 300);
    assert_eq!(balance(&runtime, &token_b, &b1), 5);
    assert_eq!(balance(&runtime, &token_b, &b2), 50);

    // The swap contract nets to zero on both tokens.
    assert_eq!(balance(&runtime, &token_a, &swap), 0);
    assert_eq!(balance(&runtime, &token_b, &swap), 0);
}

#[test]
fn atomic_multiswap_with_duplicate_account() {
    let mut runtime = SorobanEnv::new();

    let token_a = deploy_token(&mut runtime);
    let token_b = deploy_token(&mut runtime);
    let swap = runtime.deploy_contract(ATOMIC_SWAP_SRC);
    let multiswap = runtime.deploy_contract(ATOMIC_MULTISWAP_SRC);

    runtime.env.mock_all_auths_allowing_non_root_auth();

    let address_a = Address::generate(&runtime.env);
    let address_b = Address::generate(&runtime.env);

    mint(&runtime, &token_a, &address_a, 3000);
    mint(&runtime, &token_b, &address_b, 291);

    let swaps_a = Vec::from_array(
        &runtime.env,
        [
            SwapSpec {
                addr: address_a.clone(),
                amount: 1000,
                min_recv: 100,
            },
            SwapSpec {
                addr: address_a.clone(),
                amount: 2000,
                min_recv: 190,
            },
        ],
    );
    let swaps_b = Vec::from_array(
        &runtime.env,
        [
            SwapSpec {
                addr: address_b.clone(),
                amount: 101,
                min_recv: 1000,
            },
            SwapSpec {
                addr: address_b.clone(),
                amount: 190,
                min_recv: 2000,
            },
        ],
    );

    runtime.invoke_contract(
        &multiswap,
        "multi_swap",
        vec![
            swap.clone().into_val(&runtime.env),
            token_a.clone().into_val(&runtime.env),
            token_b.clone().into_val(&runtime.env),
            swaps_a.into_val(&runtime.env),
            swaps_b.into_val(&runtime.env),
        ],
    );

    // The same address participates in two swaps: a0<->b0 and a1<->b1.
    assert_eq!(balance(&runtime, &token_a, &address_a), 0);
    assert_eq!(balance(&runtime, &token_a, &address_b), 3000);
    assert_eq!(balance(&runtime, &token_b, &address_a), 290);
    assert_eq!(balance(&runtime, &token_b, &address_b), 1);
}
