// SPDX-License-Identifier: Apache-2.0

use super::{
    cfg::ReturnCode, expression, Builtin, ControlFlowGraph, Expression, Instr, Options, Type,
    Vartable,
};
use crate::codegen::revert::string_to_expr;
use crate::codegen::solana_accounts::account_management::{
    account_meta_literal, retrieve_key_from_account_info,
};
use crate::sema::ast::{
    self, ArrayLength, CallTy, ExternalCallAccounts, Function, FunctionAttributes, Namespace,
    StructType,
};
use crate::sema::diagnostics::Diagnostics;
use crate::sema::eval::eval_const_number;
use crate::sema::solana_accounts::BuiltinAccounts;
use base58::ToBase58;
use num_bigint::{BigInt, Sign};
use num_traits::{ToPrimitive, Zero};
use solang_parser::pt::Loc;

// https://github.com/solana-labs/solana/blob/7beeb83104a46b9e709f24fbf94e19a2ac564e99/sdk/program/src/rent.rs#L50
const ACCOUNT_STORAGE_OVERHEAD: u64 = 128;
// https://github.com/solana-labs/solana/blob/7beeb83104a46b9e709f24fbf94e19a2ac564e99/sdk/program/src/rent.rs#L34
const LAMPORTS_PER_BYTE_YER: u64 = 3480;
// https://github.com/solana-labs/solana/blob/7beeb83104a46b9e709f24fbf94e19a2ac564e99/sdk/program/src/rent.rs#L38
const EXEMPTION_THRESHOLD: u64 = 2;

