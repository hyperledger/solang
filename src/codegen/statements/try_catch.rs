// SPDX-License-Identifier: Apache-2.0

use super::{statement, Builtin, LoopScopes, Options};
use crate::codegen::{
    cfg::{ControlFlowGraph, Instr},
    constructor::call_constructor,
    encoding::{abi_decode, abi_encode},
    expression::{default_gas, expression},
    polkadot,
    vartable::Vartable,
    Expression,
};
use crate::sema::ast::{
    self, CallTy, Function, Namespace, RetrieveType, TryCatch, Type, Type::Uint,
};
use num_bigint::BigInt;
use num_traits::Zero;
use solang_parser::pt::{self, CodeLocation, Loc::Codegen};

/// Resolve try catch statement
pub(super) fn try_catch(
    try_stmt: &TryCatch,
    func: &Function,
    cfg: &mut ControlFlowGraph,
    callee_contract_no: usize,
    ns: &Namespace,
    vartab: &mut Vartable,
    loops: &mut LoopScopes,
    placeholder: Option<&Instr>,
    return_override: Option<&Instr>,
    opt: &Options,
) {
    if !ns.target.is_polkadot() {
        unimplemented!()
    }

    dbg!(try_stmt);

    let ok_block = cfg.new_basic_block("ok".to_string());
    let catch_block = cfg.new_basic_block("catch".to_string());
    let finally_block = cfg.new_basic_block("finally".to_string());

    let error_ret_data_var = build_try_output(
        try_stmt,
        cfg,
        ns,
        vartab,
        func,
        callee_contract_no,
        opt,
        ok_block,
        catch_block,
    );

    vartab.new_dirty_tracker();

    insert_ok(
        try_stmt,
        func,
        cfg,
        callee_contract_no,
        ns,
        vartab,
        loops,
        placeholder,
        return_override,
        opt,
        ok_block,
        finally_block,
    );

    cfg.set_basic_block(catch_block);

    for (error_param_pos, error_param, error_stmt) in &try_stmt.errors {
        let no_reason_block = cfg.new_basic_block("no_reason".to_string());

        let error_var = match error_param_pos {
            Some(pos) => *pos,
            _ => vartab.temp_anonymous(&Type::String),
        };

        let buf = Expression::Variable {
            loc: Codegen,
            ty: Type::DynamicBytes,
            var_no: error_ret_data_var,
        };

        // Expect the returned data to contain at least the selector + 1 byte of data
        let ret_data_len = Expression::Builtin {
            loc: Codegen,
            tys: vec![Type::Uint(32)],
            kind: Builtin::ArrayLength,
            args: vec![buf.clone()],
        };
        let enough_data_block = cfg.new_basic_block("enough_data".into());
        let no_match_err_id = cfg.new_basic_block("no_match_err_id".into());
        let selector_data_len = Expression::NumberLiteral {
            loc: Codegen,
            ty: Type::Uint(32),
            value: 4.into(),
        };
        let cond_enough_data = Expression::More {
            loc: Codegen,
            signed: false,
            left: ret_data_len.into(),
            right: selector_data_len.into(),
        };
        cfg.add(
            vartab,
            Instr::BranchCond {
                cond: cond_enough_data,
                true_block: enough_data_block,
                false_block: no_match_err_id,
            },
        );

        cfg.set_basic_block(enough_data_block);
        // Expect the returned data to match the 4 bytes function selector for "Error(string)"
        let tys = &[Type::Bytes(4), error_param.ty.clone()];
        let decoded = abi_decode(&Codegen, &buf, tys, ns, vartab, cfg, None);
        //let selector_ty = Type::Ref(Uint(32).into());
        let err_id = Expression::NumberLiteral {
            loc: Codegen,
            ty: Type::Bytes(4),
            value: 0x08c3_79a0.into(),
        }
        .into();
        let cond = Expression::Equal {
            loc: Codegen,
            left: decoded[0].clone().into(),
            right: err_id,
        };
        let match_err_id = cfg.new_basic_block("match_err_id".into());
        let instruction = Instr::BranchCond {
            cond,
            true_block: match_err_id,
            false_block: no_match_err_id,
        };
        cfg.add(vartab, instruction);

        cfg.set_basic_block(no_match_err_id);
        let encoded_args = Some(buf);
        cfg.add(vartab, Instr::AssertFailure { encoded_args });

        cfg.set_basic_block(match_err_id);
        let instruction = Instr::Set {
            loc: Codegen,
            res: error_var,
            expr: decoded[1].clone(),
        };
        cfg.add(vartab, instruction);

        let mut reachable = true;

        for stmt in error_stmt {
            statement(
                stmt,
                func,
                cfg,
                callee_contract_no,
                ns,
                vartab,
                loops,
                placeholder,
                return_override,
                opt,
            );

            reachable = stmt.reachable();
        }
        if reachable {
            cfg.add(
                vartab,
                Instr::Branch {
                    block: finally_block,
                },
            );
        }

        cfg.set_basic_block(no_reason_block);
    }

    if let Some(res) = try_stmt.catch_param_pos {
        let instruction = Instr::Set {
            loc: Codegen,
            res,
            expr: Expression::ReturnData { loc: Codegen },
        };
        cfg.add(vartab, instruction);
    }

    let mut reachable = true;

    if let Some(stmts) = &try_stmt.catch_stmt {
        for stmt in stmts {
            statement(
                stmt,
                func,
                cfg,
                callee_contract_no,
                ns,
                vartab,
                loops,
                placeholder,
                return_override,
                opt,
            );

            reachable = stmt.reachable();
        }
    }

    if reachable {
        cfg.add(
            vartab,
            Instr::Branch {
                block: finally_block,
            },
        );
    }

    let mut set = vartab.pop_dirty_tracker();
    if let Some(pos) = &try_stmt.catch_param_pos {
        set.remove(pos);
    }
    for (pos, _, _) in &try_stmt.errors {
        if let Some(pos) = pos {
            set.remove(pos);
        }
    }
    cfg.set_phis(finally_block, set);

    cfg.set_basic_block(finally_block);
}

