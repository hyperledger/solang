// RUN: --target polkadot --emit cfg

// Tests control commands
contract c1 {

//BEGIN-CHECK: c1::function::test1
    function test1(int a, int b) public pure returns (int) {
        int x;
        // CHECK: ty:int256 %1.cse_temp = ((arg #0) + (arg #1))
        // CHECK: ty:int256 %x = (%1.cse_temp - int256 54)
        x = a+b-54;
        // CHECK: ty:int256 %d = (%x * %1.cse_temp)
        int d = x*(a+b);

        // CHECK: ty:int256 %2.cse_temp = ((arg #0) - (arg #1))
        if (x + d > 0) {
            // NOT-CHECK: ty:int256 %t = ((arg #0) - (arg #1))
            // CHECK: ty:int256 %t = %2.cse_temp
			int t = a-b;
			bool e1 = t>3;
		}
		 else {
            // NOT-CHECK: ty:int256 %e = ((arg #0) - (arg #1))
            // CHECK: ty:int256 %e = %2.cse_temp
            int e = a-b;
            bool e2 = e > 3;
        }

        return x-d + (a-b);
        // CHECK: return ((%x - %d) + %2.cse_temp)
    }

// BEGIN-CHECK: c1::function::test2
    function test2(int a, int b) public pure returns (int) {
        int x;
        // CHECK: ty:int256 %1.cse_temp = ((arg #0) + (arg #1))
        x = a+b-54;
        // CHECK: ty:int256 %x = (%1.cse_temp - int256 54)
        int d = x*(a+b);
        // CHECK: ty:int256 %d = (%x * %1.cse_temp)
        // CHECK: ty:int256 %2.cse_temp = (%x + %d)
        // CHECK: ty:int256 %3.cse_temp = ((arg #0) - (arg #1))
        // CHECK: branchcond (signed more %2.cse_temp > int256 0), block1, block2
        if (x + d > 0) {
			int t = a-b;
            // CHECK: ty:int256 %t = %3.cse_temp
			bool e1 = t>3;
        // CHECK: return ((%x - %d) + %3.cse_temp)
		}
		 else if (x+d < 0) {
            int e = a-b;
            // CHECK: ty:int256 %e = %3.cse_temp
            bool e2 = e > 3;
        } else {
            int k = a-b;
            // CHECK: ty:int256 %k = %3.cse_temp
            bool e3 = k < 4;
        }


        return x-d + (a-b);
    }

// BEGIN-CHECK: c1::function::test3
    function test3(int a, int b) public pure returns (int) {
        int x;
        x = a+b-54;
        int d = x*(a+b);

        // CHECK: branchcond (signed more %2.cse_temp > int256 0), block1, block2
        if (x + d > 0) {
            // CHECK: ty:int256 %t = ((arg #0) - (arg #1))
			int t = a-b;
			bool e1 = t>3;
            // CHECK: branchcond (signed less %2.cse_temp < int256 0), block4, block5
            // CHECK: return ((%x - %d) + ((arg #0) - (arg #1)))
		}
		 else if (x+d < 0) {
            // CHECK: ty:int256 %e = ((arg #0) - (arg #1))
            int e = a-b;
            bool e2 = e > 3;
            // CHECK: branchcond (%2.cse_temp == int256 0), block7, block8
        } else if (x + d == 0){
            // CHECK: ty:int256 %k = %1.cse_temp
            int k = a+b;
            bool e3 = k < 4;
        }

        return x-d + (a-b);
    }

// BEGIN-CHECK: c1::function::test4
    function test4(int a, int b) public pure returns (int) {
        int x;
        x = a+b-54;
        int d = x*(a+b);
        int p = x+d;
        // CHECK: ty:int256 %2.cse_temp = ((arg #0) + (arg #1))
        // CHECK: ty:int256 %1.cse_temp = (%2.cse_temp - int256 54)
        // CHECK: ty:int256 %x = %1.cse_temp
	    // CHECK: ty:int256 %d = (%1.cse_temp * %2.cse_temp)
	    // CHECK: ty:int256 %p = (%1.cse_temp + %d)

        // CHECK: ty:int256 %3.cse_temp = (%x + %d)
        // CHECK: branchcond (signed more %3.cse_temp > int256 0), block2, block3
        while (x+d > 0) {
            // CHECK: ty:int256 %t = ((arg #0) - (arg #1))
            int t = a-b;
            bool e1 = t > 3;
            // CHECK: ty:int256 %x = %3.cse_temp
			x = x+d;
        }

        // CHECK: return (((%x - %d) + ((arg #0) - (arg #1))) - %p)
        return x-d + (a-b) - p;
    }

// BEGIN-CHECK: c1::function::test5
    function test5(int a, int b) public pure returns (int) {
        int x;
        x = a+b-54;
        int d = x*(a+b);

       	for(int i=0; i<10; i++) {
            // CHECK: ty:int256 %t = ((arg #0) - (arg #1))
			int t = a-b;
            // CHECK: ty:int256 %i = (%temp.187 + int256 1)
			bool e1 = t > 3;
		}

// CHECK: return ((%x - %d) + ((arg #0) - (arg #1)))
        return x-d + (a-b);
    }

// BEGIN-CHECK: c1::function::test6
    function test6(int a, int b) public pure returns (int) {
        int x;
        x = a+b-54;
        int d = x*(a+b);

       	do {
			int t = a-b;
			bool e1 = t > 3;
            // CHECK: ty:int256 %x = (%x + %d)
			x = x+d;
        // CHECK: branchcond (signed more (%x + %d) > int256 0), block1, block3
		} while(x+d > 0);
        int t = 3;
        bool p = t < 2;

        // CHECK: return ((%x - %d) + %t
        return x-d + (a-b);
    }

// BEGIN-CHECK: c1::function::test7
    function test7(int a, int b) public pure returns (int) {
             int x;
        // CHECK: ty:int256 %1.cse_temp = ((arg #0) + (arg #1))
        x = a+b-54;
        // CHECK: ty:int256 %x = (%1.cse_temp - int256 54)
        int d = x*(a+b);
        // CHECK: ty:int256 %d = (%x * %1.cse_temp)
        // CHECK: ty:int256 %2.cse_temp = (%x + %d)
        // CHECK: ty:int256 %3.cse_temp = ((arg #0) - (arg #1))
        // CHECK: branchcond (signed more %2.cse_temp > int256 0), block1, block2
        if (x + d > 0) {
			int t = a-b;
            // CHECK: ty:int256 %t = %3.cse_temp
			bool e1 = t>3;
        // CHECK: return ((%x - %d) + %3.cse_temp)
		}
		 else if (x+d < 0) {
            int e = a-b;
            // CHECK: ty:int256 %e = %3.cse_temp
            bool e2 = e > 3;
        } else if (x+d == 0){
            int k = a-b;
            // CHECK: ty:int256 %k = %3.cse_temp
            bool e3 = k < 4;
        } else {
            int k1 = a-b;
            // CHECK: ty:int256 %k1 = %3.cse_temp
            bool e4 = k1 < 4;
        }


        return x-d + (a-b);
    }

    int k=2;

// BEGIN-CHECK: c1::function::test8
    function test8(int a, int b) public view returns (int ret) {
        int x = a + b +k;
        // CHECK: ty:int256 %x = (%1.cse_temp + %temp.
        if(x  + k < 0) {
            // CHECK: ty:uint256 %p = uint256((%1.cse_temp + %temp.
            uint p = uint(a+b+k);
            bool e = p > 50;
        }

        // CHECK: ty:uint256 %p2 = uint256((%1.cse_temp + %temp.
        uint p2 = uint(a+b+k);
        // CHECK: ty:int256 %2.cse_temp = int256((%p2 + uint256 9))
        int r1 = int(p2+9) -4;
        // CHECK: ty:int256 %r1 = (%2.cse_temp - int256 4)
        int r2= int(p2+9) -9;
        // CHECK: ty:int256 %r2 = (%2.cse_temp - int256 9)

        // CHECK: ty:int256 %3.cse_temp = -%r1
        // CHECK: ty:int256 %ret = %3.cse_temp
        ret = -r1;

        // CHECK: ty:int256 %ret = (%3.cse_temp + %r2)
        ret = -r1 + r2;
    }

      struct stTest {
        int a;
        uint b;
    }

// BEGIN-CHECK: c1::function::test9
    function test9(int a, int b) public view returns (int ret) {
        stTest struct_instance = stTest(2, 3);
        // CHECK:  ty:int256 %1.cse_temp = ((arg #0) + (arg #1))
        int x = a + b + struct_instance.a;
        // CHECK: ty:int256 %x = (%1.cse_temp + (load (struct %struct_instance field 0)))
        // CHECK: branchcond (signed less (%x + int256((load (struct %struct_instance field 1)))) < int256 0)
        if(x  + int(struct_instance.b) < 0) {
            // CHECK: ty:uint256 %p = uint256((%1.cse_temp + (load (struct %struct_instance field 0))))
            uint p = uint(a+b+struct_instance.a);
            bool e = p > 50;
        }


        int8 trunc = int8(x);
        // CHECK: ty:bool %e2 = (signed more (sext int16 %trunc) > int16 2)
        bool e2 = trunc > 2;
        int8 trunc2 = 8 + int8(x);
        bool e3 = trunc2 < trunc;
        bool e4 = e2 || e3;
        // CHECK: branchcond %e3, block3, block4
        if (trunc2 < trunc && trunc > 2) {
            // CHECK: = %e2
            // CHECK: ty:int256 %2.cse_temp = ((arg #0) * (arg #1))
            // CHECK: ty:int256 %p2 = %1.cse_temp
            int p2 = a+b;
            int p3 = p2 - x + a + b;
            int p4 = p2-x;
            int p5 = p3 + a*b+45;
            // CHECK: ty:int256 %p5 = ((%p3 + %2.cse_temp) + int256 45)

            // CHECK: return %2.cse_temp
            if (p5 !=0) {
                // CHECK: ty:uint16 %t1 = (trunc uint16 %p5)
                uint16 t1 = uint16(p3 + a*b +45);
                // CHECK: ty:uint32 %t2 = (trunc uint32 %2.cse_temp)
                uint32 t2 = uint32(a*b);
                bool e5 = t2 < t1;
            }

            // CHECK: ty:int256 %ret = %p5
            ret = p3 + a*b + 45;
        }

        ret = a*b;
    }

// BEGIN-CHECK: c1::function::test10
    function test10(int a, int b) public pure returns (int) {
        int x;
        x = a+b-54;
        int d = x*(a+b);
        int k = x+d;
        bool e = k < 0;

       	do {
			int t = a-b;
			bool e1 = t > 3;
            // CHECK: ty:int256 %x = (%x + %d)
			x = x+d;
        // CHECK: branchcond (signed more (%x + %d) > int256 0), block1, block3
		} while(x+d > 0);
        int t = 3;
        bool p = t < 2;

        // CHECK: return ((%x - %d) + %t
        return x-d + (a-b);
    }


    function get(int a, int b) private pure returns (int) {
        return a+b+1;
    }

    event testEvent(int a, int b, string str);
    // BEGIN-CHECK: c1::function::test11
    function test11(int a, int b) public returns (int) {
        string ast = "Hello!";
        string bst = "from Solang";
        string cst = string.concat(ast, bst);
        // CHECK: ty:int256 %1.cse_temp = (signed divide (arg #0) / (int256 2 * (arg #1)))
        // CHECK: call c1::c1::function::get__int256_int256 %1.cse_temp, (arg #1)
        int p = a + get(a/(2*b), b);

        bool e = (ast == bst) || p < 2;
        // CHECK: ty:bool %3.cse_temp = (strcmp (%ast) (%bst))
        // CHECK: branchcond %3.cse_temp, block2, block1
        bool e2 = e;
        // CHECK: branchcond (strcmp ((builtin Concat (%ast, %bst))) (%cst)), block3, block4
        if (string.concat(ast, bst) == cst) {
            // CHECK: call c1::c1::function::get__int256_int256 %1.cse_temp, (arg #1)
            require(a + get(a/(2*b), b) < 0);
            emit testEvent(a + get(a/(2*b) -p, b), p, string.concat(ast, bst));
        }

        // CHECK: branchcond %3.cse_temp, block21, block22
        if (ast == bst) {
            ast = string.concat(ast, "b");
        }
        // CHECK: call c1::c1::function::get__int256_int256 (%1.cse_temp - %p), (arg #1)

        // CHECK: branchcond (strcmp (%ast) (%bst)), block24, block25
        while (ast == bst) {
            ast = string.concat(ast, "a");
        }

        // CHECK: call c1::c1::function::get__int256_int256 (arg #1), (signed divide (arg #0) / (arg #1))
        return get(b, a/b);
    }

    // BEGIN-CHECK: c1::function::test12
    function test12(int a, int b) public returns (int) {
        int x = a+b;
        bool e = (x == a);
        // NOT-CHECK: %1.cse_temp =
        bool e2 = e;

        // CHECK: branchcond (%x == (arg #0))
        while(x == a) {
            x = x+1;
            // NOT-CHECK: cse_temp
            x+=1;
            x++;
            x--;
            x++;
            x--;
            --x;
            ++x;
        }

        return x;
    }

    function testing(bytes b) public returns (string) {
        return string(b);
    }

  // BEGIN-CHECK: c1::function::test13
    function test13(int a, int b) public returns (int) {
        string c = "Hello";
        bytes b1 = bytes(c);
        string b2 = string(b1);
        string b3 = b2;
        int[4] vec = [1, 2, 3, 4];
        // CHECK: ty:int256 %1.cse_temp = ((arg #0) + (arg #1))
        int x = (a+b) - (vec[1]-vec[2]);
        // CHECK: ty:int256 %x = (%1.cse_temp - ((load (subscript int256[4] %vec[uint32 1])) - (load (subscript int256[4] %vec[uint32 2]))))
        bool k3 = x < 1;
        // CHECK: = uint256(%1.cse_temp)
        vec[uint(a+b)] = 54*(a+b);
        // CHECK: = (int256 54 * %1.cse_temp)
        // CHECK: = uint256((int256 1 - %1.cse_temp))
        vec[uint(1-(a+b))] = vec.length - (a+b);

        // CHECK: = (int256 4 - %1.cse_temp)
        if(vec.length - (a+b) == 1) {
            // CHECK:  call c1::c1::function::testing__bytes %c
            string k = testing(bytes(c));
            string p = string.concat("a", k);
            // CHECK: ty:string %p = (builtin Concat ((alloc string uint32 1 "a"), %k))
            // CHECK: branchcond ((builtin ArrayLength (%p)) == uint32 2), block11, block12
            if(p.length == 2) {
                // CHECK: ty:string %p1 = (builtin Concat ((alloc string uint32 1 "a"), %k))
                string p1 = string.concat("a", k);
                string l = p1;
            }
        }

        // CHECK: branchcond (signed less %2.cse_temp < int256 0), block14, block15
        while(a+b < 0) {
            // CHECK: branchcond (strcmp (%c) ("a")), block16, block17
            if("a" == c) {
                a = a+b;
            }
        }

        do {
            // CHECK: branchcond (strcmp (%c) ("a")), block21, block22
            if("a" == c) {
                a = a+b;
            }
            // CHECK: branchcond (signed more (%a + (arg #1)) > int256 0), block18, block20
        } while(a+b > 0);

        for(int p=0; p<a; ++p) {
            b1.push();
            // CHECK: = call c1::c1::function::testing__bytes %b1
            string k1 = testing(bytes(string(b1)));
            string k2 = k1;
        }

        return 2;
    }

    function doNothing(bytes32 b) private {
        b = hex"abcd";
    }

    // BEGIN-CHECK: c1::function::test14
     function test14(int a, uint b) public returns (int) {
        string c = "Hello";
        bytes b3 = bytes(c);
        bytes32 b1 = bytes32(b3);


        for(int p=0; p<a; ++p) {
            doNothing(b1);
            // CHECK: ty:bytes32 %b2 = %b1
            bytes32 b2 = bytes32(b3);
            doNothing(b2);
        }

        b3 = bytes("d");
        for(int p=0; p<a; ++p) {
            doNothing(b1);
            // CHECK: ty:bytes32 %b2.155 = bytes32 from:bytes (%b3)
            bytes32 b2 = bytes32(b3);
            doNothing(b2);
        }

        return 2;
    }

  // BEGIN-CHECK: c1::function::test15
    function test15(uint a, uint b) public pure returns (uint) {
        uint c = a << b;
        bool b1 = c > 0;
        // CHECK: ty:uint256 %1.cse_temp = ((arg #0) << (arg #1))
	    // CHECK: ty:uint256 %c = %1.cse_temp
	    // CHECK: ty:bool %b1 = (unsigned more %1.cse_temp > uint256 0)
	    // CHECK: ty:bool %2.cse_temp = !%b1
	    // CHECK: branchcond %2.cse_temp, block1, block2
        if (!b1) {
            // CHECK: return (%1.cse_temp + uint256 1)
            return (a << b) + 1;
        }

        // CHECK: branchcond %2.cse_temp, block4, block3
        if(!b1 || c > 0) {
            // CHECK: = %b1
            // CHECK: return ((arg #0) << ((arg #1) + uint256 1))
            return a << b + 1;
        }

        // CHECK: branchcond (unsigned more %c > uint256 0), block11, block12
        for(int i=0; c > 0 && i<10; ++i) {
            c++;
        }

        // CHECK: ty:uint256 %3.cse_temp = ((arg #0) & (arg #1))
        // CHECK: branchcond (%3.cse_temp == uint256 0), block13, block14
        if (a & b == 0) {
            return c--;
        }

        // CHECK: branchcond (unsigned more %3.cse_temp > uint256 1), block15, block16
        if (a & b > 1) {
            return a;
        }

        return c;
    }

    // BEGIN-CHECK: c1::function::test16
    function test16(int a, int b) public pure returns (int) {
        int k = (a-b);
         bool e = k>0;

        for(int i=1; a-b < 0; i++) {
            // CHECK: ty:int256 %4.cse_temp = (signed divide %k / (arg #0))
            // CHECK: ty:int256 %p = ((%1.cse_temp * int256 5) - %4.cse_temp)
            int p = (a-b)*5-k/a;
            b++;
            // CHECK: ty:int256 %1.cse_temp = ((arg #0) - %b)
            // CHECK: branchcond (signed less %1.cse_temp < int256 0), block1, block4
            // CHECK: 	ty:int256 %2.cse_temp = ((arg #0) - %b)
            // CHECK: branchcond (signed more %2.cse_temp > int256 0), block6, block7
            while(a-b > 0) {
                // CHECK: ty:int256 %p = (%2.cse_temp * int256 5)
                p = (a-b)*5;
                b--;
            }
            bool e2 = p<1;
        }

        do {
            // CHECK: ty:int256 %p.170 = ((((arg #0) - %b) * int256 5) - %4.cse_temp)
            int p = (a-b)*5-k/a;
            b++;
            bool e2 = p<1;
            // CHECK: branchcond (signed less ((arg #0) - %b) < int256 0), block8, block10
        }while(a - b < 0);

        int g = b;
        // CHECK: ty:uint256 %p1 = (uint256((arg #0)) ** uint256(%g))
        uint p1 = uint(a)**uint(g);
        bool e9 = p1 == 0;
        // CHECK: ty:int256 %3.cse_temp = ((arg #0) - %b)
        // CHECK: branchcond (signed less %3.cse_temp < int256 0), block12, block13
        while(a - b < 0) {
            // CHECK: = ((%3.cse_temp * int256 5) - %4.cse_temp)
            int p = (a-b)*5-k/a;
            b=4;
            // CHECK: ty:int256 %5.cse_temp = ((arg #0) - int256 4)
            // CHECK: branchcond (signed more %5.cse_temp > int256 0), block14, block15
            if (a-b > 0) {
                // CHECK: return (%4.cse_temp + int256(%p1))
                // CHECK:  = (%5.cse_temp * int256 4)
                p = (a-b)*4;
                b++;
            }
            bool e2 = p<1;
        }

        return k/a + int(uint(a)**uint(g));
    }

}
