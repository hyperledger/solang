// SPDX-License-Identifier: Apache-2.0

use crate::codegen::{
    cfg::{ASTFunction, ControlFlowGraph, Instr, InternalCallTy, ReturnCode},
    solana_deploy::solana_deploy,
    vartable::Vartable,
    Builtin, Expression, Options,
};
use crate::sema::ast::{Namespace, StructType, Type};
use num_bigint::{BigInt, Sign};
use num_traits::Zero;
use solang_parser::{pt, pt::Loc};

use crate::codegen::encoding::{abi_decode, abi_encode};
use crate::sema::solana_accounts::BuiltinAccounts;

pub const SOLANA_DISPATCH_CFG_NAME: &str = "solang_dispatch";

/// Create the dispatch for the Solana target
pub(crate) fn function_dispatch(
    contract_no: usize,
    all_cfg: &[ControlFlowGraph],
    ns: &mut Namespace,
    opt: &Options,
) -> ControlFlowGraph {
    let mut vartab = Vartable::new(ns.next_id);
    let mut cfg = ControlFlowGraph::new(SOLANA_DISPATCH_CFG_NAME.into(), ASTFunction::None);

    let switch_block = cfg.new_basic_block("switch".to_string());
    let no_function_matched = cfg.new_basic_block("no_function_matched".to_string());

    let argsdata_var = vartab.temp_name("input", &Type::BufferPointer);
    let argslen_var = vartab.temp_name("input_len", &Type::Uint(64));

    let sol_params = Expression::FunctionArg {
        loc: Loc::Codegen,
        ty: Type::Struct(StructType::SolParameters),
        arg_no: 0,
    };

    // ty:bufferptr argsdata_var = load ty:ref(ty:bufferptr) (structmember ty:ref(ty:bufferptr) (funcarg ty:struct(solparam), 2))
    cfg.add(
        &mut vartab,
        Instr::Set {
            res: argsdata_var,
            loc: Loc::Codegen,
            expr: Expression::Load {
                loc: Loc::Codegen,
                ty: Type::BufferPointer,
                expr: Expression::StructMember {
                    loc: Loc::Codegen,
                    ty: Type::Ref(Type::BufferPointer.into()),
                    expr: sol_params.clone().into(),
                    member: 2,
                }
                .into(),
            },
        },
    );

    let argsdata = Expression::Variable {
        loc: Loc::Codegen,
        ty: Type::BufferPointer,
        var_no: argsdata_var,
    };

    // ty:uint64 argslen_var = load ref(ty:uint64) (structmember ref(ty:uin64) (funcarg ty:struct(solparam), 3))
    cfg.add(
        &mut vartab,
        Instr::Set {
            res: argslen_var,
            loc: Loc::Codegen,
            expr: Expression::Load {
                loc: Loc::Codegen,
                ty: Type::Uint(64),
                expr: Expression::StructMember {
                    loc: Loc::Codegen,
                    ty: Type::Ref(Type::Uint(64).into()),
                    expr: sol_params.into(),
                    member: 3,
                }
                .into(),
            },
        },
    );

    let argslen = Expression::Variable {
        loc: Loc::Codegen,
        ty: Type::Uint(64),
        var_no: argslen_var,
    };

    let not_fallback = Expression::MoreEqual {
        loc: Loc::Codegen,
        signed: false,
        left: argslen.clone().into(),
        right: Expression::NumberLiteral {
            loc: Loc::Codegen,
            ty: Type::Uint(64),
            value: BigInt::from(8u8),
        }
        .into(),
    };

    cfg.add(
        &mut vartab,
        Instr::BranchCond {
            cond: not_fallback,
            true_block: switch_block,
            false_block: no_function_matched,
        },
    );
    cfg.set_basic_block(switch_block);

    let fid = Expression::Builtin {
        loc: Loc::Codegen,
        tys: vec![Type::Uint(64)],
        kind: Builtin::ReadFromBuffer,
        args: vec![
            argsdata.clone(),
            Expression::NumberLiteral {
                loc: Loc::Codegen,
                ty: Type::Uint(64),
                value: BigInt::zero(),
            },
        ],
    };

    let argsdata = Expression::AdvancePointer {
        pointer: Box::new(argsdata),
        bytes_offset: Box::new(Expression::NumberLiteral {
            loc: Loc::Codegen,
            ty: Type::Uint(32),
            value: BigInt::from(8u8),
        }),
    };
    let argslen = Expression::Subtract {
        loc: Loc::Codegen,
        ty: Type::Uint(64),
        overflowing: false,
        left: Box::new(argslen),
        right: Box::new(Expression::NumberLiteral {
            loc: Loc::Codegen,
            ty: Type::Uint(64),
            value: BigInt::from(8u8),
        }),
    };

    let mut cases = Vec::new();

    for (cfg_no, func_cfg) in all_cfg.iter().enumerate() {
        if !func_cfg.public {
            continue;
        }

        let entry = if func_cfg.ty == pt::FunctionTy::Function {
            add_function_dispatch_case(
                cfg_no,
                func_cfg,
                &argsdata,
                argslen.clone(),
                contract_no,
                ns,
                &mut vartab,
                &mut cfg,
            )
        } else if func_cfg.ty == pt::FunctionTy::Constructor {
            add_constructor_dispatch_case(
                contract_no,
                cfg_no,
                &argsdata,
                argslen.clone(),
                func_cfg,
                ns,
                &mut vartab,
                &mut cfg,
                opt,
            )
        } else {
            continue;
        };

        cases.push((
            Expression::NumberLiteral {
                loc: Loc::Codegen,
                ty: Type::Uint(64),
                value: BigInt::from_bytes_le(Sign::Plus, &func_cfg.selector),
            },
            entry,
        ));
    }

    cfg.set_basic_block(switch_block);

    cfg.add(
        &mut vartab,
        Instr::Switch {
            cond: fid,
            cases,
            default: no_function_matched,
        },
    );

    cfg.set_basic_block(no_function_matched);

    let fallback = all_cfg
        .iter()
        .enumerate()
        .find(|(_, cfg)| cfg.public && cfg.ty == pt::FunctionTy::Fallback);

    if fallback.is_none() {
        cfg.add(
            &mut vartab,
            Instr::ReturnCode {
                code: ReturnCode::FunctionSelectorInvalid,
            },
        );

        vartab.finalize(ns, &mut cfg);

        return cfg;
    }

    match fallback {
        Some((cfg_no, fallback_cfg)) => {
            let ASTFunction::SolidityFunction(ast_func_no) = fallback_cfg.function_no else {
                unreachable!("fallback must be a Solidity function");
            };

            if ns.functions[ast_func_no]
                .solana_accounts
                .borrow()
                .contains_key(BuiltinAccounts::DataAccount.as_str())
            {
                check_magic(ns.contracts[contract_no].selector(), &mut cfg, &mut vartab);
            }

            cfg.add(
                &mut vartab,
                Instr::Call {
                    res: vec![],
                    return_tys: vec![],
                    args: vec![],
                    call: InternalCallTy::Static { cfg_no },
                },
            );

            cfg.add(
                &mut vartab,
                Instr::ReturnCode {
                    code: ReturnCode::Success,
                },
            );
        }
        None => {
            cfg.add(
                &mut vartab,
                Instr::ReturnCode {
                    code: ReturnCode::InvalidDataError,
                },
            );
        }
    }

    vartab.finalize(ns, &mut cfg);

    cfg
}

