Soroban Examples Coverage
=========================

This page maps upstream examples from `stellar/soroban-examples <https://github.com/stellar/soroban-examples>`_ to documented Solang Solidity examples or coverage in this repository.

The table below only includes upstream examples for which this repository currently has a clear Solidity counterpart or nearest documented coverage example. Absence from this table does not prove that an upstream example is impossible in Solang; it means there is not yet a documented counterpart in this repository.

For the current feature-oriented support status, see :doc:`soroban_support_matrix`.

Documented Counterparts
+++++++++++++++++++++++

.. list-table::
   :header-rows: 1

   * - Upstream Rust example
     - Solang Solidity example or coverage
     - Notes
   * - `alloc <https://github.com/stellar/soroban-examples/tree/main/alloc>`_
     - `tests/soroban_testcases/alloc.rs <https://github.com/hyperledger-solang/solang/blob/main/tests/soroban_testcases/alloc.rs>`_
     - Covered by Solidity testcases for dynamic memory arrays, including vector allocation, ``push()``, iteration, and summation.
   * - `atomic_swap <https://github.com/stellar/soroban-examples/tree/main/atomic_swap>`_
     - `docs/examples/soroban/atomic_swap <https://github.com/hyperledger-solang/solang/tree/main/docs/examples/soroban/atomic_swap>`_
     - Atomic swap between two parties, with companion token contracts.
   * - `atomic_multiswap <https://github.com/stellar/soroban-examples/tree/main/atomic_multiswap>`_
     - `docs/examples/soroban/atomic_multiswap.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/atomic_multiswap.sol>`_ and `tests/soroban_testcases/example_atomic_multiswap.rs <https://github.com/hyperledger-solang/solang/blob/main/tests/soroban_testcases/example_atomic_multiswap.rs>`_
     - Batches a set of atomic token swaps between multiple parties with simple price matching. Demonstrates ``struct[]`` (array-of-struct) parameters, nested loops, dynamic memory array allocation (``new bool[](n)``), and cross-contract ``call`` into the `atomic_swap <https://github.com/hyperledger-solang/solang/tree/main/docs/examples/soroban/atomic_swap>`_ example via ``abi.encode``. Rather than removing a matched ``swaps_b`` entry (Solidity has no ``remove(i)``), a ``used`` flag array marks matched entries. Tested via ``atomic_multiswap_*`` test cases.
   * - `auth <https://github.com/stellar/soroban-examples/tree/main/auth>`_
     - `docs/examples/soroban/auth.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/auth.sol>`_
     - Simple host-managed authorization via ``requireAuth()``.
   * - `cross_contract <https://github.com/stellar/soroban-examples/tree/main/cross_contract>`_
     - `integration/soroban/caller.sol <https://github.com/hyperledger-solang/solang/blob/main/integration/soroban/caller.sol>`_ and `integration/soroban/callee.sol <https://github.com/hyperledger-solang/solang/blob/main/integration/soroban/callee.sol>`_
     - Covered in `cross_contract.spec.js <https://github.com/hyperledger-solang/solang/blob/main/integration/soroban/cross_contract.spec.js>`_.
   * - `custom_types <https://github.com/stellar/soroban-examples/tree/main/custom_types>`_
     - `docs/examples/soroban/custom_types.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/custom_types.sol>`_ and `tests/soroban_testcases/example_custom_types.rs <https://github.com/hyperledger-solang/solang/blob/main/tests/soroban_testcases/example_custom_types.rs>`_
     - Struct stored in contract state (``State`` with ``count`` and ``last_incr`` fields). Demonstrates struct storage (VecObject path) and struct ABI return (named-field MAP object). Tested via ``example_custom_types_*`` test cases.
   * - `other_custom_types <https://github.com/stellar/soroban-examples/tree/main/other_custom_types>`_
     - `docs/examples/soroban/other_custom_types.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/other_custom_types.sol>`_ and `tests/soroban_testcases/example_other_custom_types.rs <https://github.com/hyperledger-solang/solang/blob/main/tests/soroban_testcases/example_other_custom_types.rs>`_
     - Type-showcase contract: ABI round-trips for the supported subset (``uint32``/``int32``/``int64``/``int128``/``uint128``/``int256``/``uint256``, ``bool``, ``string``, ``bytes``, ``bytes9``, ``address``), a ``Symbol`` (string) echo, unit enums (``SimpleEnum``/``RoyalCard``), a ``uint32[]`` vector, a ``Test`` struct round-trip, a ``string[]`` return, composite structs (``TupleStruct`` with a nested struct and an enum field, and ``ComplexStruct`` with an address, ``uint64``, a ``uint32[]`` and enum fields), event emission with host authorization (``requireAuth`` + ``emit``), a persistent counter, multiple arguments, a void method, and ``require``-based error handling. ``TupleStruct``/``ComplexStruct`` mirror the upstream composite structs with their sum-type-enum fields adapted to unit enums and vectors (the closest supported types). The upstream methods relying on sum-type enums with associated data (``ComplexEnum``/``ComplexEnum2``/``ComplexEnum3``), tuples, ``Map``, ``Option`` and the untyped ``Val`` are omitted as those types have no Solidity/Soroban-target counterpart. Tested via ``example_other_custom_types_*`` test cases.
   * - `deep_contract_auth <https://github.com/stellar/soroban-examples/tree/main/deep_contract_auth>`_
     - `docs/examples/soroban/deep_auth <https://github.com/hyperledger-solang/solang/tree/main/docs/examples/soroban/deep_auth>`_
     - Nested contract authorization via ``authAsCurrContract(...)``.
   * - `eth_abi <https://github.com/stellar/soroban-examples/tree/main/eth_abi>`_
     - `docs/examples/soroban/eth_abi.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/eth_abi.sol>`_ and `tests/soroban_testcases/example_eth_abi.rs <https://github.com/hyperledger-solang/solang/blob/main/tests/soroban_testcases/example_eth_abi.rs>`_
     - ABI codec round-trip: decode an ``Input`` struct (``bytes32``, two ``uint256``), compute ``Output{a, b + c}``, and re-encode it. Faithful to the upstream *logic* using ``abi.encode``/``abi.decode``, but note the codec is Soroban-native, not Ethereum's 32-byte-word ABI: an encoded buffer holds live host-object handles (a struct becomes a Map object, ``bytes32`` a Bytes object, ``uint256`` a 256-bit-integer object), so the encode → ``exec`` → decode chain must stay inside a single invocation. Tested via ``example_eth_abi_*`` test cases.
   * - `hello_world <https://github.com/stellar/soroban-examples/tree/main/hello_world>`_
     - `docs/examples/soroban/hello_world.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/hello_world.sol>`_ and `tests/soroban_testcases/example_hello_world.rs <https://github.com/hyperledger-solang/solang/blob/main/tests/soroban_testcases/example_hello_world.rs>`_
     - Minimal ``hello(string) -> string[]`` contract mirroring the upstream ``String -> Vec<String>`` example: returns ``["Hello", <name>]``. Demonstrates a ``string`` parameter and a ``string[]`` return value over the Soroban ABI. Tested via ``example_hello_world_*`` test cases.
   * - `increment <https://github.com/stellar/soroban-examples/tree/main/increment>`_
     - `integration/soroban/counter.sol <https://github.com/hyperledger-solang/solang/blob/main/integration/soroban/counter.sol>`_
     - Closest local counterpart for a stored counter that can be incremented.
   * - `increment_with_pause <https://github.com/stellar/soroban-examples/tree/main/increment_with_pause>`_
     - `docs/examples/soroban/increment_with_pause.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/increment_with_pause.sol>`_ and `tests/soroban_testcases/example_increment_with_pause.rs <https://github.com/hyperledger-solang/solang/blob/main/tests/soroban_testcases/example_increment_with_pause.rs>`_
     - Counter that first checks a separate ``Pause`` contract. Demonstrates a cross-contract ``call`` with ``abi.encode``/``abi.decode``, a ``require`` guard, and extending instance storage TTL via ``extendInstanceTtl``. Works together with the `pause <https://github.com/stellar/soroban-examples/tree/main/pause>`_ example. Tested via ``example_increment_with_pause_*`` test cases.
   * - `liquidity_pool <https://github.com/stellar/soroban-examples/tree/main/liquidity_pool>`_
     - `docs/examples/soroban/liquidity_pool <https://github.com/hyperledger-solang/solang/tree/main/docs/examples/soroban/liquidity_pool>`_
     - Liquidity-pool and token-swap example with companion token contracts.
   * - `logging <https://github.com/stellar/soroban-examples/tree/main/logging>`_
     - `docs/examples/soroban/error.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/error.sol>`_
     - Demonstrates ``print()``-based runtime logging in Solang.
   * - `merkle_distribution <https://github.com/stellar/soroban-examples/tree/main/merkle_distribution>`_
     - `docs/examples/soroban/merkle_distribution.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/merkle_distribution.sol>`_ and `tests/soroban_testcases/example_merkle_distribution.rs <https://github.com/hyperledger-solang/solang/blob/main/tests/soroban_testcases/example_merkle_distribution.rs>`_
     - Merkle-proof airdrop: recipients claim tokens by supplying a Merkle proof against a stored root, and verified claims trigger a cross-contract token transfer. Demonstrates ``sha256`` hashing, the ``to_xdr`` builtin for canonical XDR leaf serialization, ``bytes.concat`` for a real 64-byte inner-node preimage, a ``bytes32[]`` proof parameter, a ``mapping(uint32 => bool)`` claimed-set, and cross-contract ``call`` via ``abi.encode``. The leaf preimage is ``sha256(to_xdr(Receiver{index, recipient, amount}))``, mirroring upstream's ``sha256(Receiver.to_xdr())``; it differs only in the field name ``recipient`` (Solidity reserves ``address``), which changes the struct's ScMap keys, so off-chain tree builders must serialize with this contract's exact struct. Tested via ``merkle_distribution_*`` test cases.
   * - `timelock <https://github.com/stellar/soroban-examples/tree/main/timelock>`_
     - `docs/examples/soroban/timelock <https://github.com/hyperledger-solang/solang/tree/main/docs/examples/soroban/timelock>`_
     - Timelock-style example using enums, mappings, authorization, and ``block.timestamp``.
   * - `token <https://github.com/stellar/soroban-examples/tree/main/token>`_
     - `docs/examples/soroban/token.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/token.sol>`_
     - Token-style contract with balances, allowances, and Soroban auth.
   * - `events <https://github.com/stellar/soroban-examples/tree/main/events>`_
     - `docs/examples/soroban/events.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/events.sol>`_
     - Solidity ``event`` declarations and ``emit`` statements, with indexed fields mapping to Soroban topics and non-indexed fields mapping to event data.
   * - `pause <https://github.com/stellar/soroban-examples/tree/main/pause>`_
     - `docs/examples/soroban/pause/pause.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/pause/pause.sol>`_ and `tests/soroban_testcases/example_pause.rs <https://github.com/hyperledger-solang/solang/blob/main/tests/soroban_testcases/example_pause.rs>`_
     - Simple pause-flag contract: a single ``bool`` in instance storage, readable via ``paused()`` and writable via ``set(bool)``. Tested via ``example_pause_*`` test cases.
   * - `single_offer <https://github.com/stellar/soroban-examples/tree/main/single_offer>`_
     - `docs/examples/soroban/single_offer.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/single_offer.sol>`_ and `tests/soroban_testcases/example_single_offer.rs <https://github.com/hyperledger-solang/solang/blob/main/tests/soroban_testcases/example_single_offer.rs>`_
     - Single-offer exchange between a seller and a buyer, using a struct in instance storage, cross-contract token calls, and ``requireAuth()``. Tested via ``example_single_offer_*`` test cases.
   * - `ttl <https://github.com/stellar/soroban-examples/tree/main/ttl>`_
     - `docs/examples/soroban/ttl_storage.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/ttl_storage.sol>`_
     - Extending TTL on stored contract data.
   * - `upgradeable_contract <https://github.com/stellar/soroban-examples/tree/main/upgradeable_contract>`_
     - `docs/examples/soroban/upgradeable_contract.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/upgradeable_contract.sol>`_ and `tests/soroban_testcases/example_upgradeable_contract.rs <https://github.com/hyperledger-solang/solang/blob/main/tests/soroban_testcases/example_upgradeable_contract.rs>`_
     - Admin-gated self-upgrade: replaces the running contract Wasm via ``updateCurrentContractWasm(bytes32)`` (host ``update_current_contract_wasm``), after ``requireAuth()`` on the stored admin. A ``version()`` marker flips 1 → 2 across the upgrade. Tested via ``example_upgradeable_contract_*`` test cases.