/// Executes the try statement and returns the variable number of the return data.
fn build_try_output(
    try_stmt: &TryCatch,
    cfg: &mut ControlFlowGraph,
    ns: &Namespace,
    vartab: &mut Vartable,
    func: &Function,
    callee_contract_no: usize,
    opt: &Options,
    ok_block: usize,
    catch_block: usize,
) -> usize {
    let (cases, return_types) = exec_try(try_stmt, func, cfg, callee_contract_no, ns, vartab, opt);

    let error_ret_data_var = vartab.temp_name("error_ret_data", &Type::DynamicBytes);

    vartab.new_dirty_tracker();

    cfg.set_basic_block(cases.error_no_data);
    cfg.add(
        vartab,
        Instr::Set {
            loc: Codegen,
            res: error_ret_data_var,
            expr: Expression::AllocDynamicBytes {
                loc: Codegen,
                ty: Type::DynamicBytes,
                size: Expression::NumberLiteral {
                    loc: Codegen,
                    ty: Uint(32),
                    value: 0.into(),
                }
                .into(),
                initializer: Some(vec![]),
            },
        },
    );
    cfg.add(vartab, Instr::Branch { block: catch_block });

    cfg.set_basic_block(cases.revert);
    cfg.add(
        vartab,
        Instr::Set {
            loc: Codegen,
            res: error_ret_data_var,
            expr: Expression::ReturnData { loc: Codegen },
        },
    );
    cfg.add(vartab, Instr::Branch { block: catch_block });

    vartab.set_dirty(error_ret_data_var);
    cfg.set_phis(catch_block, vartab.pop_dirty_tracker());

    cfg.set_basic_block(cases.success);

    if !try_stmt.returns.is_empty() {
        let mut res = Vec::new();

        for ret in &try_stmt.returns {
            res.push(match ret {
                (Some(pos), _) => *pos,
                (None, param) => vartab.temp_anonymous(&param.ty),
            });
        }

        let buf = &Expression::ReturnData { loc: Codegen };
        let decoded = abi_decode(&Codegen, buf, &return_types, ns, vartab, cfg, None);
        for instruction in res.iter().zip(decoded).map(|(var, expr)| Instr::Set {
            loc: Codegen,
            res: *var,
            expr,
        }) {
            cfg.add(vartab, instruction)
        }
    }
    cfg.add(vartab, Instr::Branch { block: ok_block });

    error_ret_data_var
}