/// Add the dispatch for function given a matched selector
fn add_function_dispatch_case(
    cfg_no: usize,
    func_cfg: &ControlFlowGraph,
    argsdata: &Expression,
    argslen: Expression,
    contract_no: usize,
    ns: &Namespace,
    vartab: &mut Vartable,
    cfg: &mut ControlFlowGraph,
) -> usize {
    let entry = cfg.new_basic_block(format!("function_cfg_{cfg_no}"));
    cfg.set_basic_block(entry);

    let ast_func_no = if let ASTFunction::SolidityFunction(func_no) = func_cfg.function_no {
        func_no
    } else if let Some(func_no) = func_cfg.modifier {
        func_no
    } else {
        unreachable!("should not dispatch this function")
    };

    if ns.functions[ast_func_no]
        .solana_accounts
        .borrow()
        .contains_key(BuiltinAccounts::DataAccount.as_str())
    {
        check_magic(ns.contracts[contract_no].selector(), cfg, vartab);
    }

    let truncated_len = Expression::Trunc {
        loc: Loc::Codegen,
        ty: Type::Uint(32),
        expr: Box::new(argslen),
    };

    let tys = func_cfg
        .params
        .iter()
        .map(|e| e.ty.clone())
        .collect::<Vec<Type>>();
    let decoded = abi_decode(
        &Loc::Codegen,
        argsdata,
        &tys,
        ns,
        vartab,
        cfg,
        Some(truncated_len),
    );

    let mut returns: Vec<usize> = Vec::with_capacity(func_cfg.returns.len());
    let mut return_tys: Vec<Type> = Vec::with_capacity(func_cfg.returns.len());
    let mut returns_expr: Vec<Expression> = Vec::with_capacity(func_cfg.returns.len());
    for item in func_cfg.returns.iter() {
        let new_var = vartab.temp_anonymous(&item.ty);
        returns.push(new_var);
        return_tys.push(item.ty.clone());
        returns_expr.push(Expression::Variable {
            loc: Loc::Codegen,
            ty: item.ty.clone(),
            var_no: new_var,
        });
    }

    cfg.add(
        vartab,
        Instr::Call {
            res: returns,
            call: InternalCallTy::Static { cfg_no },
            args: decoded,
            return_tys,
        },
    );

    if !func_cfg.returns.is_empty() {
        let (data, data_len) = abi_encode(&Loc::Codegen, returns_expr, ns, vartab, cfg, false);
        let zext_len = Expression::ZeroExt {
            loc: Loc::Codegen,
            ty: Type::Uint(64),
            expr: Box::new(data_len),
        };
        cfg.add(
            vartab,
            Instr::ReturnData {
                data,
                data_len: zext_len,
            },
        );
    } else {
        // TODO: On Solana, we could elide setting the return data if this function calls no external functions
        // and replace this with a simple Instr::Return, which does not set any return data.
        //
        // The return data buffer is empty when Solana VM first executes a program, but if another program is
        // called via CPI then that program may set return data. We must clear this buffer, else return data
        // from the CPI callee will be visible to this program's callee.
        cfg.add(
            vartab,
            Instr::ReturnData {
                data: Expression::AllocDynamicBytes {
                    loc: Loc::Codegen,
                    ty: Type::DynamicBytes,
                    size: Expression::NumberLiteral {
                        loc: Loc::Codegen,
                        ty: Type::Uint(32),
                        value: 0.into(),
                    }
                    .into(),
                    initializer: None,
                },
                data_len: Expression::NumberLiteral {
                    loc: Loc::Codegen,
                    ty: Type::Uint(64),
                    value: 0.into(),
                },
            },
        );
    }

    entry
}