Solidity Translations
+++++++++++++++++++++

The following abridged snippets show how selected upstream Soroban examples are expressed in Solang Solidity.

atomic_multiswap
^^^^^^^^^^^^^^^^

Upstream Soroban example: `atomic_multiswap <https://github.com/stellar/soroban-examples/tree/main/atomic_multiswap>`_

Solang Solidity example: `docs/examples/soroban/atomic_multiswap.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/atomic_multiswap.sol>`_

Batches a set of atomic token swaps between multiple parties, matching each ``swaps_a`` entry against the first compatible ``swaps_b`` entry and settling it through a deployed ``atomic_swap`` contract. Soroban memory arrays do support ``push``/``pop``, but Solidity has no ``remove(i)`` for deleting an arbitrary element; rather than emulating removal, matched ``swaps_b`` entries are marked in a ``used`` flag array.

.. code-block:: solidity

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

pause
^^^^^

Upstream Soroban example: `pause <https://github.com/stellar/soroban-examples/tree/main/pause>`_

Solang Solidity example: `docs/examples/soroban/pause/pause.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/pause/pause.sol>`_

.. code-block:: solidity

    contract Pause {
        bool instance paused_flag = false;

        function paused() public view returns (bool) {
            return paused_flag;
        }

        function set(bool paused) public {
            paused_flag = paused;
        }
    }


