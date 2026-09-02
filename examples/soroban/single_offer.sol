// SPDX-License-Identifier: Apache-2.0
// Mapping of https://github.com/stellar/soroban-examples/tree/main/single_offer
pragma solidity ^0.8.20;

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
