// SPDX-License-Identifier: Apache-2.0

use crate::build_solidity;
use soroban_sdk::{
    contracttype, testutils::Address as _, Address, Bytes, BytesN, FromVal, IntoVal, String,
    TryFromVal, Val, I256, U256,
};

#[test]
fn storage_array_push_i32_test() {
    let contract_src = r#"
        contract storage_array_i32 {
            int32[] mylist;

            function push_sum() public returns (int32) {
                mylist.push(5);
                mylist.push(10);
                mylist.push(15);
                return mylist[0] + mylist[1] + mylist[2];
            }
        }
    "#;

    let runtime = build_solidity(contract_src, |_| {});

    let addr = runtime.contracts.last().unwrap();
    let expected: Val = 30_i32.into_val(&runtime.env);
    let res = runtime.invoke_contract(addr, "push_sum", vec![]);
    assert!(expected.shallow_eq(&res));
}

#[test]
fn storage_array_pop_i32_test() {
    let contract_src = r#"
        contract storage_array_pop_i32 {
            int32[] mylist;

            function pop_len() public returns (uint32) {
                mylist.push(5);
                mylist.push(10);
                mylist.push(15);
                mylist.pop();
                return uint32(mylist.length);
            }

            function pop_values() public returns (int32) {
                mylist.push(5);
                mylist.push(10);
                mylist.push(15);
                mylist.pop();
                return mylist[0] + mylist[1];
            }
        }
    "#;

    let mut runtime = build_solidity(contract_src, |_| {});

    let addr = runtime.contracts.last().unwrap();
    let expected: Val = 2_u32.into_val(&runtime.env);
    let res = runtime.invoke_contract(addr, "pop_len", vec![]);
    assert!(expected.shallow_eq(&res));

    let addr2 = runtime.deploy_contract(contract_src);
    let expected: Val = 15_i32.into_val(&runtime.env);
    let res = runtime.invoke_contract(&addr2, "pop_values", vec![]);
    assert!(expected.shallow_eq(&res));
}

#[test]
fn storage_array_length_i32_test() {
    let contract_src = r#"
        contract storage_array_len_i32 {
            int32[] mylist;

            function len_after_push() public returns (uint32) {
                mylist.push(5);
                mylist.push(10);
                mylist.push(15);
                return uint32(mylist.length); 
            }

            function len_empty() public returns (uint32) {
                return uint32(mylist.length);
            }

            function subscript_sum() public returns (int32) {
                mylist.push(5);
                mylist.push(10);
                mylist.push(15);
                return mylist[0] + mylist[1] + mylist[2];
            }
        }
    "#;

    let mut runtime = build_solidity(contract_src, |_| {});

    let addr = runtime.contracts.last().unwrap();
    let expected: Val = 3_u32.into_val(&runtime.env);
    let res = runtime.invoke_contract(addr, "len_after_push", vec![]);
    assert!(expected.shallow_eq(&res));

    let addr2 = runtime.deploy_contract(contract_src);
    let expected: Val = 0_u32.into_val(&runtime.env);
    let res = runtime.invoke_contract(&addr2, "len_empty", vec![]);
    assert!(expected.shallow_eq(&res));

    let addr3 = runtime.deploy_contract(contract_src);
    let expected: Val = 30_i32.into_val(&runtime.env);
    let res = runtime.invoke_contract(&addr3, "subscript_sum", vec![]);
    assert!(expected.shallow_eq(&res));
}

#[test]
fn storage_array_subscript_read_i32_test() {
    let contract_src = r#"
        contract storage_array_read_i32 {
            int32[] mylist;

            function read_at(uint32 i) public returns (int32) {
                mylist.push(5);
                mylist.push(10);
                mylist.push(15);
                return mylist[i];
            }
        }
    "#;

    let mut runtime = build_solidity(contract_src, |_| {});

    let addr = runtime.contracts.last().unwrap();
    let expected: Val = 5_i32.into_val(&runtime.env);
    let args = vec![0_u32.into_val(&runtime.env)];
    let res = runtime.invoke_contract(addr, "read_at", args);
    assert!(expected.shallow_eq(&res));

    let addr2 = runtime.deploy_contract(contract_src);
    let expected: Val = 15_i32.into_val(&runtime.env);
    let args = vec![2_u32.into_val(&runtime.env)];
    let res = runtime.invoke_contract(&addr2, "read_at", args);
    assert!(expected.shallow_eq(&res));
}