/// Create the dispatch for a contract constructor. This case creates a new function in
/// the CFG because we want to use the abi decoding implementation from codegen.
fn add_constructor_dispatch_case(
    contract_no: usize,
    cfg_no: usize,
    argsdata: &Expression,
    argslen: Expression,
    func_cfg: &ControlFlowGraph,
    ns: &mut Namespace,
    vartab: &mut Vartable,
    cfg: &mut ControlFlowGraph,
    opt: &Options,
) -> usize {
    let entry = cfg.new_basic_block(format!("constructor_cfg_{cfg_no}"));
    cfg.set_basic_block(entry);

    check_magic(0, cfg, vartab);

    let mut returns: Vec<Expression> = Vec::new();

    if !func_cfg.params.is_empty() {
        let tys = func_cfg
            .params
            .iter()
            .map(|e| e.ty.clone())
            .collect::<Vec<Type>>();
        let truncated_len = Expression::Trunc {
            loc: Loc::Codegen,
            ty: Type::Uint(32),
            expr: Box::new(argslen),
        };
        returns = abi_decode(
            &Loc::Codegen,
            argsdata,
            &tys,
            ns,
            vartab,
            cfg,
            Some(truncated_len),
        );
    }

    if let ASTFunction::SolidityFunction(function_no) = func_cfg.function_no {
        let func = &ns.functions[function_no];

        solana_deploy(func, &returns, contract_no, vartab, cfg, ns, opt);
    } else if let Some((func, _)) = &ns.contracts[contract_no].default_constructor {
        solana_deploy(func, &returns, contract_no, vartab, cfg, ns, opt);
    } else {
        unreachable!();
    }

    // Call storage initializer
    cfg.add(
        vartab,
        Instr::Call {
            res: vec![],
            return_tys: vec![],
            call: InternalCallTy::Static {
                cfg_no: ns.contracts[contract_no].initializer.unwrap(),
            },
            args: vec![],
        },
    );

    cfg.add(
        vartab,
        Instr::Call {
            res: vec![],
            return_tys: vec![],
            call: InternalCallTy::Static { cfg_no },
            args: returns,
        },
    );

    cfg.add(
        vartab,
        Instr::ReturnCode {
            code: ReturnCode::Success,
        },
    );

    entry
}

fn check_magic(magic_value: u32, cfg: &mut ControlFlowGraph, vartab: &mut Vartable) {
    // check for magic in data account, to see if data account is initialized
    let magic_ok = cfg.new_basic_block("magic_ok".into());
    let magic_bad = cfg.new_basic_block("magic_bad".into());

    let magic = vartab.temp_name("magic", &Type::Uint(32));

    cfg.add(
        vartab,
        Instr::LoadStorage {
            res: magic,
            ty: Type::Uint(32),
            storage: Expression::NumberLiteral {
                loc: Loc::Codegen,
                ty: Type::Uint(32),
                value: 0.into(),
            },
            storage_type: None,
        },
    );

    cfg.add(
        vartab,
        Instr::BranchCond {
            cond: Expression::Equal {
                loc: Loc::Codegen,
                left: Expression::Variable {
                    loc: Loc::Codegen,
                    ty: Type::Uint(32),
                    var_no: magic,
                }
                .into(),
                right: Expression::NumberLiteral {
                    loc: Loc::Codegen,
                    ty: Type::Uint(32),
                    value: magic_value.into(),
                }
                .into(),
            },
            true_block: magic_ok,
            false_block: magic_bad,
        },
    );

    cfg.set_basic_block(magic_bad);

    cfg.add(
        vartab,
        Instr::ReturnCode {
            code: ReturnCode::InvalidDataError,
        },
    );

    cfg.set_basic_block(magic_ok);
}