auth
^^^^

Upstream Soroban example: `auth <https://github.com/stellar/soroban-examples/tree/main/auth>`_

Solang Solidity example: `docs/examples/soroban/auth.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/auth.sol>`_

.. code-block:: solidity

    contract auth {
        address public owner =
            address"GDRIX624OGPQEX264NY72UKOJQUASHU3PYKL6DDPGSTWXWJSBOTR6N7W";

        uint64 public instance counter = 20;

        function increment() public returns (uint64) {
            owner.requireAuth();
            counter = counter + 1;
            return counter;
        }
    }

custom_types
^^^^^^^^^^^^

Upstream Soroban example: `custom_types <https://github.com/stellar/soroban-examples/tree/main/custom_types>`_

Solang Solidity example: `docs/examples/soroban/custom_types.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/custom_types.sol>`_

.. code-block:: solidity

    contract CustomTypes {
        struct State {
            uint32 count;
            uint32 last_incr;
        }
        State state;

        function increment(uint32 incr) public returns (uint32) {
            state.count += incr;
            state.last_incr = incr;
            return state.count;
        }

        function get_state() public view returns (State memory) {
            return state;
        }
    }

other_custom_types
^^^^^^^^^^^^^^^^^^

Upstream Soroban example: `other_custom_types <https://github.com/stellar/soroban-examples/tree/main/other_custom_types>`_