#[test]
fn storage_array_subscript_write_i32_test() {
    let contract_src = r#"
        contract storage_array_write_i32 {
            int32[] mylist;

            function build_and_overwrite() public returns (int32) {
                mylist.push(5);
                mylist.push(10);
                mylist.push(15);
                mylist[1] = 100;
                return mylist[0] + mylist[1] + mylist[2];
            }

            function write_at(uint32 i, int32 v) public returns (int32) {
                mylist.push(1);
                mylist.push(2);
                mylist.push(3);
                mylist[i] = v;
                return mylist[i];
            }
        }
    "#;

    let mut runtime = build_solidity(contract_src, |_| {});

    let addr = runtime.contracts.last().unwrap();
    let expected: Val = 120_i32.into_val(&runtime.env);
    let res = runtime.invoke_contract(addr, "build_and_overwrite", vec![]);
    assert!(expected.shallow_eq(&res));

    let addr2 = runtime.deploy_contract(contract_src);
    let expected: Val = 42_i32.into_val(&runtime.env);
    let args = vec![2_u32.into_val(&runtime.env), 42_i32.into_val(&runtime.env)];
    let res = runtime.invoke_contract(&addr2, "write_at", args);
    assert!(expected.shallow_eq(&res));
}

#[test]
fn storage_array_bool_test() {
    let contract_src = r#"
        contract storage_array_bool {
            bool[] mylist;

            function push_read() public returns (bool) {
                mylist.push(true);
                mylist.push(false);
                mylist.push(true);
                return mylist[0] && !mylist[1] && mylist[2];
            }

            function len_after_push() public returns (uint32) {
                mylist.push(true);
                mylist.push(false);
                return uint32(mylist.length);
            }

            function write_read() public returns (bool) {
                mylist.push(false);
                mylist.push(false);
                mylist[1] = true;
                return mylist[1];
            }

            function pop_len() public returns (uint32) {
                mylist.push(true);
                mylist.push(false);
                mylist.push(true);
                mylist.pop();
                return uint32(mylist.length);
            }
        }
    "#;

    let mut runtime = build_solidity(contract_src, |_| {});

    let addr = runtime.contracts.last().unwrap();
    let expected: Val = true.into_val(&runtime.env);
    let res = runtime.invoke_contract(addr, "push_read", vec![]);
    assert!(expected.shallow_eq(&res));

    let addr2 = runtime.deploy_contract(contract_src);
    let expected: Val = 2_u32.into_val(&runtime.env);
    let res = runtime.invoke_contract(&addr2, "len_after_push", vec![]);
    assert!(expected.shallow_eq(&res));

    let addr3 = runtime.deploy_contract(contract_src);
    let expected: Val = true.into_val(&runtime.env);
    let res = runtime.invoke_contract(&addr3, "write_read", vec![]);
    assert!(expected.shallow_eq(&res));

    let addr4 = runtime.deploy_contract(contract_src);
    let expected: Val = 2_u32.into_val(&runtime.env);
    let res = runtime.invoke_contract(&addr4, "pop_len", vec![]);
    assert!(expected.shallow_eq(&res));
}

#[test]
fn storage_array_i32_test() {
    let contract_src = r#"
        contract storage_array_i32_full {
            int32[] mylist;

            // Build [-5,-3,-1,1,3,5], mutate via subscripts, pop twice, loop-sum.
            function churn() public returns (int32) {
                for (int32 i = 0; i < 6; i++) {
                    mylist.push(i * 2 - 5); // -5,-3,-1,1,3,5
                }
                // read-driven writes (binary ops)
                mylist[0] = mylist[5] + mylist[1]; // 5 + (-3) = 2 -> [2,-3,-1,1,3,5]
                mylist[4] = mylist[0] * mylist[3]; // 2 * 1   = 2 -> [2,-3,-1,1,2,5]

                mylist.pop(); // drop 5 -> [2,-3,-1,1,2]
                mylist.pop(); // drop 2 -> [2,-3,-1,1], length 4

                int32 sum = 0;
                for (uint32 j = 0; j < mylist.length; j++) {
                    sum += mylist[j]; // 2 + (-3) + (-1) + 1 = -1
                }
                return sum * 100 + int32(uint32(mylist.length)); // -1*100 + 4 = -96
            }

            // Push 1..n, swap first and last via a temp, return their difference.
            function swap_ends(uint32 n) public returns (int32) {
                for (uint32 k = 0; k < n; k++) {
                    mylist.push(int32(k) + 1); // 1,2,...,n
                }
                uint32 last = uint32(mylist.length) - 1;
                int32 tmp = mylist[0];
                mylist[0] = mylist[last];
                mylist[last] = tmp;
                return mylist[0] - mylist[last]; // n - 1
            }
        }
    "#;

    let mut runtime = build_solidity(contract_src, |_| {});

    let addr = runtime.contracts.last().unwrap();
    let expected: Val = (-96_i32).into_val(&runtime.env);
    let res = runtime.invoke_contract(addr, "churn", vec![]);
    assert!(expected.shallow_eq(&res));

    let addr2 = runtime.deploy_contract(contract_src);
    let expected: Val = 3_i32.into_val(&runtime.env);
    let args = vec![4_u32.into_val(&runtime.env)];
    let res = runtime.invoke_contract(&addr2, "swap_ends", args);
    assert!(expected.shallow_eq(&res));
}

