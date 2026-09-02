// SPDX-License-Identifier: Apache-2.0

use crate::build_solidity;
use soroban_sdk::{IntoVal, Val};

// TODO: check this test since it takes too much time.
#[test]
fn storage_array_ops_test() {
    let contract_src = r#"
        contract storage_array {
            uint64[] mylist;
            uint64 normal = 20;

            function push_pop() public returns (uint64) {
                mylist.push(5);

                mylist[0] = 15;

                mylist.push(5);

                return mylist[0] + mylist[1];
            }

            function loop() public returns (uint64) {
                uint64 sum = 0;

                mylist.push(5);
                mylist.push(10);
                mylist.push(15);

                for (uint64 i = 0; i < mylist.length; i++) {
                    sum += mylist[i];
                }

                return sum;
            }

            function random_access(uint64 index) public returns (uint64) {
                uint64 sum = 0;

                mylist.push(5);
                mylist.push(10);
                mylist.push(15);

                sum += mylist[index];
                sum += mylist[index + 1];

                return sum;
            }

            function pop_len() public returns (uint64) {
                mylist.push(1);
                mylist.push(2);
                mylist.push(3);

                mylist.pop();
                mylist.pop();

                return mylist.length;
            }

            // Copy a memory array into storage using push
            function mem_to_storage() public returns (uint64) {
                uint64[] memory tmp = new uint64[](3);
                tmp[0] = 1;
                tmp[1] = 2;
                tmp[2] = 3;

                for (uint64 i = 0; i < tmp.length; i++) {
                    mylist.push(tmp[i]);
                }

                uint64 sum = 0;
                for (uint64 i = 0; i < mylist.length; i++) {
                    sum += mylist[i];
                }
                return sum; // 1+2+3 = 6
            }

            // Copy a storage array into memory and sum
            function storage_to_mem() public returns (uint64) {
                mylist.push(7);
                mylist.push(9);
                mylist.push(11);

                uint64[] memory tmp = new uint64[](mylist.length);
                for (uint64 i = 0; i < mylist.length; i++) {
                    tmp[i] = mylist[i];
                }

                uint64 sum = 0;
                for (uint64 i = 0; i < tmp.length; i++) {
                    sum += tmp[i];
                }
                return sum; // 7+9+11 = 27
            }
        }
    "#;

    let mut runtime = build_solidity(contract_src, |_| {});

    let addr = runtime.contracts.last().unwrap();
    let expected: Val = 20_u64.into_val(&runtime.env);
    let res = runtime.invoke_contract(addr, "push_pop", vec![]);
    println!("res: {res:?}");
    println!("expected: {expected:?}");
    assert!(expected.shallow_eq(&res));

    let addr2 = runtime.deploy_contract(contract_src);
    let expected: Val = 30_u64.into_val(&runtime.env);
    let res = runtime.invoke_contract(&addr2, "loop", vec![]);
    assert!(expected.shallow_eq(&res));

    let addr3 = runtime.deploy_contract(contract_src);

    let expected: Val = 15_u64.into_val(&runtime.env);
    let args = vec![0_u64.into_val(&runtime.env)];
    let res = runtime.invoke_contract(&addr3, "random_access", args);
    assert!(expected.shallow_eq(&res));

    let expected: Val = 25_u64.into_val(&runtime.env);
    let args = vec![1_u64.into_val(&runtime.env)];
    let res = runtime.invoke_contract(&addr3, "random_access", args);
    assert!(expected.shallow_eq(&res));

    let addr4 = runtime.deploy_contract(contract_src);
    let expected: Val = 1_u64.into_val(&runtime.env);
    let res = runtime.invoke_contract(&addr4, "pop_len", vec![]);
    assert!(expected.shallow_eq(&res));

    let addr5 = runtime.deploy_contract(contract_src);
    let expected: Val = 6_u64.into_val(&runtime.env);
    let res = runtime.invoke_contract(&addr5, "mem_to_storage", vec![]);
    assert!(expected.shallow_eq(&res));

    let addr6 = runtime.deploy_contract(contract_src);
    let expected: Val = 27_u64.into_val(&runtime.env);
    let res = runtime.invoke_contract(&addr6, "storage_to_mem", vec![]);
    assert!(expected.shallow_eq(&res));
}

#[test]
fn storage_array_of_structs_test() {
    let contract_src = r#"
        contract storage_struct_vec {
            struct Pair {
                uint64 a;
                uint64 b;
            }

            Pair[] items;

            function push_pair_len() public returns (uint64) {
                Pair memory p1 = Pair({a: 1, b: 2});
                Pair memory p2 = Pair({a: 3, b: 4});
                items.push(p1);
                items.push(p2);
                return items.length; // 2
            }

            function write_then_read() public returns (uint64) {
                items.push(); // append empty slot
                items[0] = Pair({a: 9, b: 11});
                return items[0].a + items[0].b; // 20
            }

            function iter_sum() public returns (uint64) {
                items.push(Pair({a: 1, b: 2}));
                items.push(Pair({a: 3, b: 4}));
                items.push(Pair({a: 5, b: 6}));
                uint64 s = 0;
                for (uint64 i = 0; i < items.length; i++) {
                    s += items[i].a + items[i].b;
                }
                return s; // (1+2)+(3+4)+(5+6) = 21
            }
        }
    "#;

    let mut runtime = build_solidity(contract_src, |_| {});

    let addr1 = runtime.contracts.last().unwrap();
    let expected: Val = 2_u64.into_val(&runtime.env);
    let res = runtime.invoke_contract(addr1, "push_pair_len", vec![]);
    assert!(expected.shallow_eq(&res));

    let addr2 = runtime.deploy_contract(contract_src);
    let expected: Val = 20_u64.into_val(&runtime.env);
    let res = runtime.invoke_contract(&addr2, "write_then_read", vec![]);
    assert!(expected.shallow_eq(&res));

    let addr3 = runtime.deploy_contract(contract_src);
    let expected: Val = 21_u64.into_val(&runtime.env);
    let res = runtime.invoke_contract(&addr3, "iter_sum", vec![]);
    println!("res: {res:?}");
    assert!(expected.shallow_eq(&res));
}