Solang Solidity example: `docs/examples/soroban/other_custom_types.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/other_custom_types.sol>`_

The upstream contract is a showcase of every custom and primitive type the
Soroban host understands. This port mirrors every upstream method with a
Solidity/Soroban-target counterpart, following the upstream method names and
ordering — the primitive echoes, a ``Symbol`` (string) echo, unit enums, a
``uint32[]`` vector, struct round-trips, event emission with host
authorization, a persistent counter, multiple arguments, a void method and
``require``-based error handling. The only names that diverge from upstream are
``bytes_`` and ``string_`` (``bytes`` and ``string`` are Solidity type
keywords):

.. code-block:: solidity

    contract other_custom_types {
        struct Test {
            uint32 a;
            bool b;
            string c;
        }

        enum SimpleEnum { First, Second, Third }

        event AuthEvent(address indexed hello, string world);

        uint32 persistent count;

        function inc() public returns (uint32) {
            count += 1;
            return count;
        }

        function auth(address addr, string memory world) public returns (address) {
            addr.requireAuth();
            emit AuthEvent(addr, world);
            return addr;
        }

        function simple(SimpleEnum v) public pure returns (SimpleEnum) {
            return v;
        }

        function vec(uint32[] memory v) public pure returns (uint32[] memory) {
            return v;
        }

        function strukt(Test memory t) public pure returns (Test memory) {
            return t;
        }

        function u32_fail_on_even(uint32 v) public pure returns (uint32) {
            require(v % 2 == 1, "NumberMustBeOdd");
            return v;
        }
    }