#[test]
fn storage_array_u32_test() {
    let contract_src = r#"
        contract storage_array_u32_full {
            uint32[] mylist;

            function churn() public returns (uint32) {
                for (uint32 i = 0; i < 5; i++) {
                    mylist.push((i + 1) * 10); // 10,20,30,40,50
                }
                mylist[0] = mylist[4] + mylist[1]; // 50 + 20 = 70 -> [70,20,30,40,50]
                mylist[2] = mylist[2] * 3;         // 30 * 3  = 90 -> [70,20,90,40,50]

                mylist.pop(); // drop 50 -> [70,20,90,40], length 4

                uint32 sum = 0;
                for (uint32 j = 0; j < mylist.length; j++) {
                    sum += mylist[j]; // 70 + 20 + 90 + 40 = 220
                }
                return sum + uint32(mylist.length); // 220 + 4 = 224
            }

            function set_get(uint32 i, uint32 v) public returns (uint32) {
                for (uint32 k = 0; k < 4; k++) {
                    mylist.push(k); // 0,1,2,3
                }
                mylist[i] = v;
                return mylist[i] + uint32(mylist.length);
            }
        }
    "#;

    let mut runtime = build_solidity(contract_src, |_| {});

    let addr = runtime.contracts.last().unwrap();
    let expected: Val = 224_u32.into_val(&runtime.env);
    let res = runtime.invoke_contract(addr, "churn", vec![]);
    assert!(expected.shallow_eq(&res));

    let addr2 = runtime.deploy_contract(contract_src);
    let expected: Val = 103_u32.into_val(&runtime.env);
    let args = vec![2_u32.into_val(&runtime.env), 99_u32.into_val(&runtime.env)];
    let res = runtime.invoke_contract(&addr2, "set_get", args);
    assert!(expected.shallow_eq(&res));
}

#[test]
fn storage_array_i64_test() {
    let contract_src = r#"
        contract storage_array_i64_full {
            int64[] mylist;

            function churn() public returns (int64) {
                mylist.push(1_000_000_000_000); // 1e12
                mylist.push(-500_000_000_000);  // -5e11
                mylist.push(3);
                mylist.push(-7);
                // [1e12, -5e11, 3, -7]
                mylist[2] = mylist[0] + mylist[1]; // 1e12 + (-5e11) = 5e11
                mylist[3] = mylist[3] * 100;       // -7 * 100 = -700
                // [1e12, -5e11, 5e11, -700]

                mylist.pop(); // drop -700 -> [1e12, -5e11, 5e11], length 3

                int64 sum = 0;
                for (uint32 j = 0; j < mylist.length; j++) {
                    sum += mylist[j]; // 1e12 - 5e11 + 5e11 = 1e12
                }
                return sum + int64(uint32(mylist.length)); // 1e12 + 3
            }

            function set_get(uint32 i, int64 v) public returns (int64) {
                mylist.push(10);
                mylist.push(20);
                mylist.push(30);
                mylist[i] = v;
                return mylist[i];
            }
        }
    "#;

    let mut runtime = build_solidity(contract_src, |_| {});

    let addr = runtime.contracts.last().unwrap();
    let expected: Val = 1_000_000_000_003_i64.into_val(&runtime.env);
    let res = runtime.invoke_contract(addr, "churn", vec![]);
    assert!(expected.shallow_eq(&res));

    let addr2 = runtime.deploy_contract(contract_src);
    let expected: Val = (-999_i64).into_val(&runtime.env);
    let args = vec![
        1_u32.into_val(&runtime.env),
        (-999_i64).into_val(&runtime.env),
    ];
    let res = runtime.invoke_contract(&addr2, "set_get", args);
    assert!(expected.shallow_eq(&res));
}

#[test]
fn storage_array_u64_test() {
    let contract_src = r#"
        contract storage_array_u64_full {
            uint64[] mylist;

            function churn() public returns (uint64) {
                mylist.push(10_000_000_000); // 1e10
                mylist.push(20_000_000_000); // 2e10
                mylist.push(5);
                mylist.push(7);
                // [1e10, 2e10, 5, 7]
                mylist[2] = mylist[0] + mylist[1]; // 3e10
                mylist[3] = mylist[1] * 2;         // 4e10
                // [1e10, 2e10, 3e10, 4e10]

                mylist.pop(); // drop 4e10 -> [1e10, 2e10, 3e10], length 3

                uint64 sum = 0;
                for (uint32 j = 0; j < mylist.length; j++) {
                    sum += mylist[j]; // 1e10 + 2e10 + 3e10 = 6e10
                }
                return sum + uint64(mylist.length); // 6e10 + 3
            }

            function set_get(uint32 i, uint64 v) public returns (uint64) {
                mylist.push(100);
                mylist.push(200);
                mylist.push(300);
                mylist[i] = v;
                return mylist[i];
            }
        }
    "#;

    let mut runtime = build_solidity(contract_src, |_| {});

    let addr = runtime.contracts.last().unwrap();
    let expected: Val = 60_000_000_003_u64.into_val(&runtime.env);
    let res = runtime.invoke_contract(addr, "churn", vec![]);
    assert!(expected.shallow_eq(&res));

    let addr2 = runtime.deploy_contract(contract_src);
    let expected: Val = 999_888_777_666_u64.into_val(&runtime.env);
    let args = vec![
        0_u32.into_val(&runtime.env),
        999_888_777_666_u64.into_val(&runtime.env),
    ];
    let res = runtime.invoke_contract(&addr2, "set_get", args);
    assert!(expected.shallow_eq(&res));
}

