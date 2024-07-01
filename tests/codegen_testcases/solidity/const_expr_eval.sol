// RUN: --target polkadot --emit cfg
contract ConstExprEvaluate {
    // BEGIN-CHECK: ConstExprEvaluate::ConstExprEvaluate::function::less
    function less() public pure returns (bool r) {
        int a = 100;
        int b = 200;

        // CHECK: r = true
        r = a < b;
    }
}