The composite structs ``TupleStruct`` and ``ComplexStruct`` are ported with
their sum-type-enum fields adapted to unit enums and ``uint32[]`` vectors, the
closest supported types. The upstream methods relying on sum-type enums with
associated data (``ComplexEnum``/``ComplexEnum2``/``ComplexEnum3``), tuples,
``Map``, ``Option`` and the untyped ``Val`` are omitted, as those types have no
Solidity/Soroban-target counterpart.

token
^^^^^

Upstream Soroban example: `token <https://github.com/stellar/soroban-examples/tree/main/token>`_

Solang Solidity example: `docs/examples/soroban/token.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/token.sol>`_

.. code-block:: solidity

    contract token {
        address public admin;
        mapping(address => int128) public balances;

        constructor(address _admin, string memory _name, string memory _symbol, uint32 _decimals) {
            admin = _admin;
        }

        function mint(address to, int128 amount) public {
            require(amount >= 0, "Amount must be non-negative");
            admin.requireAuth();
            balances[to] = balances[to] + amount;
        }

        function transfer(address from, address to, int128 amount) public {
            from.requireAuth();
            balances[from] = balances[from] - amount;
            balances[to] = balances[to] + amount;
        }
    }

timelock
^^^^^^^^

Upstream Soroban example: `timelock <https://github.com/stellar/soroban-examples/tree/main/timelock>`_

Solang Solidity example: `docs/examples/soroban/timelock/timelock.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/timelock/timelock.sol>`_

.. code-block:: solidity

    contract timelock {
        enum TimeBoundKind { Before, After }

        struct TimeLock {
            TimeBoundKind kind;
            uint64 bound_timestamp;
            address claimant;
            uint64 amount;
        }

        mapping(address => TimeLock) public timelocks;

        function is_claimable(address claimant) public view returns (bool) {
            TimeLock storage tl = timelocks[claimant];
            return block.timestamp >= tl.bound_timestamp;
        }
    }

ttl
^^^

Upstream Soroban example: `ttl <https://github.com/stellar/soroban-examples/tree/main/ttl>`_

Solang Solidity example: `docs/examples/soroban/ttl_storage.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/ttl_storage.sol>`_

.. code-block:: solidity

    contract ttl_storage {
        uint64 public persistent pCount = 11;
        uint64 temporary tCount = 7;
        uint64 instance iCount = 3;

        function extend_persistent_ttl() public view returns (int64) {
            return pCount.extendTtl(1000, 5000);
        }

        function extend_temp_ttl() public view returns (int64) {
            return tCount.extendTtl(3000, 7000);
        }
    }

