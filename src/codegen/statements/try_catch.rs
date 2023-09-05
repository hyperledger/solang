// SPDX-License-Identifier: Apache-2.0

use super::{statement, Builtin, LoopScopes, Options};
use crate::codegen::{
    cfg::{ControlFlowGraph, Instr},
    constructor::call_constructor,
    encoding::{abi_decode, abi_encode},
    expression::{default_gas, expression},
    polkadot,
    revert::{ERROR_SELECTOR, PANIC_SELECTOR},
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

    let ok_block = cfg.new_basic_block("ok".to_string());
    let catch_block = cfg.new_basic_block("catch".to_string());
    let finally_block = cfg.new_basic_block("finally".to_string());

    let error_ret_data_var = insert_try_expression(
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

    insert_success_code_block(
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

    insert_catch_clauses(
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
        catch_block,
        finally_block,
        error_ret_data_var,
    );

    //  Remove the variables only in scope inside the catch clauses block from the phi set for the finally block
    let mut set = vartab.pop_dirty_tracker();
    if let Some(pos) = try_stmt
        .catch_all
        .as_ref()
        .and_then(|clause| clause.param_pos)
    {
        set.remove(&pos);
    }
    for clause in &try_stmt.errors {
        if let Some(pos) = clause.param_pos {
            set.remove(&pos);
        }
    }
    cfg.set_phis(finally_block, set);

    cfg.set_basic_block(finally_block);
}

/// Insert try statement execution and error data collection into the CFG.
/// Returns the variable number of the return error data.
fn insert_try_expression(
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

/// Insert the execution of the `try` expression into the CFG.
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

/// Insert the success code into the CFG.
fn insert_success_code_block(
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

/// Insert all catch cases into the CFG.
fn insert_catch_clauses(
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
    catch_block: usize,
    finally_block: usize,
    error_ret_data_var: usize,
) {
    cfg.set_basic_block(catch_block);

    let buffer = Expression::Variable {
        loc: Codegen,
        ty: Type::DynamicBytes,
        var_no: error_ret_data_var,
    };

    // Check for error blocks and dispatch
    // If no errors, go straight to the catch all block
    if try_stmt.errors.is_empty() {
        insert_catchall_clause_code_block(
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
            finally_block,
            buffer,
        );
        return;
    }

    let no_match_err_id = cfg.new_basic_block("no_match_err_id".into());

    // Dispatch according to the error selector.
    // Currently, only catchin  "Error" and "Panic" are supported.
    // Expect the returned data to contain at least the selector + 1 byte of data.
    // If the error data len is <4 and a fallback available then proceed else bubble
    let ret_data_len = Expression::Builtin {
        loc: Codegen,
        tys: vec![Type::Uint(32)],
        kind: Builtin::ArrayLength,
        args: vec![buffer.clone()],
    };

    let mut next_clause = Some(cfg.new_basic_block("catch_error_0".into()));

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
            true_block: next_clause.unwrap(),
            false_block: no_match_err_id,
        },
    );

    while let Some((n, clause)) = try_stmt.errors.iter().enumerate().next() {
        cfg.set_basic_block(next_clause.unwrap());
        next_clause = try_stmt
            .errors
            .get(n + 1)
            .map(|_| cfg.new_basic_block(format!("catch_error_{}", n + 1)));

        let error_var = clause
            .param_pos
            .unwrap_or_else(|| vartab.temp_anonymous(&clause.param.as_ref().unwrap().ty));

        // Expect the returned data to match the 4 bytes function selector for "Error(string)"
        let err_id = Expression::NumberLiteral {
            loc: Codegen,
            ty: Type::Bytes(4),
            value: match clause.param.as_ref().unwrap().ty {
                Type::String => ERROR_SELECTOR,
                Type::Uint(256) => PANIC_SELECTOR,
                _ => unreachable!("Only 'Error(string)' and 'Panic(uint256)' can be caught"),
            }
            .into(),
        };
        let offset = Expression::NumberLiteral {
            loc: Codegen,
            ty: Uint(32),
            value: 0.into(),
        };
        let err_selector = Expression::Builtin {
            loc: Codegen,
            tys: vec![Type::Bytes(4)],
            kind: Builtin::ReadFromBuffer,
            args: vec![buffer.clone(), offset],
        };
        let cond = Expression::Equal {
            loc: Codegen,
            left: err_selector.into(),
            right: err_id.into(),
        };
        let match_err_id = cfg.new_basic_block("match_err_id".into());
        let instruction = Instr::BranchCond {
            cond,
            true_block: match_err_id,
            false_block: next_clause.unwrap_or(no_match_err_id),
        };
        cfg.add(vartab, instruction);

        cfg.set_basic_block(match_err_id);
        let tys = &[Type::Bytes(4), clause.param.as_ref().unwrap().ty.clone()];
        let decoded = abi_decode(&Codegen, &buffer, tys, ns, vartab, cfg, None);
        let instruction = Instr::Set {
            loc: Codegen,
            res: error_var,
            expr: decoded[1].clone(),
        };
        cfg.add(vartab, instruction);

        let mut reachable = true;
        for stmt in &clause.stmt {
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
    }

    // If the selector doesn't match any of the errors and no fallback then bubble else catch
    cfg.set_basic_block(no_match_err_id);
    if try_stmt.catch_all.is_none() {
        let encoded_args = Some(buffer.clone());
        cfg.add(vartab, Instr::AssertFailure { encoded_args });
    } else {
        insert_catchall_clause_code_block(
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
            finally_block,
            buffer.clone(),
        );
    }
}

/// Insert the fallback catch code into the CFG.
fn insert_catchall_clause_code_block(
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
    finally_block: usize,
    error_data_buf: Expression,
) {
    if let Some(res) = try_stmt.catch_all.as_ref().unwrap().param_pos {
        let instruction = Instr::Set {
            loc: Codegen,
            res,
            expr: error_data_buf,
        };
        cfg.add(vartab, instruction);
    }

    let mut reachable = true;

    for stmt in &try_stmt.catch_all.as_ref().unwrap().stmt {
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
}
