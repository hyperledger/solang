// SPDX-License-Identifier: Apache-2.0

use crate::SorobanEnv;
use soroban_sdk::{contracttype, testutils::Address as _, Address, FromVal, IntoVal};

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

const SINGLE_OFFER_SRC: &str = r#"
contract single_offer {
    struct Offer {
        address seller;
        address sell_token;
        address buy_token;
        uint32 sell_price;
        uint32 buy_price;
    }

    Offer instance offer;
    bool instance created = false;

    function create(
        address seller,
        address sell_token,
        address buy_token,
        uint32 sell_price,
        uint32 buy_price
    ) public {
        require(!created, "offer is already created");
        require(buy_price != 0 && sell_price != 0, "zero price is not allowed");
        seller.requireAuth();
        offer = Offer({
            seller: seller,
            sell_token: sell_token,
            buy_token: buy_token,
            sell_price: sell_price,
            buy_price: buy_price
        });
        created = true;
    }

    function trade(
        address buyer,
        int128 buy_token_amount,
        int128 min_sell_token_amount
    ) public {
        buyer.requireAuth();
        Offer memory o = offer;
        int128 sell_token_amount = (buy_token_amount * int128(o.sell_price)) / int128(o.buy_price);
        require(sell_token_amount >= min_sell_token_amount, "price is too low");
        address contract_address = address(this);
        token_transfer(o.buy_token, buyer, contract_address, buy_token_amount);
        token_transfer(o.sell_token, contract_address, buyer, sell_token_amount);
        token_transfer(o.buy_token, contract_address, o.seller, buy_token_amount);
    }

    function withdraw(address token, int128 amount) public {
        Offer memory o = offer;
        o.seller.requireAuth();
        token_transfer(token, address(this), o.seller, amount);
    }

    function updt_price(uint32 sell_price, uint32 buy_price) public {
        require(buy_price != 0 && sell_price != 0, "zero price is not allowed");
        offer.seller.requireAuth();
        offer.sell_price = sell_price;
        offer.buy_price = buy_price;
    }

    function get_offer() public view returns (Offer memory) {
        return offer;
    }

    function token_transfer(address token, address from, address to, int128 amount) internal {
        bytes memory payload = abi.encode("transfer", from, to, amount);
        (bool success, bytes memory returndata) = token.call(payload);
    }
}
"#;

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Offer {
    pub seller: Address,
    pub sell_token: Address,
    pub buy_token: Address,
    pub sell_price: u32,
    pub buy_price: u32,
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

fn transfer(runtime: &SorobanEnv, token: &Address, from: &Address, to: &Address, amount: i128) {
    runtime.invoke_contract(
        token,
        "transfer",
        vec![
            from.clone().into_val(&runtime.env),
            to.clone().into_val(&runtime.env),
            amount.into_val(&runtime.env),
        ],
    );
}

fn balance(runtime: &SorobanEnv, token: &Address, owner: &Address) -> i128 {
    let val = runtime.invoke_contract(token, "balance", vec![owner.clone().into_val(&runtime.env)]);
    i128::from_val(&runtime.env, &val)
}

fn assert_balance(runtime: &SorobanEnv, token: &Address, owner: &Address, expected: i128) {
    assert_eq!(balance(runtime, token, owner), expected);
}

#[test]
fn example_single_offer_trade() {
    let mut runtime = SorobanEnv::new();

    let sell_token = deploy_token(&mut runtime);
    let buy_token = deploy_token(&mut runtime);
    let offer_addr = runtime.deploy_contract(SINGLE_OFFER_SRC);

    runtime.env.mock_all_auths();

    let seller = Address::generate(&runtime.env);
    let buyer = Address::generate(&runtime.env);

    mint(&runtime, &sell_token, &seller, 1000);
    mint(&runtime, &buy_token, &buyer, 1000);

    runtime.invoke_contract(
        &offer_addr,
        "create",
        vec![
            seller.clone().into_val(&runtime.env),
            sell_token.clone().into_val(&runtime.env),
            buy_token.clone().into_val(&runtime.env),
            1_u32.into_val(&runtime.env),
            2_u32.into_val(&runtime.env),
        ],
    );

    transfer(&runtime, &sell_token, &seller, &offer_addr, 100);

    runtime.invoke_contract(
        &offer_addr,
        "trade",
        vec![
            buyer.clone().into_val(&runtime.env),
            20_i128.into_val(&runtime.env),
            10_i128.into_val(&runtime.env),
        ],
    );

    assert_balance(&runtime, &sell_token, &seller, 900);
    assert_balance(&runtime, &sell_token, &buyer, 10);
    assert_balance(&runtime, &sell_token, &offer_addr, 90);
    assert_balance(&runtime, &buy_token, &seller, 20);
    assert_balance(&runtime, &buy_token, &buyer, 980);
    assert_balance(&runtime, &buy_token, &offer_addr, 0);
}

#[test]
fn example_single_offer_rejects_price_floor() {
    let mut runtime = SorobanEnv::new();

    let sell_token = deploy_token(&mut runtime);
    let buy_token = deploy_token(&mut runtime);
    let offer_addr = runtime.deploy_contract(SINGLE_OFFER_SRC);

    runtime.env.mock_all_auths();

    let seller = Address::generate(&runtime.env);
    let buyer = Address::generate(&runtime.env);

    mint(&runtime, &sell_token, &seller, 1000);
    mint(&runtime, &buy_token, &buyer, 1000);

    runtime.invoke_contract(
        &offer_addr,
        "create",
        vec![
            seller.clone().into_val(&runtime.env),
            sell_token.clone().into_val(&runtime.env),
            buy_token.clone().into_val(&runtime.env),
            1_u32.into_val(&runtime.env),
            2_u32.into_val(&runtime.env),
        ],
    );
    transfer(&runtime, &sell_token, &seller, &offer_addr, 100);

    // Buyer demands 11 sell_token but price only yields 10 — should fail
    let logs = runtime.invoke_contract_expect_error(
        &offer_addr,
        "trade",
        vec![
            buyer.clone().into_val(&runtime.env),
            20_i128.into_val(&runtime.env),
            11_i128.into_val(&runtime.env),
        ],
    );
    assert!(logs.iter().any(|e| e.contains("require condition failed")));

    // Balances must be unchanged
    assert_balance(&runtime, &sell_token, &buyer, 0);
    assert_balance(&runtime, &buy_token, &buyer, 1000);
}

#[test]
fn example_single_offer_withdraw() {
    let mut runtime = SorobanEnv::new();

    let sell_token = deploy_token(&mut runtime);
    let buy_token = deploy_token(&mut runtime);
    let offer_addr = runtime.deploy_contract(SINGLE_OFFER_SRC);

    runtime.env.mock_all_auths();

    let seller = Address::generate(&runtime.env);
    let buyer = Address::generate(&runtime.env);

    mint(&runtime, &sell_token, &seller, 1000);
    mint(&runtime, &buy_token, &buyer, 1000);

    runtime.invoke_contract(
        &offer_addr,
        "create",
        vec![
            seller.clone().into_val(&runtime.env),
            sell_token.clone().into_val(&runtime.env),
            buy_token.clone().into_val(&runtime.env),
            1_u32.into_val(&runtime.env),
            2_u32.into_val(&runtime.env),
        ],
    );
    transfer(&runtime, &sell_token, &seller, &offer_addr, 100);

    runtime.invoke_contract(
        &offer_addr,
        "trade",
        vec![
            buyer.clone().into_val(&runtime.env),
            20_i128.into_val(&runtime.env),
            10_i128.into_val(&runtime.env),
        ],
    );

    runtime.invoke_contract(
        &offer_addr,
        "withdraw",
        vec![
            sell_token.clone().into_val(&runtime.env),
            70_i128.into_val(&runtime.env),
        ],
    );

    assert_balance(&runtime, &sell_token, &seller, 970);
    assert_balance(&runtime, &sell_token, &offer_addr, 20);
}

#[test]
#[should_panic]
fn example_single_offer_create_zero_price_traps() {
    let mut runtime = SorobanEnv::new();

    let sell_token = deploy_token(&mut runtime);
    let buy_token = deploy_token(&mut runtime);
    let offer_addr = runtime.deploy_contract(SINGLE_OFFER_SRC);

    runtime.env.mock_all_auths();

    let seller = Address::generate(&runtime.env);

    // sell_price = 0 violates require(buy_price != 0 && sell_price != 0) and traps.
    runtime.invoke_contract(
        &offer_addr,
        "create",
        vec![
            seller.clone().into_val(&runtime.env),
            sell_token.clone().into_val(&runtime.env),
            buy_token.clone().into_val(&runtime.env),
            0_u32.into_val(&runtime.env),
            2_u32.into_val(&runtime.env),
        ],
    );
}

#[test]
fn example_single_offer_update_price() {
    let mut runtime = SorobanEnv::new();

    let sell_token = deploy_token(&mut runtime);
    let buy_token = deploy_token(&mut runtime);
    let offer_addr = runtime.deploy_contract(SINGLE_OFFER_SRC);

    runtime.env.mock_all_auths();

    let seller = Address::generate(&runtime.env);
    let buyer = Address::generate(&runtime.env);

    mint(&runtime, &sell_token, &seller, 1000);
    mint(&runtime, &buy_token, &buyer, 1000);

    runtime.invoke_contract(
        &offer_addr,
        "create",
        vec![
            seller.clone().into_val(&runtime.env),
            sell_token.clone().into_val(&runtime.env),
            buy_token.clone().into_val(&runtime.env),
            1_u32.into_val(&runtime.env),
            2_u32.into_val(&runtime.env),
        ],
    );
    transfer(&runtime, &sell_token, &seller, &offer_addr, 100);

    runtime.invoke_contract(
        &offer_addr,
        "trade",
        vec![
            buyer.clone().into_val(&runtime.env),
            20_i128.into_val(&runtime.env),
            10_i128.into_val(&runtime.env),
        ],
    );

    runtime.invoke_contract(
        &offer_addr,
        "withdraw",
        vec![
            sell_token.clone().into_val(&runtime.env),
            70_i128.into_val(&runtime.env),
        ],
    );

    runtime.invoke_contract(
        &offer_addr,
        "updt_price",
        vec![1_u32.into_val(&runtime.env), 1_u32.into_val(&runtime.env)],
    );

    let raw = runtime.invoke_contract(&offer_addr, "get_offer", vec![]);
    let o = Offer::from_val(&runtime.env, &raw);
    assert_eq!(o.sell_price, 1);
    assert_eq!(o.buy_price, 1);

    runtime.invoke_contract(
        &offer_addr,
        "trade",
        vec![
            buyer.clone().into_val(&runtime.env),
            10_i128.into_val(&runtime.env),
            9_i128.into_val(&runtime.env),
        ],
    );

    assert_balance(&runtime, &sell_token, &seller, 970);
    assert_balance(&runtime, &sell_token, &buyer, 20);
    assert_balance(&runtime, &sell_token, &offer_addr, 10);
    assert_balance(&runtime, &buy_token, &seller, 30);
    assert_balance(&runtime, &buy_token, &buyer, 970);
    assert_balance(&runtime, &buy_token, &offer_addr, 0);
}