events
^^^^^^

Upstream Soroban example: `events <https://github.com/stellar/soroban-examples/tree/main/events>`_

Solang Solidity example: `docs/examples/soroban/events.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/events.sol>`_

.. code-block:: solidity

    contract IncrementContract {
        uint32 public instance count = 0;
        event IncrementEvent(string indexed action, string indexed method, uint32 count);

        function increment() public returns (uint32) {
            count += 1;
            emit IncrementEvent("COUNTER", "increment", count);
            return count;
        }
    }

increment_with_pause
^^^^^^^^^^^^^^^^^^^^

Upstream Soroban example: `increment_with_pause <https://github.com/stellar/soroban-examples/tree/main/increment_with_pause>`_

Solang Solidity example: `docs/examples/soroban/increment_with_pause.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/increment_with_pause.sol>`_

.. code-block:: solidity

    contract IncrementContract {
        address public instance pause_contract;
        uint32 public instance count = 0;

        constructor(address _pause) {
            pause_contract = _pause;
        }

        function increment() public returns (uint32) {
            bytes payload = abi.encode("paused");
            (, bytes memory ret) = pause_contract.call(payload);
            bool is_paused = abi.decode(ret, (bool));
            require(!is_paused, "Paused");

            count += 1;
            extendInstanceTtl(50, 100);
            return count;
        }
    }

hello_world
^^^^^^^^^^^

Upstream Soroban example: `hello_world <https://github.com/stellar/soroban-examples/tree/main/hello_world>`_

Solang Solidity example: `docs/examples/soroban/hello_world.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/hello_world.sol>`_

.. code-block:: solidity

    contract hello_world {
        function hello(string memory to) public pure returns (string[] memory) {
            string[] memory result = new string[](2);
            result[0] = "Hello";
            result[1] = to;
            return result;
        }
    }

single_offer
^^^^^^^^^^^^

Upstream Soroban example: `single_offer <https://github.com/stellar/soroban-examples/tree/main/single_offer>`_

Solang Solidity example: `docs/examples/soroban/single_offer.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/single_offer.sol>`_

.. code-block:: solidity

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

upgradeable_contract
^^^^^^^^^^^^^^^^^^^^

Upstream Soroban example: `upgradeable_contract <https://github.com/stellar/soroban-examples/tree/main/upgradeable_contract>`_

Solang Solidity example: `docs/examples/soroban/upgradeable_contract.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/upgradeable_contract.sol>`_

Replaces the running contract's Wasm in place. The admin-gated ``upgrade`` first calls ``requireAuth()`` on the stored admin, then hands an already-uploaded Wasm hash to ``updateCurrentContractWasm(bytes32)``, which lowers to the host function ``update_current_contract_wasm``. The ``version()`` marker returns 1 in this build and 2 in the upgraded build, so a caller can observe the swap.

.. code-block:: solidity

    contract UpgradeableContract {
        address instance admin;

        constructor(address _admin) {
            admin = _admin;
        }

        function version() public pure returns (uint32) {
            return 1;
        }

        function upgrade(bytes32 new_wasm_hash) public {
            admin.requireAuth();
            updateCurrentContractWasm(new_wasm_hash);
        }
    }

merkle_distribution
^^^^^^^^^^^^^^^^^^^

Upstream Soroban example: `merkle_distribution <https://github.com/stellar/soroban-examples/tree/main/merkle_distribution>`_

Solang Solidity example: `docs/examples/soroban/merkle_distribution.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/merkle_distribution.sol>`_