const MODEL_SRC: &str = r#"
    contract storage_array_model {
        TYPE[] mylist;

        function push(TYPE v) public returns (uint32) {
            mylist.push(v);
            return uint32(mylist.length);
        }

        function pop() public returns (uint32) {
            mylist.pop();
            return uint32(mylist.length);
        }

        function set(uint32 i, TYPE v) public returns (TYPE) {
            mylist[i] = v;
            return mylist[i];
        }

        function get(uint32 i) public returns (TYPE) {
            return mylist[i];
        }

        function length() public returns (uint32) {
            return uint32(mylist.length);
        }

        // Count elements within the inclusive range [lo, hi] — a full read scan.
        function check_in_range(TYPE lo, TYPE hi) public returns (uint32) {
            uint32 c = 0;
            for (uint32 i = 0; i < mylist.length; i++) {
                if (mylist[i] >= lo && mylist[i] <= hi) {
                    c++;
                }
            }
            return c;
        }
    }
"#;

fn drive_vec_model<S>(
    src: &str,
    values: &[S],
    set_val: S,
    lo: S,
    hi: S,
    to_val: impl Fn(&soroban_sdk::Env, S) -> Val,
    ret_eq: impl Fn(&soroban_sdk::Env, &Val, S) -> bool,
) where
    S: Copy + Ord,
{
    let runtime = build_solidity(src, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let mut shadow: Vec<S> = Vec::new();

    let verify = |shadow: &Vec<S>| {
        let len_exp: Val = (shadow.len() as u32).into_val(&runtime.env);
        let len_got = runtime.invoke_contract(addr, "length", vec![]);
        assert!(len_exp.shallow_eq(&len_got), "length mismatch");

        for (i, v) in shadow.iter().enumerate() {
            let got = runtime.invoke_contract(addr, "get", vec![(i as u32).into_val(&runtime.env)]);
            assert!(ret_eq(&runtime.env, &got, *v), "element {i} mismatch");
        }

        let count = shadow.iter().filter(|&&e| e >= lo && e <= hi).count() as u32;
        let cexp: Val = count.into_val(&runtime.env);
        let cgot = runtime.invoke_contract(
            addr,
            "check_in_range",
            vec![to_val(&runtime.env, lo), to_val(&runtime.env, hi)],
        );
        assert!(cexp.shallow_eq(&cgot), "check_in_range mismatch");
    };

    for v in values {
        runtime.invoke_contract(addr, "push", vec![to_val(&runtime.env, *v)]);
        shadow.push(*v);
        verify(&shadow);
    }

    let n = shadow.len();
    for i in (0..n).step_by(2) {
        runtime.invoke_contract(
            addr,
            "set",
            vec![
                (i as u32).into_val(&runtime.env),
                to_val(&runtime.env, set_val),
            ],
        );
        shadow[i] = set_val;
        verify(&shadow);
    }

    while shadow.len() > n / 2 {
        runtime.invoke_contract(addr, "pop", vec![]);
        shadow.pop();
        verify(&shadow);
    }

    for v in values.iter().take(2) {
        runtime.invoke_contract(addr, "push", vec![to_val(&runtime.env, *v)]);
        shadow.push(*v);
        verify(&shadow);
    }
}

#[test]
fn storage_array_i128_model_test() {
    let src = MODEL_SRC.replace("TYPE", "int128");
    let values: [i128; 6] = [
        1_000_000_000_000_000_000_000,
        -500_000_000_000_000_000_000,
        7,
        -3,
        42,
        0,
    ];
    drive_vec_model::<i128>(
        &src,
        &values,
        5,
        -3,
        42,
        |env, s| s.into_val(env),
        |env, val, s| {
            i128::try_from_val(env, val)
                .map(|d| d == s)
                .unwrap_or(false)
        },
    );
}

#[test]
fn storage_array_u128_model_test() {
    let src = MODEL_SRC.replace("TYPE", "uint128");
    let values: [u128; 6] = [
        1_000_000_000_000_000_000_000,
        2_000_000_000_000_000_000_000,
        7,
        3,
        42,
        0,
    ];
    drive_vec_model::<u128>(
        &src,
        &values,
        5,
        0,
        42,
        |env, s| s.into_val(env),
        |env, val, s| {
            u128::try_from_val(env, val)
                .map(|d| d == s)
                .unwrap_or(false)
        },
    );
}

#[test]
fn storage_array_i256_model_test() {
    let src = MODEL_SRC.replace("TYPE", "int256");
    let values: [i128; 6] = [2i128.pow(90), -(2i128.pow(80)), 7, -3, 42, 0];
    drive_vec_model::<i128>(
        &src,
        &values,
        5,
        -3,
        42,
        |env, s| I256::from_i128(env, s).into_val(env),
        |env, val, s| I256::from_val(env, val) == I256::from_i128(env, s),
    );
}

#[test]
fn storage_array_u256_model_test() {
    let src = MODEL_SRC.replace("TYPE", "uint256");
    let values: [u128; 6] = [2u128.pow(100), 2u128.pow(90), 7, 3, 42, 0];
    drive_vec_model::<u128>(
        &src,
        &values,
        5,
        0,
        42,
        |env, s| U256::from_u128(env, s).into_val(env),
        |env, val, s| U256::from_val(env, val) == U256::from_u128(env, s),
    );
}

const REF_MODEL_SRC: &str = r#"
    contract storage_array_ref_model {
        TYPE[] mylist;

        function push(TYPE memory v) public returns (uint32) {
            mylist.push(v);
            return uint32(mylist.length);
        }

        function pop() public returns (uint32) {
            mylist.pop();
            return uint32(mylist.length);
        }

        function set(uint32 i, TYPE memory v) public returns (TYPE memory) {
            mylist[i] = v;
            return mylist[i];
        }

        function get(uint32 i) public returns (TYPE memory) {
            return mylist[i];
        }

        function length() public returns (uint32) {
            return uint32(mylist.length);
        }
    }
"#;

fn drive_vec_ref_model<Spec: Clone>(
    src: &str,
    values: &[Spec],
    set_val: Spec,
    to_val: impl Fn(&soroban_sdk::Env, &Spec) -> Val,
    eq: impl Fn(&soroban_sdk::Env, &Val, &Spec) -> bool,
) {
    let runtime = build_solidity(src, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;
    let mut shadow: Vec<Spec> = Vec::new();

    let verify = |shadow: &Vec<Spec>| {
        let len_exp: Val = (shadow.len() as u32).into_val(env);
        let len_got = runtime.invoke_contract(addr, "length", vec![]);
        assert!(len_exp.shallow_eq(&len_got), "length mismatch");

        for (i, v) in shadow.iter().enumerate() {
            let got = runtime.invoke_contract(addr, "get", vec![(i as u32).into_val(env)]);
            assert!(eq(env, &got, v), "element {i} mismatch");
        }
    };

    for v in values {
        runtime.invoke_contract(addr, "push", vec![to_val(env, v)]);
        shadow.push(v.clone());
        verify(&shadow);
    }

    let n = shadow.len();
    for i in (0..n).step_by(2) {
        runtime.invoke_contract(
            addr,
            "set",
            vec![(i as u32).into_val(env), to_val(env, &set_val)],
        );
        shadow[i] = set_val.clone();
        verify(&shadow);
    }

    while shadow.len() > n / 2 {
        runtime.invoke_contract(addr, "pop", vec![]);
        shadow.pop();
        verify(&shadow);
    }

    for v in values.iter().take(2) {
        runtime.invoke_contract(addr, "push", vec![to_val(env, v)]);
        shadow.push(v.clone());
        verify(&shadow);
    }
}

#[test]
fn storage_array_string_test() {
    let src = REF_MODEL_SRC.replace("TYPE", "string");
    let values: [&str; 5] = ["hello", "", "solang world", "x", "souka"];
    drive_vec_ref_model::<&str>(
        &src,
        &values,
        "replaced",
        |env, s| String::from_str(env, s).into_val(env),
        |env, val, s| String::from_val(env, val) == String::from_str(env, s),
    );
}

#[test]
fn storage_array_bytes_test() {
    let src = REF_MODEL_SRC.replace("TYPE", "bytes");
    let values: [&[u8]; 5] = [
        &[0xAA, 0xBB, 0xCC],
        &[],
        &[1, 2, 3, 4, 5, 6, 7, 8],
        &[0xFF],
        &[0x00, 0x10, 0x20],
    ];
    drive_vec_ref_model::<&[u8]>(
        &src,
        &values,
        &[0xDE, 0xAD, 0xBE, 0xEF],
        |env, b| Bytes::from_slice(env, b).into_val(env),
        |env, val, b| Bytes::from_val(env, val) == Bytes::from_slice(env, b),
    );
}

const BYTESN_MODEL_SRC: &str = r#"
    contract storage_array_bytesn_model {
        TYPE[] mylist;

        function push(TYPE v) public returns (uint32) {
            mylist.push(v);
            return uint32(mylist.length);
        }

        function pop() public returns (uint32) {
            mylist.pop();
            return uint32(mylist.length);
        }

        function set(uint32 i, TYPE v) public returns (TYPE) {
            mylist[i] = v;
            return mylist[i];
        }

        function get(uint32 i) public returns (TYPE) {
            return mylist[i];
        }

        function length() public returns (uint32) {
            return uint32(mylist.length);
        }
    }
"#;

#[test]
fn storage_array_bytes1_test() {
    let src = BYTESN_MODEL_SRC.replace("TYPE", "bytes1");
    let values: [[u8; 1]; 5] = [[0xAA], [0x00], [0xFF], [0x01], [0x7F]];
    drive_vec_ref_model::<[u8; 1]>(
        &src,
        &values,
        [0x42],
        |env, b| BytesN::from_array(env, b).into_val(env),
        |env, val, b| BytesN::<1>::from_val(env, val) == BytesN::from_array(env, b),
    );
}

#[test]
fn storage_array_bytes16_test() {
    let src = BYTESN_MODEL_SRC.replace("TYPE", "bytes16");
    let values: [[u8; 16]; 4] = [
        [0x11; 16],
        [0x00; 16],
        [0xFF; 16],
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    ];
    drive_vec_ref_model::<[u8; 16]>(
        &src,
        &values,
        [0xAB; 16],
        |env, b| BytesN::from_array(env, b).into_val(env),
        |env, val, b| BytesN::<16>::from_val(env, val) == BytesN::from_array(env, b),
    );
}

#[test]
fn storage_array_bytes32_test() {
    let src = BYTESN_MODEL_SRC.replace("TYPE", "bytes32");
    let mut ramp = [0u8; 32];
    for (i, b) in ramp.iter_mut().enumerate() {
        *b = i as u8;
    }
    let values: [[u8; 32]; 4] = [[0x22; 32], [0x00; 32], [0xFF; 32], ramp];
    drive_vec_ref_model::<[u8; 32]>(
        &src,
        &values,
        [0xCD; 32],
        |env, b| BytesN::from_array(env, b).into_val(env),
        |env, val, b| BytesN::<32>::from_val(env, val) == BytesN::from_array(env, b),
    );
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct ArrScalar {
    pub a: u64,
    pub b: i32,
    pub c: bool,
}

#[test]
fn storage_array_struct_scalar_test() {
    let src = r#"
        contract storage_array_struct_scalar {
            struct S { uint64 a; int32 b; bool c; }
            S[] mylist;

            function push(S memory v) public returns (uint32) {
                mylist.push(v);
                return uint32(mylist.length);
            }
            function pop() public returns (uint32) {
                mylist.pop();
                return uint32(mylist.length);
            }
            function set(uint32 i, S memory v) public returns (S memory) {
                mylist[i] = v;
                return mylist[i];
            }
            function get(uint32 i) public returns (S memory) {
                return mylist[i];
            }
            function length() public returns (uint32) {
                return uint32(mylist.length);
            }
        }
    "#;

    let values: [ArrScalar; 4] = [
        ArrScalar {
            a: 1_000_000_000_000,
            b: -5,
            c: true,
        },
        ArrScalar {
            a: 0,
            b: 0,
            c: false,
        },
        ArrScalar {
            a: u64::MAX,
            b: i32::MIN,
            c: true,
        },
        ArrScalar {
            a: 42,
            b: 7,
            c: false,
        },
    ];
    drive_vec_ref_model::<ArrScalar>(
        src,
        &values,
        ArrScalar {
            a: 99,
            b: -1,
            c: true,
        },
        |env, r| r.clone().into_val(env),
        |env, val, r| ArrScalar::from_val(env, val) == *r,
    );
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct ArrDoc {
    pub name: String,
    pub data: Bytes,
    pub tag: BytesN<4>,
    pub id: u32,
}

type DocSpec = (&'static str, &'static [u8], [u8; 4], u32);

fn make_doc(env: &soroban_sdk::Env, s: &DocSpec) -> ArrDoc {
    ArrDoc {
        name: String::from_str(env, s.0),
        data: Bytes::from_slice(env, s.1),
        tag: BytesN::from_array(env, &s.2),
        id: s.3,
    }
}

#[test]
fn storage_array_struct_ref_test() {
    let src = r#"
        contract storage_array_struct_ref {
            struct S { string name; bytes data; bytes4 tag; uint32 id; }
            S[] mylist;

            function push(S memory v) public returns (uint32) {
                mylist.push(v);
                return uint32(mylist.length);
            }
            function pop() public returns (uint32) {
                mylist.pop();
                return uint32(mylist.length);
            }
            function set(uint32 i, S memory v) public returns (S memory) {
                mylist[i] = v;
                return mylist[i];
            }
            function get(uint32 i) public returns (S memory) {
                return mylist[i];
            }
            function length() public returns (uint32) {
                return uint32(mylist.length);
            }
        }
    "#;

    let values: [DocSpec; 4] = [
        ("hello", &[0xAA, 0xBB], [0x01, 0x02, 0x03, 0x04], 7),
        ("", &[], [0, 0, 0, 0], 0),
        (
            "solang world",
            &[1, 2, 3, 4, 5],
            [0xFF, 0xFF, 0xFF, 0xFF],
            u32::MAX,
        ),
        ("x", &[0x10], [0xDE, 0xAD, 0xBE, 0xEF], 42),
    ];
    drive_vec_ref_model::<DocSpec>(
        src,
        &values,
        ("replaced", &[0x99, 0x88], [0xCA, 0xFE, 0xBA, 0xBE], 123),
        |env, s| make_doc(env, s).into_val(env),
        |env, val, s| ArrDoc::from_val(env, val) == make_doc(env, s),
    );
}

#[test]
fn storage_array_cross_tx_scalar_test() {
    let contract_src = r#"
        contract storage_array_persist_scalar {
            int32[] mylist;

            function push_val(int32 v) public { mylist.push(v); }
            function pop() public { mylist.pop(); }
            function set(uint32 i, int32 v) public { mylist[i] = v; }
            function get(uint32 i) public returns (int32) { return mylist[i]; }
            function length() public returns (uint32) { return uint32(mylist.length); }
        }
    "#;

    let runtime = build_solidity(contract_src, |_| {});
    let addr = runtime.contracts.last().unwrap();

    runtime.invoke_contract(addr, "push_val", vec![10_i32.into_val(&runtime.env)]);
    let res = runtime.invoke_contract(addr, "length", vec![]);
    let exp: Val = 1_u32.into_val(&runtime.env);
    assert!(exp.shallow_eq(&res));

    runtime.invoke_contract(addr, "push_val", vec![20_i32.into_val(&runtime.env)]);
    runtime.invoke_contract(addr, "push_val", vec![30_i32.into_val(&runtime.env)]);
    let r0 = runtime.invoke_contract(addr, "get", vec![0_u32.into_val(&runtime.env)]);
    let r1 = runtime.invoke_contract(addr, "get", vec![1_u32.into_val(&runtime.env)]);
    let r2 = runtime.invoke_contract(addr, "get", vec![2_u32.into_val(&runtime.env)]);
    let e10: Val = 10_i32.into_val(&runtime.env);
    let e20: Val = 20_i32.into_val(&runtime.env);
    let e30: Val = 30_i32.into_val(&runtime.env);
    assert!(e10.shallow_eq(&r0));
    assert!(e20.shallow_eq(&r1));
    assert!(e30.shallow_eq(&r2));

    runtime.invoke_contract(
        addr,
        "set",
        vec![1_u32.into_val(&runtime.env), 99_i32.into_val(&runtime.env)],
    );
    let got = runtime.invoke_contract(addr, "get", vec![1_u32.into_val(&runtime.env)]);
    let e99: Val = 99_i32.into_val(&runtime.env);
    assert!(e99.shallow_eq(&got));

    runtime.invoke_contract(addr, "pop", vec![]);
    let len2: Val = runtime.invoke_contract(addr, "length", vec![]);
    let exp2: Val = 2_u32.into_val(&runtime.env);
    assert!(exp2.shallow_eq(&len2));
    let a0 = runtime.invoke_contract(addr, "get", vec![0_u32.into_val(&runtime.env)]);
    let a1 = runtime.invoke_contract(addr, "get", vec![1_u32.into_val(&runtime.env)]);
    assert!(e10.shallow_eq(&a0));
    assert!(e99.shallow_eq(&a1));
}

#[test]
fn storage_array_cross_tx_string_test() {
    let contract_src = r#"
        contract storage_array_persist_string {
            string[] mylist;

            function push_val(string memory v) public { mylist.push(v); }
            function pop() public { mylist.pop(); }
            function set(uint32 i, string memory v) public { mylist[i] = v; }
            function get(uint32 i) public returns (string memory) { return mylist[i]; }
            function length() public returns (uint32) { return uint32(mylist.length); }
        }
    "#;

    let runtime = build_solidity(contract_src, |_| {});
    let addr = runtime.contracts.last().unwrap();

    let mk = |s: &str| -> Val { String::from_str(&runtime.env, s).into_val(&runtime.env) };
    let eq_str = |val: &Val, s: &str| {
        String::from_val(&runtime.env, val) == String::from_str(&runtime.env, s)
    };

    runtime.invoke_contract(addr, "push_val", vec![mk("hello")]);
    let len: Val = runtime.invoke_contract(addr, "length", vec![]);
    let exp1: Val = 1_u32.into_val(&runtime.env);
    assert!(exp1.shallow_eq(&len));
    let got = runtime.invoke_contract(addr, "get", vec![0_u32.into_val(&runtime.env)]);
    assert!(eq_str(&got, "hello"));

    runtime.invoke_contract(addr, "push_val", vec![mk("")]);
    runtime.invoke_contract(addr, "push_val", vec![mk("soroban")]);
    let g1 = runtime.invoke_contract(addr, "get", vec![1_u32.into_val(&runtime.env)]);
    let g2 = runtime.invoke_contract(addr, "get", vec![2_u32.into_val(&runtime.env)]);
    assert!(eq_str(&g1, ""));
    assert!(eq_str(&g2, "soroban"));

    runtime.invoke_contract(
        addr,
        "set",
        vec![0_u32.into_val(&runtime.env), mk("replaced")],
    );
    let r = runtime.invoke_contract(addr, "get", vec![0_u32.into_val(&runtime.env)]);
    assert!(eq_str(&r, "replaced"));

    runtime.invoke_contract(addr, "pop", vec![]);
    let len2: Val = runtime.invoke_contract(addr, "length", vec![]);
    let exp2: Val = 2_u32.into_val(&runtime.env);
    assert!(exp2.shallow_eq(&len2));
    let s0 = runtime.invoke_contract(addr, "get", vec![0_u32.into_val(&runtime.env)]);
    let s1 = runtime.invoke_contract(addr, "get", vec![1_u32.into_val(&runtime.env)]);
    assert!(eq_str(&s0, "replaced"));
    assert!(eq_str(&s1, ""));
}

#[test]
fn storage_array_address_test() {
    let contract_src = r#"
        contract storage_array_address {
            address[] mylist;

            function push_val(address v) public { mylist.push(v); }
            function pop() public { mylist.pop(); }
            function set(uint32 i, address v) public { mylist[i] = v; }
            function get(uint32 i) public returns (address) { return mylist[i]; }
            function length() public returns (uint32) { return uint32(mylist.length); }
        }
    "#;

    let runtime = build_solidity(contract_src, |_| {});
    let addr = runtime.contracts.last().unwrap();

    let a0 = Address::generate(&runtime.env);
    let a1 = Address::generate(&runtime.env);
    let a2 = Address::generate(&runtime.env);

    runtime.invoke_contract(addr, "push_val", vec![a0.clone().into_val(&runtime.env)]);
    runtime.invoke_contract(addr, "push_val", vec![a1.clone().into_val(&runtime.env)]);
    let expected: Val = 2_u32.into_val(&runtime.env);
    let res = runtime.invoke_contract(addr, "length", vec![]);
    assert!(expected.shallow_eq(&res));
    let g0 = runtime.invoke_contract(addr, "get", vec![0_u32.into_val(&runtime.env)]);
    let g1 = runtime.invoke_contract(addr, "get", vec![1_u32.into_val(&runtime.env)]);
    assert!(Address::from_val(&runtime.env, &g0) == a0);
    assert!(Address::from_val(&runtime.env, &g1) == a1);

    runtime.invoke_contract(
        addr,
        "set",
        vec![
            0_u32.into_val(&runtime.env),
            a2.clone().into_val(&runtime.env),
        ],
    );
    let got = runtime.invoke_contract(addr, "get", vec![0_u32.into_val(&runtime.env)]);
    assert!(Address::from_val(&runtime.env, &got) == a2);

    runtime.invoke_contract(addr, "pop", vec![]);
    let expected: Val = 1_u32.into_val(&runtime.env);
    let res = runtime.invoke_contract(addr, "length", vec![]);
    assert!(expected.shallow_eq(&res));
    let remaining = runtime.invoke_contract(addr, "get", vec![0_u32.into_val(&runtime.env)]);
    assert!(Address::from_val(&runtime.env, &remaining) == a2);
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct ArrPoint {
    pub x: i32,
    pub y: i32,
}

#[test]
fn storage_array_struct_member_write_test() {
    let src = r#"
        contract storage_array_struct_member_write {
            struct Point { int32 x; int32 y; }
            Point[] mylist;

            function push(Point memory v) public { mylist.push(v); }
            function get(uint32 i) public returns (Point memory) { return mylist[i]; }
            function length() public returns (uint32) { return uint32(mylist.length); }
            function set_y(uint32 i, int32 v) public { mylist[i].y = v; }
            function get_x(uint32 i) public returns (int32) { return mylist[i].x; }
            function get_y(uint32 i) public returns (int32) { return mylist[i].y; }
        }
    "#;

    let runtime = build_solidity(src, |_| {});
    let addr = runtime.contracts.last().unwrap();
    let env = &runtime.env;

    for (x, y) in [(1_i32, 2_i32), (3, 4), (5, 6)] {
        runtime.invoke_contract(addr, "push", vec![ArrPoint { x, y }.into_val(env)]);
    }

    runtime.invoke_contract(
        addr,
        "set_y",
        vec![1_u32.into_val(env), 99_i32.into_val(env)],
    );

    let got_y = runtime.invoke_contract(addr, "get_y", vec![1_u32.into_val(env)]);
    let exp_99: Val = 99_i32.into_val(env);
    assert!(exp_99.shallow_eq(&got_y), "mylist[1].y should be 99");

    let got_x = runtime.invoke_contract(addr, "get_x", vec![1_u32.into_val(env)]);
    let exp_3: Val = 3_i32.into_val(env);
    assert!(exp_3.shallow_eq(&got_x), "mylist[1].x should still be 3");

    let got_y0 = runtime.invoke_contract(addr, "get_y", vec![0_u32.into_val(env)]);
    let exp_2: Val = 2_i32.into_val(env);
    assert!(exp_2.shallow_eq(&got_y0), "mylist[0].y should still be 2");

    let got_y2 = runtime.invoke_contract(addr, "get_y", vec![2_u32.into_val(env)]);
    let exp_6: Val = 6_i32.into_val(env);
    assert!(exp_6.shallow_eq(&got_y2), "mylist[2].y should still be 6");

    let got_s1 = runtime.invoke_contract(addr, "get", vec![1_u32.into_val(env)]);
    assert!(
        ArrPoint::from_val(env, &got_s1) == ArrPoint { x: 3, y: 99 },
        "mylist[1] whole-struct mismatch after field write"
    );
}
