contract C {
    uint public constant Y = 0e1000;
    constructor(int[Y - .1] memory w) {}
}

// ---- Expect: diagnostics ----
// error: 2:30-36: exponent '1000' too large