/// Special code for Solana constructors like creating the account
///
/// On Solana, prepare the data account after deploy; ensure the account is
/// large enough and write magic to it to show the account has been deployed.
pub(super) fn solana_deploy(
    func: &Function,
    constructor_args: &[Expression],
    contract_no: usize,
    vartab: &mut Vartable,
    cfg: &mut ControlFlowGraph,
    ns: &Namespace,
    opt: &Options,
) {
    let contract = &ns.contracts[contract_no];

    let program_id = contract.program_id.as_ref();

    if let Some(program_id) = program_id {
        // emit code to check program_id == program_id
        let cond = Expression::Equal {
            loc: Loc::Codegen,
            left: Box::new(Expression::NumberLiteral {
                loc: Loc::Codegen,
                ty: Type::Address(false),
                value: BigInt::from_bytes_be(Sign::Plus, program_id),
            }),
            right: Expression::Load {
                loc: Loc::Codegen,
                ty: Type::Address(false),
                expr: Box::new(Expression::Builtin {
                    loc: Loc::Codegen,
                    tys: vec![Type::Ref(Box::new(Type::Address(false)))],
                    kind: Builtin::GetAddress,
                    args: Vec::new(),
                }),
            }
            .into(),
        };

        let id_fail = cfg.new_basic_block("program_id_fail".to_string());

        let id_ok = cfg.new_basic_block("program_id_ok".to_string());

        cfg.add(
            vartab,
            Instr::BranchCond {
                cond,
                true_block: id_ok,
                false_block: id_fail,
            },
        );

        cfg.set_basic_block(id_fail);

        cfg.add(
            vartab,
            Instr::Print {
                expr: string_to_expr(format!("program_id should be {}", program_id.to_base58())),
            },
        );

        cfg.add(
            vartab,
            Instr::ReturnCode {
                code: ReturnCode::InvalidProgramId,
            },
        );

        cfg.set_basic_block(id_ok);
    }

    // Make sure that the data account is large enough. Read the size of the
    // account via `tx.accounts[0].data.length`.

    // tx.accounts[0]
    let tx_account_0 = Expression::Subscript {
        loc: Loc::Codegen,
        ty: Type::Struct(StructType::AccountInfo),
        array_ty: Type::Array(
            Type::Struct(StructType::AccountInfo).into(),
            vec![ArrayLength::Dynamic],
        ),
        expr: Expression::Builtin {
            loc: Loc::Codegen,
            tys: vec![Type::Array(
                Type::Struct(StructType::AccountInfo).into(),
                vec![ArrayLength::Dynamic],
            )],
            kind: Builtin::Accounts,
            args: vec![],
        }
        .into(),
        index: Expression::NumberLiteral {
            loc: Loc::Codegen,
            ty: Type::Uint(32),
            value: BigInt::zero(),
        }
        .into(),
    };

    let loaded_item = Expression::Load {
        loc: Loc::Codegen,
        ty: Type::Slice(Box::new(Type::Bytes(1))),
        expr: Expression::StructMember {
            loc: Loc::Codegen,
            ty: Type::Ref(Box::new(Type::Slice(Box::new(Type::Bytes(1))))),
            expr: tx_account_0.into(),
            member: 2,
        }
        .into(),
    };

    // .data.length
    let account_length = Expression::Builtin {
        loc: Loc::Codegen,
        tys: vec![Type::Uint(32)],
        kind: Builtin::ArrayLength,
        args: vec![loaded_item],
    };

    let account_data_var = vartab.temp_name("data_length", &Type::Uint(32));

    cfg.add(
        vartab,
        Instr::Set {
            loc: Loc::Codegen,
            res: account_data_var,
            expr: account_length,
        },
    );

    let account_length = Expression::Variable {
        loc: Loc::Codegen,
        ty: Type::Uint(32),
        var_no: account_data_var,
    };

    let account_no_data = Expression::Equal {
        loc: Loc::Codegen,
        left: account_length.clone().into(),
        right: Expression::NumberLiteral {
            loc: Loc::Codegen,
            ty: Type::Uint(32),
            value: 0.into(),
        }
        .into(),
    };

    let account_exists = cfg.new_basic_block("account_exists".into());
    let create_account = cfg.new_basic_block("create_account".into());

    cfg.add(
        vartab,
        Instr::BranchCond {
            cond: account_no_data,
            true_block: create_account,
            false_block: account_exists,
        },
    );

    cfg.set_basic_block(account_exists);

    let is_enough = Expression::MoreEqual {
        loc: Loc::Codegen,
        signed: false,
        left: account_length.into(),
        right: Expression::NumberLiteral {
            loc: Loc::Codegen,
            ty: Type::Uint(32),
            value: contract.fixed_layout_size.clone(),
        }
        .into(),
    };

    let account_ok = cfg.new_basic_block("account_ok".into());
    let not_enough = cfg.new_basic_block("not_enough".into());

    cfg.add(
        vartab,
        Instr::BranchCond {
            cond: is_enough,
            true_block: account_ok,
            false_block: not_enough,
        },
    );

    cfg.set_basic_block(not_enough);

    cfg.add(
        vartab,
        Instr::ReturnCode {
            code: ReturnCode::AccountDataTooSmall,
        },
    );

    cfg.set_basic_block(create_account);

    // The expressions in the @payer, @seed, @bump, and @space have been resolved in the constructors
    // context, so any variables will be bound to that vartable. Only the parameters are visible
    // when these were resolved; simply copy the decoded constructor arguments into the right variables.
    for (i, arg) in func.get_symbol_table().arguments.iter().enumerate() {
        if let Some(arg) = arg {
            let param = &func.params[i];

            vartab.add_known(*arg, param.id.as_ref().unwrap(), &param.ty);

            cfg.add(
                vartab,
                Instr::Set {
                    loc: Loc::Codegen,
                    res: *arg,
                    expr: constructor_args[i].clone(),
                },
            );
        }
    }

    if let Some((_, name)) = &func.annotations.payer {
        let metas_ty = Type::Array(
            Box::new(Type::Struct(StructType::AccountMeta)),
            vec![ArrayLength::Fixed(BigInt::from(2))],
        );

        let metas = vartab.temp_name("metas", &metas_ty);

        let account_info_ty = Type::Ref(Box::new(Type::Struct(StructType::AccountInfo)));
        let account_info_var = vartab.temp_anonymous(&account_info_ty);
        cfg.add(
            vartab,
            Instr::AccountAccess {
                loc: Loc::Codegen,
                name: name.clone(),
                var_no: account_info_var,
            },
        );
        let data_account_info_var = vartab.temp_anonymous(&account_info_ty);
        cfg.add(
            vartab,
            Instr::AccountAccess {
                loc: Loc::Codegen,
                name: BuiltinAccounts::DataAccount.to_string(),
                var_no: data_account_info_var,
            },
        );

        let account_var = Expression::Variable {
            loc: Loc::Codegen,
            ty: account_info_ty.clone(),
            var_no: account_info_var,
        };

        let data_acc_var = Expression::Variable {
            loc: Loc::Codegen,
            ty: account_info_ty,
            var_no: data_account_info_var,
        };

        let ptr_to_address = retrieve_key_from_account_info(account_var);
        let ptr_to_data_acc = retrieve_key_from_account_info(data_acc_var);

        cfg.add(
            vartab,
            Instr::Set {
                loc: Loc::Codegen,
                res: metas,
                expr: Expression::ArrayLiteral {
                    loc: Loc::Codegen,
                    ty: metas_ty.clone(),
                    dimensions: vec![2],
                    values: vec![
                        account_meta_literal(ptr_to_address, true, true),
                        account_meta_literal(ptr_to_data_acc, true, true),
                    ],
                },
            },
        );

        // Calculate minimum balance for rent-exempt
        let (space, lamports) = if let Some((_, space_expr)) = &func.annotations.space {
            let expr = expression(space_expr, cfg, contract_no, None, ns, vartab, opt);
            // If the space is not a literal or a constant expression,
            // we must verify if we are allocating enough space during runtime.
            if eval_const_number(space_expr, ns, &mut Diagnostics::default()).is_err() {
                let cond = Expression::MoreEqual {
                    loc: Loc::Codegen,
                    signed: false,
                    left: Box::new(expr.clone()),
                    right: Box::new(Expression::NumberLiteral {
                        loc: Loc::Codegen,
                        ty: Type::Uint(64),
                        value: contract.fixed_layout_size.clone(),
                    }),
                };

                let enough = cfg.new_basic_block("enough_space".to_string());
                let not_enough = cfg.new_basic_block("not_enough_space".to_string());

                cfg.add(
                    vartab,
                    Instr::BranchCond {
                        cond,
                        true_block: enough,
                        false_block: not_enough,
                    },
                );

                cfg.set_basic_block(not_enough);
                cfg.add(
                    vartab,
                    Instr::Print {
                        expr: string_to_expr(format!(
                            "value passed for space is \
                        insufficient. Contract requires at least {} bytes",
                            contract.fixed_layout_size
                        )),
                    },
                );

                cfg.add(
                    vartab,
                    Instr::ReturnCode {
                        code: ReturnCode::AccountDataTooSmall,
                    },
                );

                cfg.set_basic_block(enough);
            }

            let space_var = vartab.temp_name("space", &Type::Uint(64));

            cfg.add(
                vartab,
                Instr::Set {
                    loc: Loc::Codegen,
                    res: space_var,
                    expr,
                },
            );

            let space = Expression::Variable {
                loc: Loc::Codegen,
                ty: Type::Uint(64),
                var_no: space_var,
            };

            // https://github.com/solana-labs/solana/blob/718f433206c124da85a8aa2476c0753f351f9a28/sdk/program/src/rent.rs#L78-L82
            let lamports = Expression::Multiply {
                loc: Loc::Codegen,
                ty: Type::Uint(64),
                overflowing: false,
                left: Expression::Add {
                    loc: Loc::Codegen,
                    ty: Type::Uint(64),
                    overflowing: false,
                    left: space.clone().into(),
                    right: Expression::NumberLiteral {
                        loc: Loc::Codegen,
                        ty: Type::Uint(64),
                        value: ACCOUNT_STORAGE_OVERHEAD.into(),
                    }
                    .into(),
                }
                .into(),
                right: Expression::NumberLiteral {
                    loc: Loc::Codegen,
                    ty: Type::Uint(64),
                    value: BigInt::from(LAMPORTS_PER_BYTE_YER * EXEMPTION_THRESHOLD),
                }
                .into(),
            };

            (space, lamports)
        } else {
            let space_runtime_constant = contract.fixed_layout_size.to_u64().unwrap();

            // https://github.com/solana-labs/solana/blob/718f433206c124da85a8aa2476c0753f351f9a28/sdk/program/src/rent.rs#L78-L82
            let lamports_runtime_constant = (ACCOUNT_STORAGE_OVERHEAD + space_runtime_constant)
                * LAMPORTS_PER_BYTE_YER
                * EXEMPTION_THRESHOLD;

            (
                Expression::NumberLiteral {
                    loc: Loc::Codegen,
                    ty: Type::Uint(64),
                    value: space_runtime_constant.into(),
                },
                Expression::NumberLiteral {
                    loc: Loc::Codegen,
                    ty: Type::Uint(64),
                    value: lamports_runtime_constant.into(),
                },
            )
        };

        let instruction_var = vartab.temp_name("instruction", &Type::DynamicBytes);
        let instruction = Expression::Variable {
            loc: Loc::Codegen,
            ty: Type::DynamicBytes,
            var_no: instruction_var,
        };

        // The CreateAccount instruction is 52 bytes (4 + 8 + 8 + 32)
        let instruction_size = 52;

        cfg.add(
            vartab,
            Instr::Set {
                loc: Loc::Codegen,
                res: instruction_var,
                expr: Expression::AllocDynamicBytes {
                    loc: Loc::Codegen,
                    ty: Type::DynamicBytes,
                    size: Expression::NumberLiteral {
                        loc: Loc::Codegen,
                        ty: Type::Uint(32),
                        value: instruction_size.into(),
                    }
                    .into(),
                    initializer: None,
                },
            },
        );

        // instruction CreateAccount
        cfg.add(
            vartab,
            Instr::WriteBuffer {
                buf: instruction.clone(),
                value: Expression::NumberLiteral {
                    loc: Loc::Codegen,
                    ty: Type::Uint(32),
                    value: BigInt::from(0),
                },
                offset: Expression::NumberLiteral {
                    loc: Loc::Codegen,
                    ty: Type::Uint(32),
                    value: BigInt::from(0),
                },
            },
        );

        // lamports
        cfg.add(
            vartab,
            Instr::WriteBuffer {
                buf: instruction.clone(),
                value: lamports,
                offset: Expression::NumberLiteral {
                    loc: Loc::Codegen,
                    ty: Type::Uint(32),
                    value: BigInt::from(4),
                },
            },
        );

        // space
        cfg.add(
            vartab,
            Instr::WriteBuffer {
                buf: instruction.clone(),
                value: space,
                offset: Expression::NumberLiteral {
                    loc: Loc::Codegen,
                    ty: Type::Uint(32),
                    value: BigInt::from(12),
                },
            },
        );

        // owner
        cfg.add(
            vartab,
            Instr::WriteBuffer {
                buf: instruction.clone(),
                value: if let Some(program_id) = program_id {
                    Expression::NumberLiteral {
                        loc: Loc::Codegen,
                        ty: Type::Address(false),
                        value: BigInt::from_bytes_be(Sign::Plus, program_id),
                    }
                } else {
                    let addr_ptr = Expression::Builtin {
                        loc: Loc::Codegen,
                        tys: vec![Type::Ref(Box::new(Type::Address(false)))],
                        kind: Builtin::GetAddress,
                        args: vec![],
                    };
                    Expression::Load {
                        loc: Loc::Codegen,
                        ty: Type::Address(false),
                        expr: Box::new(addr_ptr),
                    }
                },
                offset: Expression::NumberLiteral {
                    loc: Loc::Codegen,
                    ty: Type::Uint(32),
                    value: BigInt::from(20),
                },
            },
        );

        // seeds
        let mut seeds = func
            .annotations
            .seeds
            .iter()
            .map(|seed| expression(&seed.1, cfg, contract_no, None, ns, vartab, opt))
            .collect::<Vec<Expression>>();

        if let Some((_, bump)) = &func.annotations.bump {
            let expr = ast::Expression::Cast {
                loc: Loc::Codegen,
                to: Type::Slice(Type::Bytes(1).into()),
                expr: ast::Expression::BytesCast {
                    loc: Loc::Codegen,
                    to: Type::DynamicBytes,
                    from: Type::Bytes(1),
                    expr: bump.clone().into(),
                }
                .into(),
            };
            seeds.push(expression(&expr, cfg, contract_no, None, ns, vartab, opt));
        }

        let seeds = if !seeds.is_empty() {
            let ty = Type::Array(
                Box::new(Type::Slice(Box::new(Type::Bytes(1)))),
                vec![ArrayLength::Fixed(seeds.len().into())],
            );

            let address_seeds = Expression::ArrayLiteral {
                loc: Loc::Codegen,
                ty,
                dimensions: vec![seeds.len() as u32],
                values: seeds,
            };

            let ty = Type::Array(
                Box::new(Type::Slice(Box::new(Type::Slice(Box::new(Type::Bytes(1)))))),
                vec![ArrayLength::Fixed(1.into())],
            );

            Some(Expression::ArrayLiteral {
                loc: Loc::Codegen,
                ty,
                dimensions: vec![1],
                values: vec![address_seeds],
            })
        } else {
            None
        };

        cfg.add(
            vartab,
            Instr::ExternalCall {
                loc: Loc::Codegen,
                success: None,
                seeds,
                address: Some(Expression::NumberLiteral {
                    loc: Loc::Codegen,
                    ty: Type::Address(false),
                    value: BigInt::from(0),
                }), // SystemProgram 11111111111111111111111111111111
                accounts: ExternalCallAccounts::Present(Expression::Variable {
                    loc: Loc::Codegen,
                    ty: metas_ty,
                    var_no: metas,
                }),
                payload: instruction,
                value: Expression::NumberLiteral {
                    loc: Loc::Codegen,
                    ty: Type::Uint(64),
                    value: BigInt::from(0),
                },
                gas: Expression::NumberLiteral {
                    loc: Loc::Codegen,
                    ty: Type::Uint(64),
                    value: BigInt::from(0),
                },
                callty: CallTy::Regular,
                contract_function_no: None,
                flags: None,
            },
        );

        cfg.add(vartab, Instr::Branch { block: account_ok });
    } else {
        cfg.add(
            vartab,
            Instr::ReturnCode {
                code: ReturnCode::AccountDataTooSmall,
            },
        );
    }

    cfg.set_basic_block(account_ok);

    // Write contract magic number to offset 0
    cfg.add(
        vartab,
        Instr::SetStorage {
            ty: Type::Uint(32),
            value: Expression::NumberLiteral {
                loc: Loc::Codegen,
                ty: Type::Uint(64),
                value: BigInt::from(contract.selector()),
            },
            storage: Expression::NumberLiteral {
                loc: Loc::Codegen,
                ty: Type::Uint(64),
                value: BigInt::zero(),
            },
            storage_type: None,
        },
    );

    // Calculate heap offset
    let fixed_fields_size = contract.fixed_layout_size.to_u64().unwrap();

    // align on 8 byte boundary (round up to nearest multiple of 8)
    let heap_offset = (fixed_fields_size + 7) & !7;

    // Write heap offset to 12
    cfg.add(
        vartab,
        Instr::SetStorage {
            ty: Type::Uint(32),
            value: Expression::NumberLiteral {
                loc: Loc::Codegen,
                ty: Type::Uint(64),
                value: BigInt::from(heap_offset),
            },
            storage: Expression::NumberLiteral {
                loc: Loc::Codegen,
                ty: Type::Uint(64),
                value: BigInt::from(12),
            },
            storage_type: None,
        },
    );
}