#[test]
fn storage_nested_array_test() {
    let contract_src = r#"
        contract nested_storage {
            struct Pair { uint64 a; uint64 b; }

            uint64[][] grid;
            Pair[][] pairs;

            function int_leaf() public returns (uint64) {
                grid.push();
                grid.push();
                grid[0].push(1);
                grid[0].push(2);
                grid[1].push(3);
                grid[1].push(4);

                grid[1][0] = 30;

                return grid[0][0] + grid[0][1] + grid[1][0] + grid[1][1];
            }

            function struct_leaf() public returns (uint64) {
                pairs.push();
                pairs.push();
                pairs[0].push(Pair(1, 2));
                pairs[0].push(Pair(3, 4));
                pairs[1].push(Pair(5, 6));

                pairs[1][0].a = 30;

                return pairs[0][0].a + pairs[0][0].b
                     + pairs[0][1].a + pairs[0][1].b
                     + pairs[1][0].a + pairs[1][0].b;
            }
        }
    "#;

    let mut runtime = build_solidity(contract_src, |_| {});

    let addr1 = runtime.contracts.last().unwrap();
    let expected: Val = 37_u64.into_val(&runtime.env);
    let res = runtime.invoke_contract(addr1, "int_leaf", vec![]);
    assert!(expected.shallow_eq(&res));

    let addr2 = runtime.deploy_contract(contract_src);
    let expected: Val = 46_u64.into_val(&runtime.env);
    let res = runtime.invoke_contract(&addr2, "struct_leaf", vec![]);
    assert!(expected.shallow_eq(&res));
}

#[test]
fn storage_nested_array_ops_test() {
    let contract_src = r#"
        contract nested_ops {
            struct Pair { uint64 a; uint64 b; }

            uint64[][] grid;
            Pair[][] pairs;

            // ---- int leaf ----
            function add_row() public { grid.push(); }                       // push outer
            function push_int(uint64 i, uint64 v) public { grid[i].push(v); } // push inner
            function pop_int(uint64 i) public { grid[i].pop(); }             // pop inner
            function set_int(uint64 i, uint64 j, uint64 v) public { grid[i][j] = v; } // write leaf
            function get_int(uint64 i, uint64 j) public returns (uint64) { return grid[i][j]; } // read leaf
            function row_len(uint64 i) public returns (uint64) { return grid[i].length; }

            // ---- struct leaf ----
            function add_prow() public { pairs.push(); }
            function push_pair(uint64 i, uint64 a, uint64 b) public { pairs[i].push(Pair(a, b)); }
            function pop_pair(uint64 i) public { pairs[i].pop(); }
            function set_pair(uint64 i, uint64 j, uint64 a, uint64 b) public { pairs[i][j] = Pair(a, b); } // whole element
            function set_pair_a(uint64 i, uint64 j, uint64 v) public { pairs[i][j].a = v; }               // field only
            function get_pair(uint64 i, uint64 j) public returns (uint64) { return pairs[i][j].a + pairs[i][j].b; }
            function prow_len(uint64 i) public returns (uint64) { return pairs[i].length; }
        }
    "#;

    let runtime = build_solidity(contract_src, |_| {});
    let addr = runtime.contracts.last().unwrap().clone();
    let e = runtime.env.clone();

    let u = |n: u64| -> Val { n.into_val(&e) };
    macro_rules! call {
        ($f:expr $(, $a:expr)*) => {
            runtime.invoke_contract(&addr, $f, vec![$(u($a)),*])
        };
    }

    call!("add_row");
    call!("add_row");
    call!("push_int", 0, 1);
    call!("push_int", 0, 2);
    call!("push_int", 0, 3);
    call!("push_int", 1, 10);

    call!("pop_int", 0);
    call!("set_int", 0, 1, 99);

    assert!(u(1).shallow_eq(&call!("get_int", 0, 0)));
    assert!(u(99).shallow_eq(&call!("get_int", 0, 1)));
    assert!(u(10).shallow_eq(&call!("get_int", 1, 0)));
    assert!(u(2).shallow_eq(&call!("row_len", 0)));
    assert!(u(1).shallow_eq(&call!("row_len", 1)));

    call!("add_prow");
    call!("add_prow");
    call!("push_pair", 0, 1, 2);
    call!("push_pair", 0, 3, 4);
    call!("push_pair", 1, 5, 6);

    call!("pop_pair", 0);
    call!("set_pair", 0, 0, 7, 8);
    call!("set_pair_a", 1, 0, 30);
    assert!(u(15).shallow_eq(&call!("get_pair", 0, 0)));
    assert!(u(36).shallow_eq(&call!("get_pair", 1, 0)));
    assert!(u(1).shallow_eq(&call!("prow_len", 0)));
    assert!(u(1).shallow_eq(&call!("prow_len", 1)));
}
