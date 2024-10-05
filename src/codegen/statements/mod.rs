// SPDX-License-Identifier: Apache-2.0

use super::{
    cfg::{ControlFlowGraph, Instr},
    events::new_event_emitter,
    expression::{assign_single, emit_function_call, expression},
    revert::revert,
    unused_variable::{
        should_remove_assignment, should_remove_variable, SideEffectsCheckParameters,
    },
    vartable::Vartable,
    yul::inline_assembly_cfg,
    Builtin, Expression, Options,
};
use crate::sema::ast::{
    self, ArrayLength, DestructureField, Function, Namespace, RetrieveType, SolanaAccount,
    Statement, Type, Type::Uint,
};
use crate::sema::solana_accounts::BuiltinAccounts;
use crate::sema::Recurse;
use num_bigint::BigInt;
use num_traits::Zero;
use solang_parser::pt::{self, CodeLocation, Loc, Loc::Codegen};

mod try_catch;

/// Resolve a statement, which might be a block of statements or an entire body of a function
pub(crate) fn statement(
    stmt: &Statement,
    func: &Function,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    ns: &Namespace,
    vartab: &mut Vartable,
    loops: &mut LoopScopes,
    placeholder: Option<&Instr>,
    return_override: Option<&Instr>,
    opt: &Options,
) {
    match stmt {
        Statement::Block { statements, .. } => {
            for stmt in statements {
                statement(
                    stmt,
                    func,
                    cfg,
                    contract_no,
                    ns,
                    vartab,
                    loops,
                    placeholder,
                    return_override,
                    opt,
                );

                if !stmt.reachable() {
                    break;
                }
            }
        }
        Statement::VariableDecl(loc, pos, _, Some(init)) => {
            if should_remove_variable(*pos, func, opt, ns) {
                let mut params = SideEffectsCheckParameters {
                    cfg,
                    contract_no,
                    func: Some(func),
                    ns,
                    vartab,
                    opt,
                };
                //If we remove the assignment, we must keep expressions that have side effects
                init.recurse(&mut params, process_side_effects_expressions);
                return;
            }

            let mut expression = expression(init, cfg, contract_no, Some(func), ns, vartab, opt);

            // Let's check if the declaration is a declaration of a dynamic array
            if let Expression::AllocDynamicBytes {
                loc: loc_dyn_arr,
                ty: ty_dyn_arr @ Type::Array(..),
                size,
                initializer: opt,
            } = expression
            {
                let temp_res = vartab.temp_name("array_length", &Uint(32));

                cfg.add(
                    vartab,
                    Instr::Set {
                        loc: *loc,
                        res: temp_res,
                        expr: *size,
                    },
                );
                // If expression is an AllocDynamic array, replace the expression with AllocDynamicArray(_,_,tempvar,_) to avoid inserting size twice in the cfg
                expression = Expression::AllocDynamicBytes {
                    loc: loc_dyn_arr,
                    ty: ty_dyn_arr,
                    size: Box::new(Expression::Variable {
                        loc: *loc,
                        ty: Uint(32),
                        var_no: temp_res,
                    }),
                    initializer: opt,
                };
                cfg.array_lengths_temps.insert(*pos, temp_res);
            } else if let Expression::Variable { var_no, .. } = &expression {
                // If declaration happens with an existing array, check if the size of the array is known.
                // If the size of the right hand side is known (is in the array_length_map), make the left hand side track it
                // Now, we will have two keys in the map that point to the same temporary variable
                if let Some(to_add) = cfg.array_lengths_temps.clone().get(var_no) {
                    cfg.array_lengths_temps.insert(*pos, *to_add);
                }
            }

            cfg.add(
                vartab,
                Instr::Set {
                    loc: *loc,
                    res: *pos,
                    expr: expression,
                },
            );
        }
        Statement::VariableDecl(loc, pos, param, None) => {
            if should_remove_variable(*pos, func, opt, ns) {
                return;
            }

            // Add variable as undefined
            cfg.add(
                vartab,
                Instr::Set {
                    loc: *loc,
                    res: *pos,
                    expr: Expression::Undefined {
                        ty: param.ty.clone(),
                    },
                },
            );
            // Handling arrays without size, defaulting the initial size with zero

            if matches!(param.ty, Type::Array(..)) {
                let num = Expression::NumberLiteral {
                    loc: Codegen,
                    ty: Uint(32),
                    value: BigInt::zero(),
                };
                let temp_res = vartab.temp_name("array_length", &Uint(32));
                cfg.add(
                    vartab,
                    Instr::Set {
                        loc: *loc,
                        res: temp_res,
                        expr: num,
                    },
                );
                cfg.array_lengths_temps.insert(*pos, temp_res);
            }
        }
        Statement::Return(_, expr) => {
            if let Some(return_instr) = return_override {
                cfg.add(vartab, return_instr.clone());
            } else {
                match expr {
                    None => cfg.add(vartab, Instr::Return { value: Vec::new() }),
                    Some(expr) => returns(expr, cfg, contract_no, func, ns, vartab, opt),
                }
            }
        }
        Statement::Expression(_, _, expr) => {
            match expr {
                ast::Expression::Assign { left, right, .. } => {
                    if should_remove_assignment(left, func, opt, ns) {
                        let mut params = SideEffectsCheckParameters {
                            cfg,
                            contract_no,
                            func: Some(func),
                            ns,
                            vartab,
                            opt,
                        };
                        right.recurse(&mut params, process_side_effects_expressions);

                        return;
                    }
                }
                ast::Expression::Builtin { args, .. } => {
                    // When array pop and push are top-level expressions, they can be removed
                    if should_remove_assignment(expr, func, opt, ns) {
                        let mut params = SideEffectsCheckParameters {
                            cfg,
                            contract_no,
                            func: Some(func),
                            ns,
                            vartab,
                            opt,
                        };
                        for arg in args {
                            arg.recurse(&mut params, process_side_effects_expressions);
                        }

                        return;
                    }
                }
                ast::Expression::TypeOperator { .. } => {
                    // just a stray type(int), no code to generate
                    return;
                }
                _ => (),
            }

            let _ = expression(expr, cfg, contract_no, Some(func), ns, vartab, opt);
        }
        Statement::Delete(_, ty, expr) => {
            let var_expr = expression(expr, cfg, contract_no, Some(func), ns, vartab, opt);

            cfg.add(
                vartab,
                Instr::ClearStorage {
                    ty: ty.clone(),
                    storage: var_expr,
                },
            );
        }
        Statement::Break(_) => {
            cfg.add(
                vartab,
                Instr::Branch {
                    block: loops.do_break(),
                },
            );
        }
        Statement::Continue(_) => {
            cfg.add(
                vartab,
                Instr::Branch {
                    block: loops.do_continue(),
                },
            );
        }
        Statement::If(_, _, cond, then_stmt, else_stmt) if else_stmt.is_empty() => {
            if_then(
                cond,
                then_stmt,
                func,
                cfg,
                contract_no,
                ns,
                vartab,
                loops,
                placeholder,
                return_override,
                opt,
            );
        }
        Statement::If(_, _, cond, then_stmt, else_stmt) => if_then_else(
            cond,
            then_stmt,
            else_stmt,
            func,
            cfg,
            contract_no,
            ns,
            vartab,
            loops,
            placeholder,
            return_override,
            opt,
        ),
        Statement::DoWhile(_, _, body_stmt, cond_expr) => {
            let body = cfg.new_basic_block("body".to_string());
            let cond = cfg.new_basic_block("conf".to_string());
            let end = cfg.new_basic_block("enddowhile".to_string());

            cfg.add(vartab, Instr::Branch { block: body });

            cfg.set_basic_block(body);

            vartab.new_dirty_tracker();
            loops.enter_scope(end, cond);

            let mut body_reachable = true;

            for stmt in body_stmt {
                statement(
                    stmt,
                    func,
                    cfg,
                    contract_no,
                    ns,
                    vartab,
                    loops,
                    placeholder,
                    return_override,
                    opt,
                );

                body_reachable = stmt.reachable();
            }

            if body_reachable {
                cfg.add(vartab, Instr::Branch { block: cond });
            }

            cfg.set_basic_block(cond);

            let cond_expr = expression(cond_expr, cfg, contract_no, Some(func), ns, vartab, opt);

            cfg.add(
                vartab,
                Instr::BranchCond {
                    cond: cond_expr,
                    true_block: body,
                    false_block: end,
                },
            );

            let set = vartab.pop_dirty_tracker();
            cfg.set_phis(end, set.clone());
            cfg.set_phis(body, set.clone());
            cfg.set_phis(cond, set);

            cfg.set_basic_block(end);
        }
        Statement::While(_, _, cond_expr, body_stmt) => {
            let cond = cfg.new_basic_block("cond".to_string());
            let body = cfg.new_basic_block("body".to_string());
            let end = cfg.new_basic_block("endwhile".to_string());

            cfg.add(vartab, Instr::Branch { block: cond });

            cfg.set_basic_block(cond);

            let cond_expr = expression(cond_expr, cfg, contract_no, Some(func), ns, vartab, opt);

            cfg.add(
                vartab,
                Instr::BranchCond {
                    cond: cond_expr,
                    true_block: body,
                    false_block: end,
                },
            );

            cfg.set_basic_block(body);

            vartab.new_dirty_tracker();
            loops.enter_scope(end, cond);

            let mut body_reachable = true;

            for stmt in body_stmt {
                statement(
                    stmt,
                    func,
                    cfg,
                    contract_no,
                    ns,
                    vartab,
                    loops,
                    placeholder,
                    return_override,
                    opt,
                );

                body_reachable = stmt.reachable();
            }

            if body_reachable {
                cfg.add(vartab, Instr::Branch { block: cond });
            }

            loops.leave_scope();
            let set = vartab.pop_dirty_tracker();
            cfg.set_phis(end, set.clone());
            cfg.set_phis(cond, set);

            cfg.set_basic_block(end);
        }
        Statement::For {
            init,
            cond: None,
            next,
            body,
            ..
        } => {
            let body_block = cfg.new_basic_block("body".to_string());
            let next_block = cfg.new_basic_block("next".to_string());
            let end_block = cfg.new_basic_block("endfor".to_string());

            for stmt in init {
                statement(
                    stmt,
                    func,
                    cfg,
                    contract_no,
                    ns,
                    vartab,
                    loops,
                    placeholder,
                    return_override,
                    opt,
                );
            }

            cfg.add(vartab, Instr::Branch { block: body_block });

            cfg.set_basic_block(body_block);

            loops.enter_scope(
                end_block,
                if next.is_none() {
                    body_block
                } else {
                    next_block
                },
            );

            vartab.new_dirty_tracker();

            let mut body_reachable = true;

            for stmt in body {
                statement(
                    stmt,
                    func,
                    cfg,
                    contract_no,
                    ns,
                    vartab,
                    loops,
                    placeholder,
                    return_override,
                    opt,
                );

                body_reachable = stmt.reachable();
            }

            if body_reachable {
                cfg.add(vartab, Instr::Branch { block: next_block });
            }

            loops.leave_scope();

            if body_reachable {
                cfg.set_basic_block(next_block);

                if let Some(next) = next {
                    expression(next, cfg, contract_no, Some(func), ns, vartab, opt);

                    body_reachable = next.ty() != Type::Unreachable;
                }

                if body_reachable {
                    cfg.add(vartab, Instr::Branch { block: body_block });
                }
            }

            let set = vartab.pop_dirty_tracker();

            cfg.set_phis(next_block, set.clone());
            cfg.set_phis(body_block, set.clone());
            cfg.set_phis(end_block, set);

            cfg.set_basic_block(end_block);
        }
        Statement::For {
            init,
            cond: Some(cond_expr),
            next,
            body,
            ..
        } => {
            let body_block = cfg.new_basic_block("body".to_string());
            let cond_block = cfg.new_basic_block("cond".to_string());
            let next_block = cfg.new_basic_block("next".to_string());
            let end_block = cfg.new_basic_block("endfor".to_string());

            for stmt in init {
                statement(
                    stmt,
                    func,
                    cfg,
                    contract_no,
                    ns,
                    vartab,
                    loops,
                    placeholder,
                    return_override,
                    opt,
                );
            }

            cfg.add(vartab, Instr::Branch { block: cond_block });

            cfg.set_basic_block(cond_block);

            let cond_expr = expression(cond_expr, cfg, contract_no, Some(func), ns, vartab, opt);

            cfg.add(
                vartab,
                Instr::BranchCond {
                    cond: cond_expr,
                    true_block: body_block,
                    false_block: end_block,
                },
            );

            cfg.set_basic_block(body_block);

            // continue goes to next
            loops.enter_scope(end_block, next_block);

            vartab.new_dirty_tracker();

            let mut body_reachable = true;

            for stmt in body {
                statement(
                    stmt,
                    func,
                    cfg,
                    contract_no,
                    ns,
                    vartab,
                    loops,
                    placeholder,
                    return_override,
                    opt,
                );

                body_reachable = stmt.reachable();
            }

            if body_reachable {
                cfg.add(vartab, Instr::Branch { block: next_block });
            }

            loops.leave_scope();

            cfg.set_basic_block(next_block);

            let mut next_reachable = true;

            if let Some(next) = next {
                expression(next, cfg, contract_no, Some(func), ns, vartab, opt);

                next_reachable = next.ty() != Type::Unreachable;
            }

            if next_reachable {
                cfg.add(vartab, Instr::Branch { block: cond_block });
            }

            cfg.set_basic_block(end_block);

            let set = vartab.pop_dirty_tracker();
            cfg.set_phis(next_block, set.clone());
            cfg.set_phis(end_block, set.clone());
            cfg.set_phis(cond_block, set);
        }
        Statement::Destructure(_, fields, expr) => {
            destructure(fields, expr, cfg, contract_no, func, ns, vartab, opt)
        }
        Statement::TryCatch(_, _, try_stmt) => self::try_catch::try_catch(
            try_stmt,
            func,
            cfg,
            contract_no,
            ns,
            vartab,
            loops,
            placeholder,
            return_override,
            opt,
        ),
        Statement::Emit {
            loc,
            event_no,
            args,
            ..
        } => {
            let emitter = new_event_emitter(loc, *event_no, args, ns);
            emitter.emit(contract_no, func, cfg, vartab, opt);
        }
        Statement::Revert {
            loc,
            args,
            error_no,
        } => {
            revert(
                args,
                error_no,
                cfg,
                contract_no,
                Some(func),
                ns,
                vartab,
                opt,
                loc,
            );
        }
        Statement::Underscore(_) => {
            // ensure we get phi nodes for the return values
            if let Some(instr @ Instr::Call { res, .. }) = placeholder {
                for var_no in res {
                    vartab.set_dirty(*var_no);
                }

                cfg.add(vartab, instr.clone());
            } else {
                panic!("placeholder should be provided for modifiers");
            }
        }

        Statement::Assembly(inline_assembly, ..) => {
            inline_assembly_cfg(inline_assembly, contract_no, ns, cfg, vartab, opt);
        }
    }
}