/// Insert the execution of the `try` statement into the CFG.
fn exec_try(
    try_stmt: &TryCatch,
    func: &Function,
    cfg: &mut ControlFlowGraph,
    callee_contract_no: usize,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
) -> (polkadot::RetCodeCheck, Vec<Type>) {
    let success = vartab.temp(
        &pt::Identifier {
            loc: try_stmt.expr.loc(),
            name: "success".to_owned(),
        },
        &Type::Bool,
    );
    match &try_stmt.expr {
        ast::Expression::ExternalFunctionCall {
            loc,
            function,
            args,
            call_args,
            ..
        } => {
            if let Type::ExternalFunction {
                returns: func_returns,
                ..
            } = function.ty()
            {
                let value = if let Some(value) = &call_args.value {
                    expression(value, cfg, callee_contract_no, Some(func), ns, vartab, opt)
                } else {
                    Expression::NumberLiteral {
                        loc: Codegen,
                        ty: Type::Value,
                        value: BigInt::zero(),
                    }
                };
                let gas = if let Some(gas) = &call_args.gas {
                    expression(gas, cfg, callee_contract_no, Some(func), ns, vartab, opt)
                } else {
                    default_gas(ns)
                };
                let function = expression(
                    function,
                    cfg,
                    callee_contract_no,
                    Some(func),
                    ns,
                    vartab,
                    opt,
                );

                let mut args = args
                    .iter()
                    .map(|a| expression(a, cfg, callee_contract_no, Some(func), ns, vartab, opt))
                    .collect::<Vec<Expression>>();

                let selector = function.external_function_selector();

                let address = function.external_function_address();

                args.insert(0, selector);
                let (payload, _) = abi_encode(loc, args, ns, vartab, cfg, false);

                let flags = call_args.flags.as_ref().map(|expr| {
                    expression(expr, cfg, callee_contract_no, Some(func), ns, vartab, opt)
                });

                cfg.add(
                    vartab,
                    Instr::ExternalCall {
                        success: Some(success),
                        address: Some(address),
                        accounts: None,
                        seeds: None,
                        payload,
                        value,
                        gas,
                        callty: CallTy::Regular,
                        contract_function_no: None,
                        flags,
                    },
                );

                let cases = polkadot::RetCodeCheckBuilder::default()
                    .loc(*loc)
                    .success_var(success)
                    .insert(cfg, vartab);
                (cases, func_returns)
            } else {
                // dynamic dispatch
                unimplemented!();
            }
        }
        ast::Expression::Constructor {
            loc,
            contract_no,
            constructor_no,
            args,
            call_args,
            ..
        } => {
            let address_res = match try_stmt.returns.get(0) {
                Some((Some(pos), _)) => *pos,
                _ => vartab.temp_anonymous(&Type::Contract(*contract_no)),
            };

            call_constructor(
                loc,
                *contract_no,
                callee_contract_no,
                constructor_no,
                args,
                call_args,
                address_res,
                Some(success),
                Some(func),
                ns,
                vartab,
                cfg,
                opt,
            );

            let cases = polkadot::RetCodeCheckBuilder::default()
                .loc(*loc)
                .success_var(success)
                .insert(cfg, vartab);
            (cases, vec![])
        }
        _ => unreachable!(),
    }
}

/// Insert the `Ok` code into the CFG.
fn insert_ok(
    try_stmt: &TryCatch,
    func: &Function,
    cfg: &mut ControlFlowGraph,
    callee_contract_no: usize,
    ns: &Namespace,
    vartab: &mut Vartable,
    loops: &mut LoopScopes,
    placeholder: Option<&Instr>,
    return_override: Option<&Instr>,
    opt: &Options,
    ok_block: usize,
    finally_block: usize,
) {
    cfg.set_basic_block(ok_block);

    let mut finally_reachable = true;

    for stmt in &try_stmt.ok_stmt {
        statement(
            stmt,
            func,
            cfg,
            callee_contract_no,
            ns,
            vartab,
            loops,
            placeholder,
            return_override,
            opt,
        );

        finally_reachable = stmt.reachable();
    }

    if finally_reachable {
        cfg.add(
            vartab,
            Instr::Branch {
                block: finally_block,
            },
        );
    }
}
