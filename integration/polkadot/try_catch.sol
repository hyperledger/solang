contract TryCatchCaller {
    constructor() payable {}

    function test(uint128 div) public payable returns (uint128) {
        TryCatchCallee contract_instance = new TryCatchCallee();

        try contract_instance.test(div) returns (uint128) {
            return 4;
        } catch Error(string reason) {
            assert(reason == "foo");
            return 1;
        } catch Panic(uint reason) {
            assert(reason == 0x12);
            return 0;
        } catch (bytes raw) {
            if (raw.length == 0) {
                return 3;
            }
            if (raw == hex"bfb4ebcf") {
                return 2;
            }
        }

        assert(false);
    }
}

contract TryCatchCallee {
    error Foo();

    // div = 0: Reverts with Panic error
    // div = 1: Reverts with Error error
    // div = 2: Reverts with Foo error
    // div = 3: Reverts with empty error
    // div > 3: Doesn't revert
    function test(uint128 div) public pure returns (uint128) {
        if (div == 1) {
            revert("foo");
        }
        if (div == 2) {
            revert Foo();
        }
        if (div == 3) {
            revert();
        }
        return 123 / div;
    }
}