/// Generate if-then-no-else
fn if_then(
    cond: &ast::Expression,
    then_stmt: &[Statement],
    func: &Function,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    ns: &Namespace,
    vartab: &mut Vartable,
    loops: &mut LoopScopes,
    placeholder: Option<&Instr>,
    return_override: Option<&Instr>,
    opt: &Options,
) {
    let cond = expression(cond, cfg, contract_no, Some(func), ns, vartab, opt);

    let then = cfg.new_basic_block("then".to_string());
    let endif = cfg.new_basic_block("endif".to_string());

    cfg.add(
        vartab,
        Instr::BranchCond {
            cond,
            true_block: then,
            false_block: endif,
        },
    );

    cfg.set_basic_block(then);

    vartab.new_dirty_tracker();

    let mut reachable = true;

    for stmt in then_stmt {
        statement(
            stmt,
            func,
            cfg,
            contract_no,
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
        cfg.add(vartab, Instr::Branch { block: endif });
    }

    cfg.set_phis(endif, vartab.pop_dirty_tracker());

    cfg.set_basic_block(endif);
}

/// Generate if-then-else
fn if_then_else(
    cond: &ast::Expression,
    then_stmt: &[Statement],
    else_stmt: &[Statement],
    func: &Function,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    ns: &Namespace,
    vartab: &mut Vartable,
    loops: &mut LoopScopes,
    placeholder: Option<&Instr>,
    return_override: Option<&Instr>,
    opt: &Options,
) {
    let cond = expression(cond, cfg, contract_no, Some(func), ns, vartab, opt);

    let then = cfg.new_basic_block("then".to_string());
    let else_ = cfg.new_basic_block("else".to_string());
    let endif = cfg.new_basic_block("endif".to_string());

    cfg.add(
        vartab,
        Instr::BranchCond {
            cond,
            true_block: then,
            false_block: else_,
        },
    );

    // then
    cfg.set_basic_block(then);

    vartab.new_dirty_tracker();

    let mut then_reachable = true;

    for stmt in then_stmt {
        statement(
            stmt,
            func,
            cfg,
            contract_no,
            ns,
            vartab,
            loops,
            placeholder,
            return_override,
            opt,
        );

        then_reachable = stmt.reachable();
    }

    if then_reachable {
        cfg.add(vartab, Instr::Branch { block: endif });
    }

    // else
    cfg.set_basic_block(else_);

    let mut else_reachable = true;

    for stmt in else_stmt {
        statement(
            stmt,
            func,
            cfg,
            contract_no,
            ns,
            vartab,
            loops,
            placeholder,
            return_override,
            opt,
        );

        else_reachable = stmt.reachable();
    }

    if else_reachable {
        cfg.add(vartab, Instr::Branch { block: endif });
    }

    cfg.set_phis(endif, vartab.pop_dirty_tracker());

    cfg.set_basic_block(endif);
}

fn returns(
    expr: &ast::Expression,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: &Function,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
) {
    // Can only be another function call without returns
    let uncast_values = match expr {
        // Explicitly recurse for conditinal operator expressions.
        // `return a ? b : c` is transformed into pseudo code `a ? return b : return c`
        ast::Expression::ConditionalOperator {
            cond,
            true_option: left,
            false_option: right,
            ..
        } => {
            let cond = expression(cond, cfg, contract_no, Some(func), ns, vartab, opt);

            let left_block = cfg.new_basic_block("left".to_string());
            let right_block = cfg.new_basic_block("right".to_string());

            cfg.add(
                vartab,
                Instr::BranchCond {
                    cond,
                    true_block: left_block,
                    false_block: right_block,
                },
            );

            vartab.new_dirty_tracker();

            cfg.set_basic_block(left_block);
            returns(left, cfg, contract_no, func, ns, vartab, opt);

            cfg.set_basic_block(right_block);
            returns(right, cfg, contract_no, func, ns, vartab, opt);

            return;
        }

        ast::Expression::Builtin {
            kind: ast::Builtin::AbiDecode,
            ..
        }
        | ast::Expression::InternalFunctionCall { .. }
        | ast::Expression::ExternalFunctionCall { .. }
        | ast::Expression::ExternalFunctionCallRaw { .. } => {
            emit_function_call(expr, contract_no, cfg, Some(func), ns, vartab, opt)
        }

        ast::Expression::List { list, .. } => list
            .iter()
            .map(|e| expression(e, cfg, contract_no, Some(func), ns, vartab, opt))
            .collect::<Vec<Expression>>(),

        // Can be any other expression
        _ => {
            vec![expression(
                expr,
                cfg,
                contract_no,
                Some(func),
                ns,
                vartab,
                opt,
            )]
        }
    };

    let cast_values = func
        .returns
        .iter()
        .zip(uncast_values)
        .map(|(left, right)| try_load_and_cast(&right.loc(), &right, &left.ty, ns, cfg, vartab))
        .collect();

    cfg.add(vartab, Instr::Return { value: cast_values });
}

fn destructure(
    fields: &[DestructureField],
    expr: &ast::Expression,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: &Function,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
) {
    if let ast::Expression::ConditionalOperator {
        cond,
        true_option: left,
        false_option: right,
        ..
    } = expr
    {
        let cond = expression(cond, cfg, contract_no, Some(func), ns, vartab, opt);

        let left_block = cfg.new_basic_block("left".to_string());
        let right_block = cfg.new_basic_block("right".to_string());
        let done_block = cfg.new_basic_block("done".to_string());

        cfg.add(
            vartab,
            Instr::BranchCond {
                cond,
                true_block: left_block,
                false_block: right_block,
            },
        );

        vartab.new_dirty_tracker();

        cfg.set_basic_block(left_block);

        destructure(fields, left, cfg, contract_no, func, ns, vartab, opt);

        cfg.add(vartab, Instr::Branch { block: done_block });

        cfg.set_basic_block(right_block);

        destructure(fields, right, cfg, contract_no, func, ns, vartab, opt);

        cfg.add(vartab, Instr::Branch { block: done_block });

        cfg.set_phis(done_block, vartab.pop_dirty_tracker());

        cfg.set_basic_block(done_block);

        return;
    }

    let mut values = match expr {
        ast::Expression::List { list, .. } => {
            let mut values = Vec::new();

            for expr in list {
                let loc = expr.loc();
                let expr = expression(expr, cfg, contract_no, Some(func), ns, vartab, opt);
                let ty = expr.ty();

                let res = vartab.temp_anonymous(&ty);

                cfg.add(vartab, Instr::Set { loc, res, expr });

                values.push(Expression::Variable {
                    loc,
                    ty,
                    var_no: res,
                });
            }

            values
        }
        _ => {
            // must be function call, either internal or external
            emit_function_call(expr, contract_no, cfg, Some(func), ns, vartab, opt)
        }
    };

    for field in fields.iter() {
        let right = values.remove(0);

        match field {
            DestructureField::None => {
                // nothing to do
            }
            DestructureField::VariableDecl(res, param) => {
                let expr = try_load_and_cast(&param.loc, &right, &param.ty, ns, cfg, vartab);

                if should_remove_variable(*res, func, opt, ns) {
                    continue;
                }

                cfg.add(
                    vartab,
                    Instr::Set {
                        loc: param.loc,
                        res: *res,
                        expr,
                    },
                );
            }
            DestructureField::Expression(left) => {
                let expr = try_load_and_cast(&left.loc(), &right, &left.ty(), ns, cfg, vartab);

                if should_remove_assignment(left, func, opt, ns) {
                    continue;
                }

                assign_single(left, expr, cfg, contract_no, Some(func), ns, vartab, opt);
            }
        }
    }
}

/// During a destructure statement, sema only checks if the cast is possible. During codegen, we
/// perform the real cast and add an instruction to the CFG to load a value from the storage if want it.
/// The existing codegen cast function does not manage the CFG, so the loads must be done here.
fn try_load_and_cast(
    loc: &pt::Loc,
    expr: &Expression,
    to_ty: &Type,
    ns: &Namespace,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
) -> Expression {
    match expr.ty() {
        Type::StorageRef(_, ty) => {
            if let Expression::Subscript { array_ty, .. } = &expr {
                if array_ty.is_storage_bytes() {
                    return expr.cast(to_ty, ns);
                }
            }

            if matches!(to_ty, Type::StorageRef(..)) {
                // If we want a storage reference, there is no need to load from storage
                return expr.cast(to_ty, ns);
            }

            let anonymous_no = vartab.temp_anonymous(&ty);
            cfg.add(
                vartab,
                Instr::LoadStorage {
                    res: anonymous_no,
                    ty: (*ty).clone(),
                    storage: expr.cast(to_ty, ns),
                    storage_type: None,
                },
            );

            Expression::Variable {
                loc: *loc,
                ty: (*ty).clone(),
                var_no: anonymous_no,
            }
        }
        Type::Ref(ty) => match *ty {
            Type::Array(_, _) => expr.cast(to_ty, ns),
            _ => Expression::Load {
                loc: pt::Loc::Builtin,
                ty: *ty,
                expr: expr.clone().into(),
            }
            .cast(to_ty, ns),
        },
        _ => expr.cast(to_ty, ns),
    }
}

pub struct LoopScope {
    break_bb: usize,
    continue_bb: usize,
}

pub struct LoopScopes(Vec<LoopScope>);

impl LoopScopes {
    pub fn new() -> Self {
        LoopScopes(Vec::new())
    }

    pub(crate) fn enter_scope(&mut self, break_bb: usize, continue_bb: usize) {
        self.0.push(LoopScope {
            break_bb,
            continue_bb,
        })
    }

    pub(crate) fn leave_scope(&mut self) -> LoopScope {
        self.0.pop().expect("should be in loop scope")
    }

    pub(crate) fn do_break(&mut self) -> usize {
        self.0.last().unwrap().break_bb
    }

    pub(crate) fn do_continue(&mut self) -> usize {
        self.0.last().unwrap().continue_bb
    }
}

impl Type {
    /// Default value for a type, e.g. an empty string. Some types cannot have a default value,
    /// for example a reference to a variable in storage.
    pub fn default(&self, ns: &Namespace) -> Option<Expression> {
        match self {
            Type::Address(_) | Uint(_) | Type::Int(_) => Some(Expression::NumberLiteral {
                loc: Codegen,
                ty: self.clone(),
                value: BigInt::from(0),
            }),
            Type::Bool => Some(Expression::BoolLiteral {
                loc: Codegen,
                value: false,
            }),
            Type::Bytes(n) => {
                let l = vec![0; *n as usize];
                Some(Expression::BytesLiteral {
                    loc: Codegen,
                    ty: self.clone(),
                    value: l,
                })
            }
            Type::Enum(e) => ns.enums[*e].ty.default(ns),
            Type::Struct(struct_ty) => {
                // make sure all our fields have default values
                for field in &struct_ty.definition(ns).fields {
                    field.ty.default(ns)?;
                }

                Some(Expression::StructLiteral {
                    loc: Codegen,
                    ty: self.clone(),
                    values: Vec::new(),
                })
            }
            Type::Ref(ty) => {
                assert!(matches!(ty.as_ref(), Type::Address(_)));

                Some(Expression::GetRef {
                    loc: Codegen,
                    ty: Type::Ref(Box::new(ty.as_ref().clone())),
                    expr: Box::new(Expression::NumberLiteral {
                        loc: Codegen,
                        ty: ty.as_ref().clone(),
                        value: BigInt::from(0),
                    }),
                })
            }
            Type::StorageRef(..) => None,
            Type::String | Type::DynamicBytes => Some(Expression::AllocDynamicBytes {
                loc: Codegen,
                ty: self.clone(),
                size: Box::new(Expression::NumberLiteral {
                    loc: Codegen,
                    ty: Uint(32),
                    value: BigInt::zero(),
                }),
                initializer: None,
            }),
            Type::InternalFunction { .. } | Type::Contract(_) | Type::ExternalFunction { .. } => {
                None
            }
            Type::Array(ty, dims) => {
                ty.default(ns)?;

                if dims.last() == Some(&ArrayLength::Dynamic) {
                    Some(Expression::AllocDynamicBytes {
                        loc: Codegen,
                        ty: self.clone(),
                        size: Box::new(Expression::NumberLiteral {
                            loc: Codegen,
                            ty: Uint(32),
                            value: BigInt::zero(),
                        }),
                        initializer: None,
                    })
                } else {
                    Some(Expression::ArrayLiteral {
                        loc: Codegen,
                        ty: self.clone(),
                        dimensions: Vec::new(),
                        values: Vec::new(),
                    })
                }
            }
            _ => None,
        }
    }
}

impl Namespace {
    /// Phoney default constructor
    pub fn default_constructor(&self, contract_no: usize) -> Function {
        let mut func = Function::new(
            Codegen,
            Codegen,
            pt::Identifier {
                name: "".to_owned(),
                loc: Codegen,
            },
            Some(contract_no),
            vec![],
            pt::FunctionTy::Constructor,
            None,
            pt::Visibility::Public(None),
            Vec::new(),
            Vec::new(),
            self,
        );

        func.body = vec![Statement::Return(Codegen, None)];
        func.has_body = true;
        func.solana_accounts.borrow_mut().insert(
            BuiltinAccounts::DataAccount.to_string(),
            SolanaAccount {
                loc: Loc::Codegen,
                is_signer: false,
                is_writer: true,
                generated: true,
            },
        );

        func
    }
}

/// This function looks for expressions that have side effects during code execution and
/// processes them.
/// They must be added to the cfg event if we remove the assignment
pub fn process_side_effects_expressions(
    exp: &ast::Expression,
    ctx: &mut SideEffectsCheckParameters,
) -> bool {
    match &exp {
        ast::Expression::InternalFunctionCall { .. }
        | ast::Expression::ExternalFunctionCall { .. }
        | ast::Expression::ExternalFunctionCallRaw { .. }
        | ast::Expression::Constructor { .. }
        | ast::Expression::Assign { .. } => {
            let _ = expression(
                exp,
                ctx.cfg,
                ctx.contract_no,
                ctx.func,
                ctx.ns,
                ctx.vartab,
                ctx.opt,
            );
            false
        }

        ast::Expression::Builtin {
            kind: ast::Builtin::PayableSend
            | ast::Builtin::ArrayPush
            | ast::Builtin::ArrayPop
            // PayableTransfer, Revert, Require and SelfDestruct do not occur inside an expression
            // for they return no value. They should not bother the unused variable elimination.
            | ast::Builtin::PayableTransfer
            | ast::Builtin::Require
            | ast::Builtin::SelfDestruct
            | ast::Builtin::WriteInt8
            | ast::Builtin::WriteInt16LE
            | ast::Builtin::WriteInt32LE
            | ast::Builtin::WriteInt64LE
            | ast::Builtin::WriteInt128LE
            | ast::Builtin::WriteInt256LE
            | ast::Builtin::WriteUint16LE
            | ast::Builtin::WriteUint32LE
            | ast::Builtin::WriteUint64LE
            | ast::Builtin::WriteUint128LE
            | ast::Builtin::WriteUint256LE
            | ast::Builtin::WriteAddress, ..
        } =>  {
                let _ = expression(exp, ctx.cfg, ctx.contract_no, ctx.func, ctx.ns, ctx.vartab, ctx.opt);
                false
        },

        _ => true,
    }
}
