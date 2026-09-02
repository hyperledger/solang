// SPDX-License-Identifier: Apache-2.0

use crate::SorobanEnv;
use soroban_sdk::{
    testutils::Address as _, Address, BytesN, FromVal, IntoVal, TryFromVal, Vec as SVec,
};

const HASHER_SRC: &str = r#"
contract merkle_hasher {
    struct Receiver {
        uint32 index;
        address recipient;
        int128 amount;
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
}
"#;

const TOKEN_SRC: &str = r#"
contract token {
    address public admin;
    mapping(address => int128) public balances;

    constructor(address _admin) {
        admin = _admin;
    }

    function mint(address to, int128 amount) public {
        admin.requireAuth();
        balances[to] = balances[to] + amount;
    }

    function transfer(address from, address to, int128 amount) public {
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

const MERKLE_SRC: &str = r#"
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
"#;

fn deploy_token(runtime: &mut SorobanEnv) -> (Address, Address) {
    let admin = Address::generate(&runtime.env);
    let token = runtime.deploy_contract_with_args(TOKEN_SRC, (admin.clone(),));
    (token, admin)
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

fn token_balance(runtime: &SorobanEnv, token: &Address, owner: &Address) -> i128 {
    let val = runtime.invoke_contract(token, "balance", vec![owner.clone().into_val(&runtime.env)]);
    i128::from_val(&runtime.env, &val)
}

fn deploy_merkle(
    runtime: &mut SorobanEnv,
    root: &BytesN<32>,
    token: &Address,
    funding_amount: i128,
    funding_source: &Address,
) -> Address {
    runtime.deploy_contract_with_args(
        MERKLE_SRC,
        (
            root.clone(),
            token.clone(),
            funding_amount,
            funding_source.clone(),
        ),
    )
}

fn leaf_hash(
    runtime: &SorobanEnv,
    hasher: &Address,
    index: u32,
    receiver: &Address,
    amount: i128,
) -> BytesN<32> {
    let val = runtime.invoke_contract(
        hasher,
        "leaf_hash",
        vec![
            index.into_val(&runtime.env),
            receiver.clone().into_val(&runtime.env),
            amount.into_val(&runtime.env),
        ],
    );
    BytesN::<32>::try_from_val(&runtime.env, &val).unwrap()
}

fn compute_root(
    runtime: &SorobanEnv,
    hasher: &Address,
    leaf: &BytesN<32>,
    proof: &[BytesN<32>],
) -> BytesN<32> {
    let mut proof_vec = SVec::<BytesN<32>>::new(&runtime.env);
    for item in proof {
        proof_vec.push_back(item.clone());
    }
    let val = runtime.invoke_contract(
        hasher,
        "compute_root",
        vec![
            leaf.clone().into_val(&runtime.env),
            proof_vec.into_val(&runtime.env),
        ],
    );
    BytesN::<32>::try_from_val(&runtime.env, &val).unwrap()
}

fn claim(
    runtime: &SorobanEnv,
    contract: &Address,
    index: u32,
    receiver: &Address,
    amount: i128,
    proof: &[BytesN<32>],
) {
    let mut proof_vec = SVec::<BytesN<32>>::new(&runtime.env);
    for item in proof {
        proof_vec.push_back(item.clone());
    }
    runtime.invoke_contract(
        contract,
        "claim",
        vec![
            index.into_val(&runtime.env),
            receiver.clone().into_val(&runtime.env),
            amount.into_val(&runtime.env),
            proof_vec.into_val(&runtime.env),
        ],
    );
}

#[test]
fn merkle_distribution_single_leaf_claim() {
    let mut runtime = SorobanEnv::new();
    runtime.env.mock_all_auths_allowing_non_root_auth();

    let (token, _) = deploy_token(&mut runtime);
    let funding_source = Address::generate(&runtime.env);
    let claimant = Address::generate(&runtime.env);
    let payout: i128 = 500;

    mint(&runtime, &token, &funding_source, payout);

    let hasher = runtime.deploy_contract(HASHER_SRC);

    let leaf = leaf_hash(&runtime, &hasher, 0, &claimant, payout);

    let contract = deploy_merkle(&mut runtime, &leaf, &token, payout, &funding_source);

    claim(&runtime, &contract, 0, &claimant, payout, &[]);

    assert_eq!(token_balance(&runtime, &token, &claimant), payout);
    assert_eq!(token_balance(&runtime, &token, &contract), 0);
}

#[test]
fn merkle_distribution_four_leaf_tree() {
    let mut runtime = SorobanEnv::new();
    runtime.env.mock_all_auths_allowing_non_root_auth();

    let (token, _) = deploy_token(&mut runtime);
    let funding_source = Address::generate(&runtime.env);
    let amounts: [i128; 4] = [100, 200, 300, 400];
    let total: i128 = amounts.iter().sum();

    let addrs: std::vec::Vec<Address> = (0..4).map(|_| Address::generate(&runtime.env)).collect();

    mint(&runtime, &token, &funding_source, total);

    let hasher = runtime.deploy_contract(HASHER_SRC);

    let leaves: std::vec::Vec<BytesN<32>> = (0..4u32)
        .map(|i| {
            leaf_hash(
                &runtime,
                &hasher,
                i,
                &addrs[i as usize],
                amounts[i as usize],
            )
        })
        .collect();

    let n01 = compute_root(&runtime, &hasher, &leaves[0], &[leaves[1].clone()]);
    let n23 = compute_root(&runtime, &hasher, &leaves[2], &[leaves[3].clone()]);
    let root = compute_root(&runtime, &hasher, &n01, std::slice::from_ref(&n23));

    let contract = deploy_merkle(&mut runtime, &root, &token, total, &funding_source);

    // Claim for index 0 — proof: [L1, N23].
    claim(
        &runtime,
        &contract,
        0,
        &addrs[0],
        amounts[0],
        &[leaves[1].clone(), n23.clone()],
    );
    assert_eq!(token_balance(&runtime, &token, &addrs[0]), amounts[0]);

    // Claim for index 3 — proof: [L2, N01].
    claim(
        &runtime,
        &contract,
        3,
        &addrs[3],
        amounts[3],
        &[leaves[2].clone(), n01.clone()],
    );
    assert_eq!(token_balance(&runtime, &token, &addrs[3]), amounts[3]);

    // Indices 1 and 2 are still unclaimed — contract retains their amounts.
    assert_eq!(
        token_balance(&runtime, &token, &contract),
        amounts[1] + amounts[2]
    );
}

#[test]
fn merkle_distribution_double_claim_rejected() {
    let mut runtime = SorobanEnv::new();
    runtime.env.mock_all_auths_allowing_non_root_auth();

    let (token, _) = deploy_token(&mut runtime);
    let funding_source = Address::generate(&runtime.env);
    let claimant = Address::generate(&runtime.env);
    let payout: i128 = 100;

    mint(&runtime, &token, &funding_source, payout);

    let hasher = runtime.deploy_contract(HASHER_SRC);
    let leaf = leaf_hash(&runtime, &hasher, 0, &claimant, payout);
    let contract = deploy_merkle(&mut runtime, &leaf, &token, payout, &funding_source);

    claim(&runtime, &contract, 0, &claimant, payout, &[]);
    assert_eq!(token_balance(&runtime, &token, &claimant), payout);

    let empty_proof = SVec::<BytesN<32>>::new(&runtime.env);
    let logs = runtime.invoke_contract_expect_error(
        &contract,
        "claim",
        vec![
            0u32.into_val(&runtime.env),
            claimant.clone().into_val(&runtime.env),
            payout.into_val(&runtime.env),
            empty_proof.into_val(&runtime.env),
        ],
    );
    assert!(
        logs.iter()
            .any(|e| e.contains("AlreadyClaimed") || e.contains("require")),
        "expected AlreadyClaimed error, got: {:?}",
        logs
    );
}

#[test]
fn merkle_distribution_invalid_proof_rejected() {
    let mut runtime = SorobanEnv::new();
    runtime.env.mock_all_auths_allowing_non_root_auth();

    let (token, _) = deploy_token(&mut runtime);
    let funding_source = Address::generate(&runtime.env);
    let claimant = Address::generate(&runtime.env);
    let payout: i128 = 300;

    mint(&runtime, &token, &funding_source, payout);

    let hasher = runtime.deploy_contract(HASHER_SRC);
    let leaf = leaf_hash(&runtime, &hasher, 0, &claimant, payout);
    let contract = deploy_merkle(&mut runtime, &leaf, &token, payout, &funding_source);

    let empty_proof = SVec::<BytesN<32>>::new(&runtime.env);
    let logs = runtime.invoke_contract_expect_error(
        &contract,
        "claim",
        vec![
            0u32.into_val(&runtime.env),
            claimant.clone().into_val(&runtime.env),
            (payout - 1).into_val(&runtime.env),
            empty_proof.into_val(&runtime.env),
        ],
    );
    assert!(
        logs.iter()
            .any(|e| e.contains("InvalidProof") || e.contains("require")),
        "expected InvalidProof error, got: {:?}",
        logs
    );
}

#[test]
#[should_panic(expected = "InvalidAction")]
fn merkle_distribution_underfunded_constructor_panics() {
    let mut runtime = SorobanEnv::new();
    runtime.env.mock_all_auths_allowing_non_root_auth();

    let (token, _) = deploy_token(&mut runtime);
    let funding_source = Address::generate(&runtime.env);

    mint(&runtime, &token, &funding_source, 100);

    let root = BytesN::<32>::from_array(&runtime.env, &[0u8; 32]);

    let _ = deploy_merkle(&mut runtime, &root, &token, 500, &funding_source);
}

#[test]
fn merkle_distribution_compute_root_is_order_independent() {
    let mut runtime = SorobanEnv::new();

    let a0 = Address::generate(&runtime.env);
    let a1 = Address::generate(&runtime.env);

    let hasher = runtime.deploy_contract(HASHER_SRC);

    let l0 = leaf_hash(&runtime, &hasher, 0, &a0, 100);
    let l1 = leaf_hash(&runtime, &hasher, 1, &a1, 200);

    let root_via_l0 = compute_root(&runtime, &hasher, &l0, std::slice::from_ref(&l1));
    let root_via_l1 = compute_root(&runtime, &hasher, &l1, std::slice::from_ref(&l0));

    assert_eq!(root_via_l0, root_via_l1);
}

fn setup_four_leaf(
    runtime: &mut SorobanEnv,
    funding_amount: i128,
) -> (Address, Address, Address, i128, std::vec::Vec<BytesN<32>>) {
    let (token, _) = deploy_token(runtime);
    let funding_source = Address::generate(&runtime.env);
    mint(runtime, &token, &funding_source, funding_amount);

    let hasher = runtime.deploy_contract(HASHER_SRC);

    let addrs: std::vec::Vec<Address> = (0..4).map(|_| Address::generate(&runtime.env)).collect();
    let amounts: [i128; 4] = [10, 20, 30, 100];

    let leaves: std::vec::Vec<BytesN<32>> = (0..4u32)
        .map(|i| leaf_hash(runtime, &hasher, i, &addrs[i as usize], amounts[i as usize]))
        .collect();

    let n01 = compute_root(runtime, &hasher, &leaves[0], &[leaves[1].clone()]);
    let n23 = compute_root(runtime, &hasher, &leaves[2], &[leaves[3].clone()]);
    let root = compute_root(runtime, &hasher, &n01, std::slice::from_ref(&n23));

    let contract = deploy_merkle(runtime, &root, &token, funding_amount, &funding_source);

    let proof = vec![leaves[2].clone(), n01];
    (token, contract, addrs[3].clone(), amounts[3], proof)
}

#[test]
fn test_valid_claim() {
    let mut runtime = SorobanEnv::new();
    runtime.env.mock_all_auths_allowing_non_root_auth();

    let funding_amount: i128 = 1000;
    let (token, contract, receiver, amount, proof) = setup_four_leaf(&mut runtime, funding_amount);

    claim(&runtime, &contract, 3, &receiver, amount, &proof);

    assert_eq!(token_balance(&runtime, &token, &receiver), amount);
    assert_eq!(
        token_balance(&runtime, &token, &contract),
        funding_amount - amount
    );
}

#[test]
#[should_panic(expected = "InvalidAction")]
fn test_double_claim() {
    let mut runtime = SorobanEnv::new();
    runtime.env.mock_all_auths_allowing_non_root_auth();

    let (_token, contract, receiver, amount, proof) = setup_four_leaf(&mut runtime, 1000);

    claim(&runtime, &contract, 3, &receiver, amount, &proof);
    claim(&runtime, &contract, 3, &receiver, amount, &proof);
}

#[test]
#[should_panic(expected = "InvalidAction")]
fn test_bad_claim() {
    let mut runtime = SorobanEnv::new();
    runtime.env.mock_all_auths_allowing_non_root_auth();

    let (_token, contract, receiver, _amount, proof) = setup_four_leaf(&mut runtime, 1000);

    claim(&runtime, &contract, 3, &receiver, 100_000, &proof);
}