A Merkle-proof airdrop. The constructor stores the tree root and pulls a
funding deposit from a token contract; ``claim`` recomputes the Merkle root
from a leaf and its proof, verifies it against the stored root, and pays the
receiver. Inner nodes hash a real 64-byte buffer via ``bytes.concat`` (not
``abi.encode``, which on Soroban would pack ScVal object handles). The leaf is
``sha256(to_xdr(Receiver{index, recipient, amount}))``, matching upstream's
``sha256(Receiver.to_xdr())`` — the ``to_xdr`` builtin lowers to the Soroban
host's ``serialize_to_bytes``, the same primitive the Rust SDK uses, so the leaf
commits to the value's canonical XDR rather than to ephemeral ScVal object
handles. The one divergence from upstream is the field name ``recipient``
(Solidity reserves ``address`` as a keyword), which changes the struct's ScMap
symbol keys and therefore the XDR bytes, so an off-chain tree builder must
serialize with this contract's exact field names.

.. code-block:: solidity

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

        function claim(
            uint32 index,
            address receiver,
            int128 amount,
            bytes32[] memory proof
        ) public {
            require(!claimed[index], "AlreadyClaimed");
            Receiver memory node = Receiver(index, receiver, amount);
            bytes32 leaf = sha256(to_xdr(node));
            require(compute_root(leaf, proof) == rootHash, "InvalidProof");
            bytes payload = abi.encode(
                "transfer", address(this), receiver, amount
            );
            (bool ok, ) = tokenAddress.call(payload);
            require(ok, "payout transfer failed");
            claimed[index] = true;
        }
    }

eth_abi
^^^^^^^

Upstream Soroban example: `eth_abi <https://github.com/stellar/soroban-examples/tree/main/eth_abi>`_

Solang Solidity example: `docs/examples/soroban/eth_abi.sol <https://github.com/hyperledger-solang/solang/blob/main/docs/examples/soroban/eth_abi.sol>`_

The upstream example decodes an ABI-encoded ``Input``, computes an ``Output`` whose ``r`` is the sum of the two input words, and returns the ABI-encoded ``Output``. The port keeps that logic in ``exec`` using ``abi.decode``/``abi.encode``. On the Soroban target these builtins operate on Soroban's native value model rather than Ethereum's 32-byte-word ABI: an encoded buffer carries live host-object handles (a struct becomes one Map object, ``bytes32`` a Bytes object, ``uint256`` a 256-bit-integer object), not portable padded words. Those handles are only valid within the invocation that produced them, so the ``run`` driver performs the whole encode → ``exec`` → decode chain in a single call.

.. code-block:: solidity

    contract EthAbi {
        struct Input  { bytes32 a; uint256 b; uint256 c; }
        struct Output { bytes32 a; uint256 r; }

        function exec(bytes memory input) public pure returns (bytes memory) {
            Input memory i = abi.decode(input, (Input));
            return abi.encode(Output(i.a, i.b + i.c));
        }

        function run(bytes32 a, uint256 b, uint256 c) public pure returns (Output memory) {
            bytes memory input = abi.encode(Input(a, b, c));
            bytes memory output = exec(input);
            return abi.decode(output, (Output));
        }
    }

Upstream Examples Not Yet Documented as Supported
+++++++++++++++++++++++++++++++++++++++++++++++++

The following upstream examples do not currently have a documented Solidity counterpart, as some needed Soroban features are not yet supported.
- `bls_signature <https://github.com/stellar/soroban-examples/tree/main/bls_signature>`_
- `deployer <https://github.com/stellar/soroban-examples/tree/main/deployer>`_
- `errors <https://github.com/stellar/soroban-examples/tree/main/errors>`_
- `fuzzing <https://github.com/stellar/soroban-examples/tree/main/fuzzing>`_
- `mint-lock <https://github.com/stellar/soroban-examples/tree/main/mint-lock>`_
- `privacy-pools <https://github.com/stellar/soroban-examples/tree/main/privacy-pools>`_
- `simple_account <https://github.com/stellar/soroban-examples/tree/main/simple_account>`_
- `workspace <https://github.com/stellar/soroban-examples/tree/main/workspace>`_

Want to add support for one of the remaining examples? Open a pull request against `hyperledger-solang/solang <https://github.com/hyperledger-solang/solang>`_ and follow the `contribution guide <https://github.com/hyperledger-solang/solang/blob/main/CONTRIBUTING.md>`_.
