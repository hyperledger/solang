
        contract b {
            int32 s;

            function step1() public returns (int32) {
                this.other({n: 102});
                return s;
            }

            function other(int32 n) private {
                s = n;
            }
        }
// ---- Expect: diagnostics ----
// error: 6:17-37: function 'other' is not 'public' or 'external'
