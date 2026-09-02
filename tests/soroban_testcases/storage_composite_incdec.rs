// SPDX-License-Identifier: Apache-2.0

use crate::build_solidity;
use soroban_sdk::{IntoVal, Val};

#[test]
fn storage_composite_incdec_test() {
    let contract_src = r#"
        contract c {
            struct S { uint64 a; uint64 b; }
            S s;
            uint64[] arr;

            function struct_member_post_inc() public returns (uint64) {
                s.b = 10;
                uint64 old = s.b++;   // old == 10, s.b becomes 11
                return old * 100 + s.b; // 10*100 + 11 = 1011
            }

            function struct_member_pre_dec() public returns (uint64) {
                s.a = 5;
                uint64 nw = --s.a;    // nw == 4, s.a becomes 4
                return nw * 100 + s.a; // 4*100 + 4 = 404
            }

            function array_elem_post_inc() public returns (uint64) {
                arr.push(7);
                arr.push(20);
                uint64 old = arr[1]++; // old == 20, arr[1] becomes 21
                return old * 100 + arr[1]; // 20*100 + 21 = 2021
            }

            function array_elem_pre_inc() public returns (uint64) {
                arr.push(30);
                uint64 nw = ++arr[0];  // nw == 31, arr[0] becomes 31
                return nw * 100 + arr[0]; // 31*100 + 31 = 3131
            }
        }
    "#;

    let mut runtime = build_solidity(contract_src, |_| {});

    let addr = runtime.contracts.last().unwrap().clone();
    let expected: Val = 1011_u64.into_val(&runtime.env);
    let res = runtime.invoke_contract(&addr, "struct_member_post_inc", vec![]);
    assert!(expected.shallow_eq(&res));

    let addr2 = runtime.deploy_contract(contract_src);
    let expected: Val = 404_u64.into_val(&runtime.env);
    let res = runtime.invoke_contract(&addr2, "struct_member_pre_dec", vec![]);
    assert!(expected.shallow_eq(&res));

    let addr3 = runtime.deploy_contract(contract_src);
    let expected: Val = 2021_u64.into_val(&runtime.env);
    let res = runtime.invoke_contract(&addr3, "array_elem_post_inc", vec![]);
    assert!(expected.shallow_eq(&res));

    let addr4 = runtime.deploy_contract(contract_src);
    let expected: Val = 3131_u64.into_val(&runtime.env);
    let res = runtime.invoke_contract(&addr4, "array_elem_pre_inc", vec![]);
    assert!(expected.shallow_eq(&res));
}
