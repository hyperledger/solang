// SPDX-License-Identifier: Apache-2.0

use super::ast::*;
use super::contracts::is_base;
use super::diagnostics::Diagnostics;
use super::expression::{
    function_call::{available_functions, call_expr, named_call_expr},
    ExprContext, ResolveTo,
};
use super::symtable::Symtable;
use crate::sema::expression::constructor::{
    constructor_named_args, match_constructor_to_args, new,
};
use crate::sema::expression::function_call::{
    function_call_expr, function_call_pos_args, named_function_call_expr,
};
use crate::sema::expression::resolve_expression::expression;
use crate::sema::function_annotation::function_body_annotations;
use crate::sema::function_annotation::{unexpected_parameter_annotation, UnresolvedAnnotation};
use crate::sema::namespace::ResolveTypeContext;
use crate::sema::symtable::{VariableInitializer, VariableUsage};
use crate::sema::unused_variable::{assigned_variable, check_function_call, used_variable};
use crate::sema::yul::resolve_inline_assembly;
use crate::sema::Recurse;
use crate::Target;
use solang_parser::pt;
use solang_parser::pt::CatchClause;
use solang_parser::pt::CodeLocation;
use solang_parser::pt::OptionalCodeLocation;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

pub fn resolve_function_body(
    def: &pt::FunctionDefinition,
    annotations: &[&pt::Annotation],
    file_no: usize,
    contract_no: Option<usize>,
    function_no: usize,
    ns: &mut Namespace,
) -> Result<(), ()> {
    let mut symtable = Symtable::default();
    let mut res = Vec::new();
    let mut context = ExprContext {
        file_no,
        contract_no,
        function_no: Some(function_no),
        ambiguous_emit: ns.solidity_minor_version(file_no, 5),
        ..Default::default()
    };
    context.enter_scope();

    let mut unresolved_annotation: Vec<UnresolvedAnnotation> = Vec::new();
    // first add function parameters
    for (i, p) in def.params.iter().enumerate() {
        let p = p.1.as_ref().unwrap();
        if let Some(ref name) = p.name {
            if let Some(pos) = symtable.add(
                name,
                ns.functions[function_no].params[i].ty.clone(),
                ns,
                VariableInitializer::Solidity(None),
                VariableUsage::Parameter,
                p.storage.clone(),
                &mut context,
            ) {
                ns.check_shadowing(file_no, contract_no, name);
                if p.annotation.is_some() {
                    unresolved_annotation.push(UnresolvedAnnotation {
                        parameter_no: i,
                        var_no: pos,
                    });
                }
                symtable.arguments.push(Some(pos));
            }
        } else {
            symtable.arguments.push(None);
        }
    }

    function_body_annotations(
        function_no,
        annotations,
        &unresolved_annotation,
        &mut symtable,
        &mut context,
        ns,
    );

    // now that the function arguments have been resolved, we can resolve the bases for
    // constructors.
    if def.ty == pt::FunctionTy::Constructor {
        let contract_no = contract_no.unwrap();
        let mut resolve_bases: BTreeMap<usize, pt::Loc> = BTreeMap::new();
        let mut all_ok = true;
        let mut diagnostics = Diagnostics::default();

        for attr in &def.attributes {
            if let pt::FunctionAttribute::BaseOrModifier(loc, base) = attr {
                match ns.resolve_contract_with_namespace(file_no, &base.name, &mut diagnostics) {
                    Ok(base_no) => {
                        if base_no == contract_no || !is_base(base_no, contract_no, ns) {
                            ns.diagnostics.push(Diagnostic::error(
                                *loc,
                                format!(
                                    "contract '{}' is not a base contract of '{}'",
                                    base.name, ns.contracts[contract_no].id,
                                ),
                            ));
                            all_ok = false;
                        } else if let Some(prev) = resolve_bases.get(&base_no) {
                            ns.diagnostics.push(Diagnostic::error_with_note(
                                *loc,
                                format!("duplicate base contract '{}'", base.name),
                                *prev,
                                format!("previous base contract '{}'", base.name),
                            ));
                            all_ok = false;
                        } else if let Some(args) = &base.args {
                            let mut diagnostics = Diagnostics::default();

                            // find constructor which matches this
                            if let Ok((Some(constructor_no), args)) = match_constructor_to_args(
                                &base.loc,
                                args,
                                base_no,
                                &mut context,
                                ns,
                                &mut symtable,
                                &mut diagnostics,
                            ) {
                                for arg in &args {
                                    used_variable(ns, arg, &mut symtable);
                                }
                                ns.functions[function_no]
                                    .bases
                                    .insert(base_no, (base.loc, constructor_no, args));

                                resolve_bases.insert(base_no, base.loc);
                            }

                            ns.diagnostics.extend(diagnostics);
                        } else {
                            ns.diagnostics.push(Diagnostic::error(
                                *loc,
                                format!(
                                    "missing arguments to constructor of contract '{}'",
                                    base.name
                                ),
                            ));
                            all_ok = false;
                        }
                    }
                    Err(_) => {
                        all_ok = false;
                    }
                }
            }
        }

        if all_ok && ns.contracts[contract_no].instantiable {
            for base in &ns.contracts[contract_no].bases {
                // do we have constructor arguments
                if base.constructor.is_some() || resolve_bases.contains_key(&base.contract_no) {
                    continue;
                }

                // does the contract require arguments
                if ns.contracts[base.contract_no].constructor_needs_arguments(ns) {
                    ns.diagnostics.push(Diagnostic::error(
                        def.loc_prototype,
                        format!(
                            "missing arguments to contract '{}' constructor",
                            ns.contracts[base.contract_no].id
                        ),
                    ));
                }
            }
        }

        ns.diagnostics.extend(diagnostics);
    }

    // resolve modifiers on functions
    if def.ty == pt::FunctionTy::Function {
        let mut modifiers = Vec::new();
        let mut diagnostics = Diagnostics::default();

        for attr in &def.attributes {
            if let pt::FunctionAttribute::BaseOrModifier(_, modifier) = attr {
                if modifier.name.identifiers.len() != 1 {
                    ns.diagnostics.push(Diagnostic::error(
                        def.loc_prototype,
                        format!("unknown modifier '{}' on function", modifier.name),
                    ));
                } else {
                    let modifier_name = &modifier.name.identifiers[0];
                    if let Ok(e) = function_call_pos_args(
                        &modifier.loc,
                        &modifier.name,
                        pt::FunctionTy::Modifier,
                        modifier.args.as_ref().unwrap_or(&Vec::new()),
                        available_functions(
                            &modifier_name.name,
                            false,
                            context.file_no,
                            context.contract_no,
                            ns,
                        ),
                        true,
                        &mut context,
                        ns,
                        ResolveTo::Unknown,
                        &mut symtable,
                        &mut diagnostics,
                    ) {
                        modifiers.push(e);
                    }
                }
            }
        }

        ns.diagnostics.extend(diagnostics);
        ns.functions[function_no].modifiers = modifiers;
    }

    // If there is no return statement, any unnamed return types will
    // implicitly be 0. If there is a return type which is a storage
    // reference (e.g. int[] storage), a value must be given via
    // a return statement, else the value will not refer to valid storage,
    // as 0 is almost certainly an invalid storage key.
    let mut return_required = false;

    // If any of the return values are named, then the return statement can be omitted at
    // the end of the function, and return values may be omitted too. Create variables to
    // store the return values
    for (i, p) in def.returns.iter().enumerate() {
        let ret = &ns.functions[function_no].returns[i];

        if let Some(ref name) = p.1.as_ref().unwrap().name {
            if let Some(pos) = symtable.add(
                name,
                ret.ty.clone(),
                ns,
                VariableInitializer::Solidity(None),
                VariableUsage::ReturnVariable,
                p.1.as_ref().unwrap().storage.clone(),
                &mut context,
            ) {
                ns.check_shadowing(file_no, contract_no, name);
                symtable.returns.push(pos);
            }
        } else {
            if ret.ty.is_contract_storage() {
                return_required = true;
            }

            // anonymous return
            let id = pt::Identifier {
                loc: p.0,
                name: "".to_owned(),
            };

            let pos = symtable
                .add(
                    &id,
                    ret.ty.clone(),
                    ns,
                    VariableInitializer::Solidity(None),
                    VariableUsage::AnonymousReturnVariable,
                    p.1.as_ref().unwrap().storage.clone(),
                    &mut context,
                )
                .unwrap();

            symtable.returns.push(pos);
        }
    }

    let body = match def.body {
        None => return Ok(()),
        Some(ref body) => body,
    };

    let mut diagnostics = Diagnostics::default();

    let reachable = statement(
        body,
        &mut res,
        &mut context,
        &mut symtable,
        ns,
        &mut diagnostics,
    );

    ns.diagnostics.extend(diagnostics);

    if reachable? && return_required {
        for param in ns.functions[function_no].returns.iter() {
            if param.id.is_none() && param.ty.is_contract_storage() {
                ns.diagnostics.push(Diagnostic::error(
                    param.loc,
                    "storage reference must be given value with a return statement".to_string(),
                ));
            }
        }
    }

    if def.ty == pt::FunctionTy::Modifier {
        let mut has_underscore = false;

        // unsure modifier has underscore
        fn check_statement(stmt: &Statement, has_underscore: &mut bool) -> bool {
            if stmt.is_underscore() {
                *has_underscore = true;
                false
            } else {
                true
            }
        }

        for stmt in &mut res {
            stmt.recurse(&mut has_underscore, check_statement);
        }

        if !has_underscore {
            ns.diagnostics.push(Diagnostic::error(
                body.loc().end_range(),
                "missing '_' in modifier".to_string(),
            ));
        }
    }

    context.leave_scope(&mut symtable, ns.functions[function_no].loc);

    ns.functions[function_no].body = res;

    std::mem::swap(&mut ns.functions[function_no].symtable, &mut symtable);

    Ok(())
}

/// Resolve a statement
fn statement(
    stmt: &pt::Statement,
    res: &mut Vec<Statement>,
    context: &mut ExprContext,
    symtable: &mut Symtable,
    ns: &mut Namespace,
    diagnostics: &mut Diagnostics,
) -> Result<bool, ()> {
    let function_no = context.function_no.unwrap();

    match stmt {
        pt::Statement::VariableDefinition(loc, decl, initializer) => {
            let (var_ty, ty_loc) =
                resolve_var_decl_ty(&decl.ty, &decl.storage, context, ns, diagnostics)?;

            let initializer = if let Some(init) = initializer {
                let expr = expression(
                    init,
                    context,
                    ns,
                    symtable,
                    diagnostics,
                    ResolveTo::Type(&var_ty),
                )?;

                expr.check_constant_overflow(diagnostics);

                used_variable(ns, &expr, symtable);

                Some(Arc::new(expr.cast(
                    &expr.loc(),
                    &var_ty,
                    true,
                    ns,
                    diagnostics,
                )?))
            } else {
                None
            };

            if let Some(pos) = symtable.add(
                decl.name.as_ref().unwrap(),
                var_ty.clone(),
                ns,
                VariableInitializer::Solidity(initializer.clone()),
                VariableUsage::LocalVariable,
                decl.storage.clone(),
                context,
            ) {
                ns.check_shadowing(
                    context.file_no,
                    context.contract_no,
                    decl.name.as_ref().unwrap(),
                );

                res.push(Statement::VariableDecl(
                    *loc,
                    pos,
                    Parameter {
                        loc: decl.loc,
                        ty: var_ty,
                        ty_loc: Some(ty_loc),
                        id: Some(decl.name.clone().unwrap()),
                        indexed: false,
                        readonly: false,
                        infinite_size: false,
                        recursive: false,
                        annotation: None,
                    },
                    initializer,
                ));
            }

            Ok(true)
        }
        pt::Statement::Block {
            statements,
            unchecked,
            loc,
        } => {
            context.enter_scope();
            let mut reachable = true;
            let mut already_unreachable = false;

            let prev_unchecked = context.unchecked;
            context.unchecked |= *unchecked;

            let mut context = scopeguard::guard(context, |context| {
                context.unchecked = prev_unchecked;
            });

            let mut resolved_stmts = Vec::new();

            for stmt in statements {
                if !reachable && !already_unreachable {
                    ns.diagnostics.push(Diagnostic::warning(
                        stmt.loc(),
                        "unreachable statement".to_string(),
                    ));
                    already_unreachable = true;
                }
                reachable = statement(
                    stmt,
                    &mut resolved_stmts,
                    &mut context,
                    symtable,
                    ns,
                    diagnostics,
                )?;
            }

            context.leave_scope(symtable, *loc);

            res.push(Statement::Block {
                loc: *loc,
                unchecked: *unchecked,
                statements: resolved_stmts,
            });

            Ok(reachable)
        }
        pt::Statement::Break(loc) => {
            if context.loops.do_break() {
                res.push(Statement::Break(*loc));
                Ok(false)
            } else {
                diagnostics.push(Diagnostic::error(
                    stmt.loc(),
                    "break statement not in loop".to_string(),
                ));
                Err(())
            }
        }
        pt::Statement::Continue(loc) => {
            if context.loops.do_continue() {
                res.push(Statement::Continue(*loc));
                Ok(false)
            } else {
                diagnostics.push(Diagnostic::error(
                    stmt.loc(),
                    "continue statement not in loop".to_string(),
                ));
                Err(())
            }
        }
        pt::Statement::While(loc, cond_expr, body) => {
            let expr = expression(
                cond_expr,
                context,
                ns,
                symtable,
                diagnostics,
                ResolveTo::Type(&Type::Bool),
            )?;
            used_variable(ns, &expr, symtable);
            let cond = expr.cast(&expr.loc(), &Type::Bool, true, ns, diagnostics)?;

            context.enter_scope();
            let mut body_stmts = Vec::new();
            context.loops.enter_scope();
            statement(body, &mut body_stmts, context, symtable, ns, diagnostics)?;
            context.leave_scope(symtable, *loc);
            context.loops.leave_scope();

            res.push(Statement::While(*loc, true, cond, body_stmts));
            Ok(true)
        }
        pt::Statement::DoWhile(loc, body, cond_expr) => {
            let expr = expression(
                cond_expr,
                context,
                ns,
                symtable,
                diagnostics,
                ResolveTo::Type(&Type::Bool),
            )?;
            used_variable(ns, &expr, symtable);
            let cond = expr.cast(&expr.loc(), &Type::Bool, true, ns, diagnostics)?;

            context.enter_scope();
            let mut body_stmts = Vec::new();
            context.loops.enter_scope();
            statement(body, &mut body_stmts, context, symtable, ns, diagnostics)?;
            context.leave_scope(symtable, *loc);
            context.loops.leave_scope();

            res.push(Statement::DoWhile(*loc, true, body_stmts, cond));
            Ok(true)
        }
        pt::Statement::If(loc, cond_expr, then, else_) => {
            let expr = expression(
                cond_expr,
                context,
                ns,
                symtable,
                diagnostics,
                ResolveTo::Type(&Type::Bool),
            )?;
            used_variable(ns, &expr, symtable);

            let cond = expr.cast(&expr.loc(), &Type::Bool, true, ns, diagnostics)?;

            context.enter_scope();
            let mut then_stmts = Vec::new();
            let mut reachable =
                statement(then, &mut then_stmts, context, symtable, ns, diagnostics)?;
            context.leave_scope(symtable, *loc);

            let mut else_stmts = Vec::new();
            if let Some(stmts) = else_ {
                context.enter_scope();
                reachable |= statement(stmts, &mut else_stmts, context, symtable, ns, diagnostics)?;

                context.leave_scope(symtable, *loc);
            } else {
                reachable = true;
            }

            res.push(Statement::If(*loc, reachable, cond, then_stmts, else_stmts));

            Ok(reachable)
        }
        pt::Statement::Args(loc, _) => {
            ns.diagnostics.push(Diagnostic::error(
                *loc,
                "expected code block, not list of named arguments".to_string(),
            ));
            Err(())
        }
        pt::Statement::For(loc, init_stmt, None, next_expr, body_stmt) => {
            context.enter_scope();

            let mut init = Vec::new();

            if let Some(init_stmt) = init_stmt {
                statement(init_stmt, &mut init, context, symtable, ns, diagnostics)?;
            }

            context.loops.enter_scope();

            let mut body = Vec::new();

            if let Some(body_stmt) = body_stmt {
                statement(body_stmt, &mut body, context, symtable, ns, diagnostics)?;
            }

            let control = context.loops.leave_scope();
            let reachable = control.no_breaks > 0;
            let mut next = None;

            if let Some(next_expr) = next_expr {
                next = Some(expression(
                    next_expr,
                    context,
                    ns,
                    symtable,
                    diagnostics,
                    ResolveTo::Type(&Type::Bool),
                )?);
            }

            context.leave_scope(symtable, *loc);

            res.push(Statement::For {
                loc: *loc,
                reachable,
                init,
                next,
                cond: None,
                body,
            });
            Ok(reachable)
        }
        pt::Statement::For(loc, init_stmt, Some(cond_expr), next_expr, body_stmt) => {
            context.enter_scope();

            let mut init = Vec::new();
            let mut body = Vec::new();
            let mut next = None;

            if let Some(init_stmt) = init_stmt {
                statement(init_stmt, &mut init, context, symtable, ns, diagnostics)?;
            }

            let cond = expression(
                cond_expr,
                context,
                ns,
                symtable,
                diagnostics,
                ResolveTo::Type(&Type::Bool),
            )?;

            let cond = cond.cast(&cond_expr.loc(), &Type::Bool, true, ns, diagnostics)?;

            // continue goes to next, and if that does exist, cond
            context.loops.enter_scope();

            let mut body_reachable = match body_stmt {
                Some(body_stmt) => {
                    statement(body_stmt, &mut body, context, symtable, ns, diagnostics)?
                }
                None => true,
            };

            let control = context.loops.leave_scope();

            if control.no_continues > 0 {
                body_reachable = true;
            }

            if let Some(next_expr) = next_expr {
                if body_reachable {
                    next = Some(expression(
                        next_expr,
                        context,
                        ns,
                        symtable,
                        diagnostics,
                        ResolveTo::Type(&Type::Bool),
                    )?);
                }
            }

            context.leave_scope(symtable, *loc);

            res.push(Statement::For {
                loc: *loc,
                reachable: true,
                init,
                next,
                cond: Some(cond),
                body,
            });

            Ok(true)
        }
        pt::Statement::Return(loc, None) => {
            let no_returns = ns.functions[context.function_no.unwrap()].returns.len();

            if symtable.returns.len() != no_returns {
                ns.diagnostics.push(Diagnostic::error(
                    *loc,
                    format!("missing return value, {no_returns} return values expected"),
                ));
                return Err(());
            }

            res.push(Statement::Return(*loc, None));

            Ok(false)
        }
        pt::Statement::Return(loc, Some(returns)) => {
            let expr = return_with_values(returns, loc, context, symtable, ns, diagnostics)?;

            expr.check_constant_overflow(diagnostics);

            for offset in symtable.returns.iter() {
                let elem = symtable.vars.get_mut(offset).unwrap();
                elem.assigned = true;
            }

            res.push(Statement::Return(*loc, Some(expr)));

            Ok(false)
        }
        pt::Statement::Expression(loc, expr) => {
            let expr = match expr {
                // delete statement
                pt::Expression::Delete(_, expr) => {
                    let expr =
                        expression(expr, context, ns, symtable, diagnostics, ResolveTo::Unknown)?;
                    used_variable(ns, &expr, symtable);
                    return if let Type::StorageRef(_, ty) = expr.ty() {
                        if expr.ty().is_mapping() {
                            ns.diagnostics.push(Diagnostic::error(
                                *loc,
                                "'delete' cannot be applied to mapping type".to_string(),
                            ));
                            return Err(());
                        }

                        res.push(Statement::Delete(*loc, ty.as_ref().clone(), expr));

                        Ok(true)
                    } else {
                        ns.diagnostics.push(Diagnostic::warning(
                            *loc,
                            "argument to 'delete' should be storage reference".to_string(),
                        ));

                        Err(())
                    };
                }
                // is it an underscore modifier statement
                pt::Expression::Variable(id)
                    if id.name == "_"
                        && ns.functions[function_no].ty == pt::FunctionTy::Modifier =>
                {
                    res.push(Statement::Underscore(*loc));
                    return Ok(true);
                }
                pt::Expression::FunctionCall(loc, ty, args) => {
                    let ret = call_expr(
                        loc,
                        ty,
                        args,
                        true,
                        context,
                        ns,
                        symtable,
                        diagnostics,
                        ResolveTo::Discard,
                    )?;

                    ret.check_constant_overflow(diagnostics);
                    ret
                }
                pt::Expression::NamedFunctionCall(loc, ty, args) => {
                    let ret = named_call_expr(
                        loc,
                        ty,
                        args,
                        true,
                        context,
                        ns,
                        symtable,
                        diagnostics,
                        ResolveTo::Discard,
                    )?;
                    ret.check_constant_overflow(diagnostics);
                    ret
                }
                _ => {
                    // is it a destructure statement
                    if let pt::Expression::Assign(_, var, expr) = expr {
                        if let pt::Expression::List(_, var) = var.as_ref() {
                            res.push(destructure(
                                loc,
                                var,
                                expr,
                                context,
                                symtable,
                                ns,
                                diagnostics,
                            )?);

                            // if a noreturn function was called, then the destructure would not resolve
                            return Ok(true);
                        }
                    }
                    // the rest. We don't care about the result
                    expression(expr, context, ns, symtable, diagnostics, ResolveTo::Unknown)?
                }
            };

            let reachable = expr.tys() != vec![Type::Unreachable];

            res.push(Statement::Expression(*loc, reachable, expr));

            Ok(reachable)
        }
        pt::Statement::Try(loc, expr, returns_and_ok, clause_stmts) => {
            let (stmt, reachable) = try_catch(
                loc,
                expr,
                returns_and_ok,
                clause_stmts,
                context,
                symtable,
                ns,
                diagnostics,
            )?;
            res.push(stmt);

            Ok(reachable)
        }
        pt::Statement::Emit(loc, ty) => {
            if let Ok(emit) = emit_event(loc, ty, context, symtable, ns, diagnostics) {
                res.push(emit);
            }

            Ok(true)
        }
        pt::Statement::Assembly {
            loc,
            dialect,
            flags,
            block,
        } => {
            if dialect.is_some() && dialect.as_ref().unwrap().string != "evmasm" {
                ns.diagnostics.push(Diagnostic::error(
                    dialect.as_ref().unwrap().loc,
                    "only evmasm dialect is supported".to_string(),
                ));
                return Err(());
            }

            let mut memory_safe = None;

            if let Some(flags) = flags {
                for flag in flags {
                    if flag.string == "memory-safe" && ns.target == Target::EVM {
                        if let Some(prev) = &memory_safe {
                            ns.diagnostics.push(Diagnostic::warning_with_note(
                                flag.loc,
                                format!("flag '{}' already specified", flag.string),
                                *prev,
                                "previous location".into(),
                            ));
                        } else {
                            memory_safe = Some(flag.loc);
                        }
                    } else {
                        ns.diagnostics.push(Diagnostic::warning(
                            flag.loc,
                            format!("flag '{}' not supported", flag.string),
                        ));
                    }
                }
            }

            let resolved_asm = resolve_inline_assembly(
                loc,
                memory_safe.is_some(),
                &block.statements,
                context,
                symtable,
                ns,
            );
            res.push(Statement::Assembly(resolved_asm.0, resolved_asm.1));
            Ok(resolved_asm.1)
        }
        pt::Statement::Revert(loc, path, args) => {
            if let Ok(stmt) = revert_pos_arg(loc, path, args, context, symtable, diagnostics, ns) {
                res.push(stmt);
            }

            Ok(false)
        }
        pt::Statement::RevertNamedArgs(loc, path, args) => {
            if let Ok(stmt) = revert_named_arg(loc, path, args, context, symtable, diagnostics, ns)
            {
                res.push(stmt);
            }
            Ok(false)
        }
        pt::Statement::Error(_) => unimplemented!(),
    }
}

/// Resolve a revert statement with position arguments, and optional error, e.g.
/// ```ignore
/// revert();
/// revert("the reason why");
/// revert NotEnoughBalance(address);
/// ```
fn revert_pos_arg(
    loc: &pt::Loc,
    path: &Option<pt::IdentifierPath>,
    args: &[pt::Expression],
    context: &mut ExprContext,
    symtable: &mut Symtable,
    diagnostics: &mut Diagnostics,
    ns: &mut Namespace,
) -> Result<Statement, ()> {
    if let Some(path) = path {
        let error_no = ns.resolve_error(context.file_no, context.contract_no, path, diagnostics)?;

        let mut arguments = Vec::new();

        for (pos, arg) in args.iter().enumerate() {
            let error = &ns.errors[error_no];

            let ty = if let Some(field) = error.fields.get(pos) {
                field.ty.clone()
            } else {
                let _ = expression(arg, context, ns, symtable, diagnostics, ResolveTo::Unknown);

                continue;
            };

            let arg = match expression(
                arg,
                context,
                ns,
                symtable,
                diagnostics,
                ResolveTo::Type(&ty),
            ) {
                Ok(e) => e,
                Err(()) => {
                    continue;
                }
            };

            match arg.cast(&arg.loc(), &ty, true, ns, diagnostics) {
                Ok(expr) => {
                    arguments.push(expr);
                }
                Err(()) => {
                    continue;
                }
            }
        }

        let error = &ns.errors[error_no];

        if args.len() != error.fields.len() {
            ns.diagnostics.push(Diagnostic::error_with_note(
                path.loc,
                format!(
                    "error '{}' has {} fields, {} provided",
                    error.name,
                    error.fields.len(),
                    args.len()
                ),
                error.loc,
                format!("definition of '{}'", error.name),
            ));
        }

        if ns.target == Target::Solana {
            ns.diagnostics.push(Diagnostic::error(
                *loc,
                format!("revert with custom errors not supported on {}", ns.target),
            ));
        }

        ns.errors[error_no].used = true;

        Ok(Statement::Revert {
            loc: *loc,
            error_no: Some(error_no),
            args: arguments,
        })
    } else {
        let mut arguments = Vec::new();

        match args.len() {
            0 => (),
            1 => {
                if let Ok(arg) = expression(
                    &args[0],
                    context,
                    ns,
                    symtable,
                    diagnostics,
                    ResolveTo::Type(&Type::String),
                ) {
                    if let Ok(expr) = arg.cast(&arg.loc(), &Type::String, true, ns, diagnostics) {
                        arguments.push(expr);
                    }
                }
            }
            count => {
                let loc = pt::Loc::File(loc.file_no(), loc.start(), loc.start() + 6);

                ns.diagnostics.push(Diagnostic::error(
                    loc,
                    format!(
                        "revert takes either no argument or a single reason string argument, {count} provided"
                    ),
                ));

                for arg in args {
                    let _ = expression(arg, context, ns, symtable, diagnostics, ResolveTo::Unknown);
                }
            }
        }

        Ok(Statement::Revert {
            loc: *loc,
            error_no: None,
            args: arguments,
        })
    }
}

/// Resolve a revert statement with named arguments, and optional error, e.g.
/// ```ignore
/// revert({foo: 1}); // not allowed
/// revert({reason:"the reason why"}); // not allowed
/// revert NotEnoughBalance({user: address});
/// ```
fn revert_named_arg(
    loc: &pt::Loc,
    path: &Option<pt::IdentifierPath>,
    args: &[pt::NamedArgument],
    context: &mut ExprContext,
    symtable: &mut Symtable,
    diagnostics: &mut Diagnostics,
    ns: &mut Namespace,
) -> Result<Statement, ()> {
    if let Some(path) = path {
        let error_no = ns.resolve_error(context.file_no, context.contract_no, path, diagnostics)?;

        let error = &ns.errors[error_no];

        let unnamed_fields = error
            .fields
            .iter()
            .filter(|param| param.id.is_none())
            .count();

        if unnamed_fields > 0 {
            ns.diagnostics.push(Diagnostic::error_with_note(
                path.loc,
                format!(
                    "error '{}' has {} unnamed fields",
                    error.name, unnamed_fields
                ),
                error.loc,
                format!("definition of '{}'", error.name),
            ));
        }

        let fields: HashMap<String, (usize, Type)> = error
            .fields
            .iter()
            .enumerate()
            .filter_map(|(i, p)| {
                p.id.as_ref()
                    .map(|id| (id.name.to_owned(), (i, p.ty.clone())))
            })
            .collect();

        let mut arguments = HashMap::new();

        for arg in args {
            let error = &ns.errors[error_no];

            let name = arg.name.name.as_str();

            if let Some((pos, ty)) = fields.get(name) {
                if arguments.contains_key(pos) {
                    diagnostics.push(Diagnostic::error(
                        arg.name.loc,
                        format!("duplicate argument with name '{}'", arg.name.name),
                    ));

                    let _ = expression(
                        &arg.expr,
                        context,
                        ns,
                        symtable,
                        diagnostics,
                        ResolveTo::Unknown,
                    );

                    continue;
                }

                let arg = match expression(
                    &arg.expr,
                    context,
                    ns,
                    symtable,
                    diagnostics,
                    ResolveTo::Type(ty),
                ) {
                    Ok(e) => e,
                    Err(()) => {
                        continue;
                    }
                };

                match arg.cast(&arg.loc(), ty, true, ns, diagnostics) {
                    Ok(expr) => {
                        arguments.insert(*pos, expr);
                    }
                    Err(()) => {
                        continue;
                    }
                }
            } else {
                ns.diagnostics.push(Diagnostic::error_with_note(
                    arg.name.loc,
                    format!("error '{}' has no field called '{}'", error.name, name),
                    error.loc,
                    format!("definition of '{}'", error.name),
                ));

                let _ = expression(
                    &arg.expr,
                    context,
                    ns,
                    symtable,
                    diagnostics,
                    ResolveTo::Unknown,
                );
            }
        }

        if ns.target == Target::Solana {
            ns.diagnostics.push(Diagnostic::error(
                *loc,
                format!("revert with custom errors not supported on {}", ns.target),
            ));
        }

        let mut args = Vec::new();
        let error = &ns.errors[error_no];

        for pos in 0..error.fields.len() {
            if let Some(arg) = arguments.remove(&pos) {
                args.push(arg);
            } else if let Some(id) = &error.fields[pos].id {
                ns.diagnostics.push(Diagnostic::error_with_note(
                    path.loc,
                    format!("missing field '{}'", id.name),
                    error.loc,
                    format!("definition of '{}'", error.name),
                ));
            }
        }

        ns.errors[error_no].used = true;

        Ok(Statement::Revert {
            loc: *loc,
            error_no: Some(error_no),
            args,
        })
    } else {
        ns.diagnostics.push(Diagnostic::error(
            *loc,
            "revert with named arguments requires error type".to_string(),
        ));

        Err(())
    }
}

/// Resolve emit event
fn emit_event(
    loc: &pt::Loc,
    ty: &pt::Expression,
    context: &mut ExprContext,
    symtable: &mut Symtable,
    ns: &mut Namespace,
    diagnostics: &mut Diagnostics,
) -> Result<Statement, ()> {
    let function_no = context.function_no.unwrap();
    let to_stmt =
        |ns: &mut Namespace, event_no: usize, event_loc: pt::Loc, args: Vec<Expression>| {
            if !ns.functions[function_no].emits_events.contains(&event_no) {
                ns.functions[function_no].emits_events.push(event_no);
            }
            Statement::Emit {
                loc: *loc,
                event_no,
                event_loc,
                args,
            }
        };

    match ty {
        pt::Expression::FunctionCall(_, ty, args) => {
            let event_loc = ty.loc();
            let mut emit_diagnostics = Diagnostics::default();
            let mut valid_args = Vec::new();

            for arg in args {
                if let Ok(exp) = expression(
                    arg,
                    context,
                    ns,
                    symtable,
                    &mut emit_diagnostics,
                    ResolveTo::Unknown,
                ) {
                    used_variable(ns, &exp, symtable);
                    valid_args.push(exp);
                };
            }

            let Ok(event_nos) = ns.resolve_event(
                context.file_no,
                context.contract_no,
                ty,
                &mut emit_diagnostics,
            ) else {
                diagnostics.extend(emit_diagnostics);
                return Err(());
            };

            if emit_diagnostics.any_errors() {
                diagnostics.extend(emit_diagnostics);
                return Ok(to_stmt(ns, event_nos[0], event_loc, valid_args));
            }

            let mut resolved_events = Vec::new();

            for event_no in &event_nos {
                let mut candidate_diagnostics = Diagnostics::default();

                let event = &ns.events[*event_no];
                let mut cast_args = Vec::new();

                if args.len() != event.fields.len() {
                    candidate_diagnostics.push(Diagnostic::error_with_note(
                        *loc,
                        format!(
                            "event type '{}' has {} fields, {} provided",
                            event.id,
                            event.fields.len(),
                            args.len()
                        ),
                        event.id.loc,
                        format!("definition of {}", event.id),
                    ));
                }

                // check if arguments can be implicitly casted
                for (i, arg) in args.iter().enumerate() {
                    if let Some(ty) = ns.events[*event_no]
                        .fields
                        .get(i)
                        .map(|field| field.ty.clone())
                    {
                        if let Ok(expr) = expression(
                            arg,
                            context,
                            ns,
                            symtable,
                            &mut candidate_diagnostics,
                            ResolveTo::Type(&ty),
                        )
                        .and_then(|arg| {
                            arg.cast(&arg.loc(), &ty, true, ns, &mut candidate_diagnostics)
                        }) {
                            used_variable(ns, &expr, symtable);
                            cast_args.push(expr);
                        }
                    }
                }

                if candidate_diagnostics.any_errors() {
                    if event_nos.len() != 1 {
                        let event = &ns.events[*event_no];

                        candidate_diagnostics.iter_mut().for_each(|diagnostic| {
                            diagnostic.notes.push(Note {
                                loc: event.loc,
                                message: "candidate event".into(),
                            })
                        });

                        // will be de-duped
                        candidate_diagnostics.push(Diagnostic::error(
                            *loc,
                            "cannot find event with matching signature".into(),
                        ));
                    }

                    emit_diagnostics.extend(candidate_diagnostics);
                } else {
                    resolved_events.push((
                        *event_no,
                        candidate_diagnostics,
                        to_stmt(ns, *event_no, event_loc, cast_args),
                    ));
                }
            }

            remove_duplicate_events(loc, &mut resolved_events, context, diagnostics, ns);

            let count = resolved_events.len();

            if count == 0 {
                diagnostics.extend(emit_diagnostics);
            } else if count == 1 {
                let (event_no, candidate_diagnostics, stmt) = resolved_events.remove(0);

                let event = &mut ns.events[event_no];
                event.used = true;

                diagnostics.extend(candidate_diagnostics);

                return Ok(stmt);
            } else {
                diagnostics.push(Diagnostic::error_with_notes(
                    *loc,
                    "emit can be resolved to multiple events".into(),
                    resolved_events
                        .into_iter()
                        .map(|(event_no, _, _)| {
                            let event = &ns.events[event_no];

                            Note {
                                loc: event.id.loc,
                                message: "candidate event".into(),
                            }
                        })
                        .collect(),
                ));
            };
        }
        pt::Expression::NamedFunctionCall(_, ty, args) => {
            let event_loc = ty.loc();

            let mut emit_diagnostics = Diagnostics::default();
            let mut arguments = HashMap::new();
            let mut resolved_events = Vec::new();
            let mut valid_args = Vec::new();

            for arg in args {
                if let Ok(expr) = expression(
                    &arg.expr,
                    context,
                    ns,
                    symtable,
                    &mut emit_diagnostics,
                    ResolveTo::Unknown,
                ) {
                    used_variable(ns, &expr, symtable);
                    valid_args.push(expr);
                };

                if arguments.contains_key(arg.name.name.as_str()) {
                    emit_diagnostics.push(Diagnostic::error(
                        arg.name.loc,
                        format!("duplicate argument with name '{}'", arg.name.name),
                    ));
                    continue;
                }

                arguments.insert(arg.name.name.as_str(), &arg.expr);
            }

            let Ok(event_nos) = ns.resolve_event(
                context.file_no,
                context.contract_no,
                ty,
                &mut emit_diagnostics,
            ) else {
                diagnostics.extend(emit_diagnostics);
                return Err(());
            };

            if emit_diagnostics.any_errors() {
                diagnostics.extend(emit_diagnostics);
                return Ok(to_stmt(ns, event_nos[0], event_loc, valid_args));
            }

            for event_no in &event_nos {
                let mut candidate_diagnostics = Diagnostics::default();
                let event = &ns.events[*event_no];
                let params_len = event.fields.len();

                let unnamed_fields = event.fields.iter().filter(|p| p.id.is_none()).count();

                let mut cast_args = Vec::new();

                if unnamed_fields > 0 {
                    candidate_diagnostics.push(Diagnostic::error_with_note(
                        *loc,
                        format!(
                            "event cannot be emitted with named fields as {unnamed_fields} of its fields do not have names"
                        ),
                        event.id.loc,
                        format!("definition of {}", event.id),
                    ));
                } else if params_len != arguments.len() {
                    candidate_diagnostics.push(Diagnostic::error_with_note(
                        *loc,
                        format!(
                            "event expects {} arguments, {} provided",
                            params_len,
                            arguments.len()
                        ),
                        event.id.loc,
                        format!("definition of {}", event.id),
                    ));
                }

                // check if arguments can be implicitly casted
                for i in 0..params_len {
                    let param = ns.events[*event_no].fields[i].clone();

                    if param.id.is_none() {
                        continue;
                    }

                    let arg = match arguments.get(param.name_as_str()) {
                        Some(a) => a,
                        None => {
                            candidate_diagnostics.push(Diagnostic::error(
                                *loc,
                                format!(
                                    "missing argument '{}' to event '{}'",
                                    param.name_as_str(),
                                    ns.events[*event_no].id,
                                ),
                            ));
                            continue;
                        }
                    };

                    if let Ok(expr) = expression(
                        arg,
                        context,
                        ns,
                        symtable,
                        &mut candidate_diagnostics,
                        ResolveTo::Type(&param.ty),
                    )
                    .and_then(|arg| {
                        arg.cast(&arg.loc(), &param.ty, true, ns, &mut candidate_diagnostics)
                    }) {
                        used_variable(ns, &expr, symtable);
                        cast_args.push(expr);
                    }
                }

                if candidate_diagnostics.any_errors() {
                    if event_nos.len() != 1 {
                        let event = &ns.events[*event_no];

                        candidate_diagnostics.iter_mut().for_each(|diagnostic| {
                            diagnostic.notes.push(Note {
                                loc: event.loc,
                                message: "candidate event".into(),
                            })
                        });

                        // will be de-duped
                        candidate_diagnostics.push(Diagnostic::error(
                            *loc,
                            "cannot find event with matching signature".into(),
                        ));
                    }

                    emit_diagnostics.extend(candidate_diagnostics);
                } else {
                    resolved_events.push((
                        *event_no,
                        candidate_diagnostics,
                        to_stmt(ns, *event_no, event_loc, cast_args),
                    ));
                }
            }

            remove_duplicate_events(loc, &mut resolved_events, context, diagnostics, ns);

            let count = resolved_events.len();

            if count == 0 {
                diagnostics.extend(emit_diagnostics);
            } else if count == 1 {
                let (event_no, candidate_diagnostics, stmt) = resolved_events.remove(0);

                diagnostics.extend(candidate_diagnostics);

                let event = &mut ns.events[event_no];
                event.used = true;

                return Ok(stmt);
            } else {
                diagnostics.push(Diagnostic::error_with_notes(
                    *loc,
                    "emit can be resolved to multiple events".into(),
                    resolved_events
                        .into_iter()
                        .map(|(event_no, _, _)| {
                            let event = &ns.events[event_no];

                            Note {
                                loc: event.id.loc,
                                message: "candidate event".into(),
                            }
                        })
                        .collect(),
                ));
            }
        }
        pt::Expression::FunctionCallBlock(_, ty, block) => {
            let _ = ns.resolve_event(context.file_no, context.contract_no, ty, diagnostics);

            diagnostics.push(Diagnostic::error(
                block.loc(),
                "expected event arguments, found code block".to_string(),
            ));
        }
        _ => unreachable!(),
    }

    Err(())
}

fn remove_duplicate_events(
    loc: &pt::Loc,
    resolved_events: &mut Vec<(usize, Diagnostics, Statement)>,
    context: &ExprContext,
    diagnostics: &mut Diagnostics,
    ns: &Namespace,
) {
    resolved_events.dedup_by(|a, b| ns.events[a.0].identical(&ns.events[b.0]));

    if context.ambiguous_emit
        && resolved_events.len() > 1
        && resolved_events
            .windows(2)
            .all(|slice| ns.events[slice[0].0].identical_v0_5(&ns.events[slice[1].0]))
    {
        diagnostics.push(Diagnostic::warning_with_notes(
            *loc,
            "emit can be resolved to multiple incompatible events. This is permitted in Solidity v0.5 and earlier, however it could indicate a bug.".into(),
            resolved_events
                .iter()
                .map(|(event_no, _, _)| {
                    let event = &ns.events[*event_no];

                    Note {
                        loc: event.id.loc,
                        message: "candidate event".into(),
                    }
                })
                .collect(),
        ));

        resolved_events.truncate(1);
    }
}

impl EventDecl {
    // Are two events Identical?
    pub fn identical(&self, other: &Self) -> bool {
        self.id.name == other.id.name
            && self.anonymous == other.anonymous
            && self
                .fields
                .iter()
                .zip(other.fields.iter())
                .all(|(l, r)| l.indexed == r.indexed && l.ty == r.ty)
    }

    // Are two events Identical in Solidity v0.5?
    // Solidity v0.5 erronously does not compare indexed-ness and anonymous-ness
    pub fn identical_v0_5(&self, other: &Self) -> bool {
        self.id.name == other.id.name
            && self
                .fields
                .iter()
                .zip(other.fields.iter())
                .all(|(l, r)| l.ty == r.ty)
    }
}

/// Resolve destructuring assignment
fn destructure(
    loc: &pt::Loc,
    vars: &[(pt::Loc, Option<pt::Parameter>)],
    expr: &pt::Expression,
    context: &mut ExprContext,
    symtable: &mut Symtable,
    ns: &mut Namespace,
    diagnostics: &mut Diagnostics,
) -> Result<Statement, ()> {
    // first resolve the fields so we know the types
    let mut fields = Vec::new();
    let mut left_tys = Vec::new();

    let prev_lvalue = context.lvalue;
    context.lvalue = true;

    let mut context = scopeguard::guard(context, |context| {
        context.lvalue = prev_lvalue;
    });

    for (_, param) in vars {
        match param {
            None => {
                left_tys.push(None);
                fields.push(DestructureField::None);
            }

            Some(pt::Parameter {
                loc,
                ty,
                storage,
                name: None,
                annotation,
            }) => {
                // The grammar does not allow annotation in destructures, so this assertion shall
                // always be true.
                assert!(annotation.is_none());
                if let Some(storage) = storage {
                    diagnostics.push(Diagnostic::error(
                        storage.loc(),
                        format!("storage modifier '{storage}' not permitted on assignment"),
                    ));
                    return Err(());
                }

                // ty will just be a normal expression, not a type
                let e = expression(
                    ty,
                    &mut context,
                    ns,
                    symtable,
                    diagnostics,
                    ResolveTo::Unknown,
                )?;

                match &e {
                    Expression::ConstantVariable {
                        contract_no: Some(contract_no),
                        var_no,
                        ..
                    } => {
                        diagnostics.push(Diagnostic::error(
                            *loc,
                            format!(
                                "cannot assign to constant '{}'",
                                ns.contracts[*contract_no].variables[*var_no].name
                            ),
                        ));
                        return Err(());
                    }
                    Expression::ConstantVariable {
                        contract_no: None,
                        var_no,
                        ..
                    } => {
                        diagnostics.push(Diagnostic::error(
                            *loc,
                            format!("cannot assign to constant '{}'", ns.constants[*var_no].name),
                        ));
                        return Err(());
                    }
                    Expression::StorageVariable {
                        contract_no,
                        var_no,
                        ..
                    } => {
                        let store_var = &ns.contracts[*contract_no].variables[*var_no];

                        if store_var.immutable
                            && !ns.functions[context.function_no.unwrap()].is_constructor()
                        {
                            diagnostics.push(Diagnostic::error(
                                *loc,
                                format!(
                                    "cannot assign to immutable '{}' outside of constructor",
                                    store_var.name
                                ),
                            ));
                            return Err(());
                        }
                    }
                    Expression::Variable { .. } => (),
                    _ => match e.ty() {
                        Type::Ref(_) | Type::StorageRef(false, _) => (),
                        _ => {
                            diagnostics.push(Diagnostic::error(
                                *loc,
                                "expression is not assignable".to_string(),
                            ));
                            return Err(());
                        }
                    },
                }

                assigned_variable(ns, &e, symtable);
                left_tys.push(Some(e.ty()));
                fields.push(DestructureField::Expression(e));
            }
            Some(pt::Parameter {
                loc,
                ty,
                storage,
                name: Some(name),
                annotation,
            }) => {
                // The grammar does not allow annotation in destructures, so this assertion shall
                // always be true.
                assert!(annotation.is_none());
                let (ty, ty_loc) = resolve_var_decl_ty(ty, storage, &mut context, ns, diagnostics)?;

                if let Some(pos) = symtable.add(
                    name,
                    ty.clone(),
                    ns,
                    VariableInitializer::Solidity(None),
                    VariableUsage::DestructureVariable,
                    storage.clone(),
                    &mut context,
                ) {
                    ns.check_shadowing(context.file_no, context.contract_no, name);

                    left_tys.push(Some(ty.clone()));

                    fields.push(DestructureField::VariableDecl(
                        pos,
                        Parameter {
                            loc: *loc,
                            id: Some(name.clone()),
                            ty,
                            ty_loc: Some(ty_loc),
                            indexed: false,
                            readonly: false,
                            infinite_size: false,
                            recursive: false,
                            annotation: None,
                        },
                    ));
                }
            }
        }
    }

    context.lvalue = false;

    let expr = destructure_values(
        loc,
        expr,
        &left_tys,
        &fields,
        &mut context,
        symtable,
        ns,
        diagnostics,
    )?;

    Ok(Statement::Destructure(*loc, fields, expr))
}

fn destructure_values(
    loc: &pt::Loc,
    expr: &pt::Expression,
    left_tys: &[Option<Type>],
    fields: &[DestructureField],
    context: &mut ExprContext,
    symtable: &mut Symtable,
    ns: &mut Namespace,
    diagnostics: &mut Diagnostics,
) -> Result<Expression, ()> {
    let expr = match expr.remove_parenthesis() {
        pt::Expression::FunctionCall(loc, ty, args) => {
            let res = function_call_expr(
                loc,
                ty,
                args,
                context,
                ns,
                symtable,
                diagnostics,
                ResolveTo::Unknown,
            )?;
            check_function_call(ns, &res, symtable);
            res
        }
        pt::Expression::NamedFunctionCall(loc, ty, args) => {
            let res = named_function_call_expr(
                loc,
                ty,
                args,
                context,
                ns,
                symtable,
                diagnostics,
                ResolveTo::Unknown,
            )?;
            check_function_call(ns, &res, symtable);
            res
        }
        pt::Expression::ConditionalOperator(loc, cond, left, right) => {
            let cond = expression(
                cond,
                context,
                ns,
                symtable,
                diagnostics,
                ResolveTo::Type(&Type::Bool),
            )?;

            used_variable(ns, &cond, symtable);
            let left = destructure_values(
                &left.loc(),
                left,
                left_tys,
                fields,
                context,
                symtable,
                ns,
                diagnostics,
            )?;
            used_variable(ns, &left, symtable);
            let right = destructure_values(
                &right.loc(),
                right,
                left_tys,
                fields,
                context,
                symtable,
                ns,
                diagnostics,
            )?;
            used_variable(ns, &right, symtable);

            return Ok(Expression::ConditionalOperator {
                loc: *loc,
                ty: Type::Unreachable,
                cond: Box::new(cond),
                true_option: Box::new(left),
                false_option: Box::new(right),
            });
        }
        _ => {
            let mut list = Vec::new();

            let exprs = parameter_list_to_expr_list(expr, diagnostics)?;

            if exprs.len() != left_tys.len() {
                diagnostics.push(Diagnostic::error(
                    *loc,
                    format!(
                        "destructuring assignment has {} elements on the left and {} on the right",
                        left_tys.len(),
                        exprs.len(),
                    ),
                ));
                return Err(());
            }

            for (i, e) in exprs.iter().enumerate() {
                let e = expression(
                    e,
                    context,
                    ns,
                    symtable,
                    diagnostics,
                    if let Some(ty) = left_tys[i].as_ref() {
                        ResolveTo::Type(ty)
                    } else {
                        ResolveTo::Unknown
                    },
                )?;
                match e.ty() {
                    Type::Void | Type::Unreachable => {
                        diagnostics.push(Diagnostic::error(
                            e.loc(),
                            "function does not return a value".to_string(),
                        ));
                        return Err(());
                    }
                    _ => {
                        used_variable(ns, &e, symtable);
                    }
                }

                list.push(e);
            }

            Expression::List { loc: *loc, list }
        }
    };

    let mut right_tys = expr.tys();

    // Return type void or unreachable are synthetic
    if right_tys.len() == 1 && (right_tys[0] == Type::Unreachable || right_tys[0] == Type::Void) {
        right_tys.truncate(0);
    }

    if left_tys.len() != right_tys.len() {
        diagnostics.push(Diagnostic::error(
            *loc,
            format!(
                "destructuring assignment has {} elements on the left and {} on the right",
                left_tys.len(),
                right_tys.len()
            ),
        ));
        return Err(());
    }

    // Check that the values can be cast
    for (i, field) in fields.iter().enumerate() {
        if let Some(left_ty) = &left_tys[i] {
            let loc = field.loc_opt().unwrap();
            let _ = Expression::Variable {
                loc,
                ty: right_tys[i].clone(),
                var_no: i,
            }
            .cast(&loc, left_ty.deref_any(), true, ns, diagnostics)?;
        }
    }
    Ok(expr)
}

/// Resolve the type of a variable declaration
fn resolve_var_decl_ty(
    ty: &pt::Expression,
    storage: &Option<pt::StorageLocation>,
    context: &mut ExprContext,
    ns: &mut Namespace,
    diagnostics: &mut Diagnostics,
) -> Result<(Type, pt::Loc), ()> {
    let mut loc_ty = ty.loc();
    let mut var_ty = ns.resolve_type(
        context.file_no,
        context.contract_no,
        ResolveTypeContext::None,
        ty,
        diagnostics,
    )?;

    if let Some(storage) = storage {
        if !var_ty.can_have_data_location() {
            diagnostics.push(Diagnostic::error(
                storage.loc(),
                format!("data location '{storage}' only allowed for array, struct or mapping type"),
            ));
            return Err(());
        }

        if let pt::StorageLocation::Storage(loc) = storage {
            loc_ty.use_end_from(loc);
            var_ty = Type::StorageRef(false, Box::new(var_ty));
        }

        // Note we are completely ignoring memory or calldata data locations. Everything
        // will be stored in memory.
    }

    if var_ty.contains_mapping(ns) && !var_ty.is_contract_storage() {
        diagnostics.push(Diagnostic::error(
            ty.loc(),
            "mapping only allowed in storage".to_string(),
        ));
        return Err(());
    }

    if !var_ty.is_contract_storage() && !var_ty.fits_in_memory(ns) {
        diagnostics.push(Diagnostic::error(
            ty.loc(),
            "type is too large to fit into memory".to_string(),
        ));
        return Err(());
    }

    Ok((var_ty, loc_ty))
}

/// Resolve return statement
fn return_with_values(
    returns: &pt::Expression,
    loc: &pt::Loc,
    context: &mut ExprContext,
    symtable: &mut Symtable,
    ns: &mut Namespace,
    diagnostics: &mut Diagnostics,
) -> Result<Expression, ()> {
    let function_no = context.function_no.unwrap();

    let no_returns = ns.functions[function_no].returns.len();
    let expr_returns = match returns.remove_parenthesis() {
        pt::Expression::FunctionCall(loc, ty, args) => {
            let expr = call_expr(
                loc,
                ty,
                args,
                true,
                context,
                ns,
                symtable,
                diagnostics,
                ResolveTo::Unknown,
            )?;
            used_variable(ns, &expr, symtable);
            expr
        }
        pt::Expression::NamedFunctionCall(loc, ty, args) => {
            let expr = named_call_expr(
                loc,
                ty,
                args,
                true,
                context,
                ns,
                symtable,
                diagnostics,
                ResolveTo::Unknown,
            )?;
            used_variable(ns, &expr, symtable);
            expr
        }
        pt::Expression::ConditionalOperator(loc, cond, left, right) => {
            let cond = expression(
                cond,
                context,
                ns,
                symtable,
                diagnostics,
                ResolveTo::Type(&Type::Bool),
            )?;
            used_variable(ns, &cond, symtable);

            let cond = cond.cast(&cond.loc(), &Type::Bool, true, ns, diagnostics)?;

            let left = return_with_values(left, &left.loc(), context, symtable, ns, diagnostics)?;
            used_variable(ns, &left, symtable);

            let right =
                return_with_values(right, &right.loc(), context, symtable, ns, diagnostics)?;
            used_variable(ns, &right, symtable);

            return Ok(Expression::ConditionalOperator {
                loc: *loc,
                ty: Type::Unreachable,
                cond: Box::new(cond),
                true_option: Box::new(left),
                false_option: Box::new(right),
            });
        }
        _ => {
            let returns = parameter_list_to_expr_list(returns, diagnostics)?;

            if no_returns > 0 && returns.is_empty() {
                diagnostics.push(Diagnostic::error(
                    *loc,
                    format!("missing return value, {no_returns} return values expected"),
                ));
                return Err(());
            }

            if no_returns == 0 && !returns.is_empty() {
                diagnostics.push(Diagnostic::error(
                    *loc,
                    "function has no return values".to_string(),
                ));
                return Err(());
            }

            if no_returns != returns.len() {
                diagnostics.push(Diagnostic::error(
                    *loc,
                    format!(
                        "incorrect number of return values, expected {} but got {}",
                        no_returns,
                        returns.len(),
                    ),
                ));
                return Err(());
            }

            let mut exprs = Vec::new();

            let return_tys = ns.functions[function_no]
                .returns
                .iter()
                .map(|r| r.ty.clone())
                .collect::<Vec<_>>();

            for (expr_return, return_ty) in returns.iter().zip(return_tys) {
                let expr = expression(
                    expr_return,
                    context,
                    ns,
                    symtable,
                    diagnostics,
                    ResolveTo::Type(&return_ty),
                )?;
                let expr = expr.cast(&expr_return.loc(), &return_ty, true, ns, diagnostics)?;
                used_variable(ns, &expr, symtable);
                exprs.push(expr);
            }

            return Ok(if exprs.len() == 1 {
                exprs[0].clone()
            } else {
                Expression::List {
                    loc: *loc,
                    list: exprs,
                }
            });
        }
    };

    let mut expr_return_tys = expr_returns.tys();
    // Return type void or unreachable are synthetic
    if expr_return_tys.len() == 1
        && (expr_return_tys[0] == Type::Unreachable || expr_return_tys[0] == Type::Void)
    {
        expr_return_tys.truncate(0);
    }

    if no_returns > 0 && expr_return_tys.is_empty() {
        diagnostics.push(Diagnostic::error(
            *loc,
            format!("missing return value, {no_returns} return values expected"),
        ));
        return Err(());
    }

    if no_returns == 0 && !expr_return_tys.is_empty() {
        diagnostics.push(Diagnostic::error(
            *loc,
            "function has no return values".to_string(),
        ));
        return Err(());
    }

    if no_returns != expr_return_tys.len() {
        diagnostics.push(Diagnostic::error(
            *loc,
            format!(
                "incorrect number of return values, expected {} but got {}",
                no_returns,
                expr_return_tys.len(),
            ),
        ));
        return Err(());
    }

    let func_returns_tys = ns.functions[function_no]
        .returns
        .iter()
        .map(|r| r.ty.clone())
        .collect::<Vec<_>>();

    // Check that the values can be cast
    let _ = expr_return_tys
        .into_iter()
        .zip(func_returns_tys)
        .enumerate()
        .map(|(i, (expr_return_ty, func_return_ty))| {
            Expression::Variable {
                loc: expr_returns.loc(),
                ty: expr_return_ty,
                var_no: i,
            }
            .cast(&expr_returns.loc(), &func_return_ty, true, ns, diagnostics)
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(expr_returns)
}

/// The parser generates parameter lists for lists. Sometimes this needs to be a
/// simple expression list.
pub fn parameter_list_to_expr_list<'a>(
    e: &'a pt::Expression,
    diagnostics: &mut Diagnostics,
) -> Result<Vec<&'a pt::Expression>, ()> {
    match e {
        pt::Expression::List(_, v) => {
            let mut list = Vec::new();
            let mut broken = false;

            for e in v {
                match &e.1 {
                    None => {
                        diagnostics.push(Diagnostic::error(e.0, "stray comma".to_string()));
                        broken = true;
                    }
                    Some(pt::Parameter {
                        name: Some(name),
                        annotation,
                        ..
                    }) => {
                        // The grammar does not allow an annotation inside a list
                        assert!(annotation.is_none());
                        diagnostics.push(Diagnostic::error(
                            name.loc,
                            format!("unexpected identifier '{}'", name.name),
                        ));
                        broken = true;
                    }
                    Some(pt::Parameter {
                        storage: Some(storage),
                        annotation,
                        ..
                    }) => {
                        // The grammar does not allow an annotation inside a list
                        assert!(annotation.is_none());
                        diagnostics.push(Diagnostic::error(
                            storage.loc(),
                            "storage specified not permitted here".to_string(),
                        ));
                        broken = true;
                    }
                    Some(pt::Parameter { annotation, ty, .. }) => {
                        // The grammar does not allow an annotation inside a list
                        assert!(annotation.is_none());
                        list.push(ty);
                    }
                }
            }

            if !broken {
                Ok(list)
            } else {
                Err(())
            }
        }
        pt::Expression::Parenthesis(_, e) => Ok(vec![e]),
        e => Ok(vec![e]),
    }
}

/// Parse try catch
#[allow(clippy::type_complexity)]
fn try_catch(
    loc: &pt::Loc,
    expr: &pt::Expression,
    returns_and_ok: &Option<(Vec<(pt::Loc, Option<pt::Parameter>)>, Box<pt::Statement>)>,
    clause_stmts: &[pt::CatchClause],
    context: &mut ExprContext,
    symtable: &mut Symtable,
    ns: &mut Namespace,
    diagnostics: &mut Diagnostics,
) -> Result<(Statement, bool), ()> {
    if ns.target == Target::Solana {
        diagnostics.push(Diagnostic::error(
            *loc,
            "The try-catch statement is not supported on Solana. Please, go to \
             https://solang.readthedocs.io/en/latest/language/statements.html#try-catch-statement \
             for more information"
                .to_string(),
        ));
        return Err(());
    }

    let mut expr = expr.remove_parenthesis();
    let mut ok = None;

    while let pt::Expression::FunctionCallBlock(_, e, block) = expr {
        if ok.is_some() {
            diagnostics.push(Diagnostic::error(
                block.loc(),
                "unexpected code block".to_string(),
            ));
            return Err(());
        }

        ok = Some(block.as_ref());

        expr = e.as_ref();
    }

    let fcall = match expr.remove_parenthesis() {
        pt::Expression::FunctionCall(loc, ty, args) => {
            let res = match ty.remove_parenthesis() {
                pt::Expression::New(_, ty) => {
                    new(loc, ty, args, context, ns, symtable, diagnostics)?
                }
                pt::Expression::FunctionCallBlock(loc, expr, _)
                    if matches!(expr.remove_parenthesis(), pt::Expression::New(..)) =>
                {
                    new(loc, ty, args, context, ns, symtable, diagnostics)?
                }
                _ => function_call_expr(
                    loc,
                    ty,
                    args,
                    context,
                    ns,
                    symtable,
                    diagnostics,
                    ResolveTo::Unknown,
                )?,
            };
            check_function_call(ns, &res, symtable);
            res
        }
        pt::Expression::NamedFunctionCall(loc, ty, args) => {
            let res = named_function_call_expr(
                loc,
                ty,
                args,
                context,
                ns,
                symtable,
                diagnostics,
                ResolveTo::Unknown,
            )?;

            check_function_call(ns, &res, symtable);

            res
        }
        pt::Expression::New(loc, call) => {
            let mut call = call.remove_parenthesis();

            while let pt::Expression::FunctionCallBlock(_, expr, block) = call {
                if ok.is_some() {
                    ns.diagnostics.push(Diagnostic::error(
                        block.loc(),
                        "unexpected code block".to_string(),
                    ));
                    return Err(());
                }

                ok = Some(block.as_ref());

                call = expr.remove_parenthesis();
            }

            match call {
                pt::Expression::FunctionCall(_, ty, args) => {
                    let res = new(loc, ty, args, context, ns, symtable, diagnostics)?;
                    check_function_call(ns, &res, symtable);

                    res
                }
                pt::Expression::NamedFunctionCall(_, ty, args) => {
                    let res =
                        constructor_named_args(loc, ty, args, context, ns, symtable, diagnostics)?;
                    check_function_call(ns, &res, symtable);

                    res
                }
                _ => unreachable!(),
            }
        }
        _ => {
            diagnostics.push(Diagnostic::error(
                expr.loc(),
                "try only supports external calls or constructor calls".to_string(),
            ));
            return Err(());
        }
    };

    let mut returns = &Vec::new();

    if let Some((rets, block)) = returns_and_ok {
        if ok.is_some() {
            diagnostics.push(Diagnostic::error(
                block.loc(),
                "unexpected code block".to_string(),
            ));
            return Err(());
        }

        ok = Some(block);

        returns = rets;
    }

    let ok = match ok {
        Some(ok) => ok,
        None => {
            // position after the expression
            let pos = expr.loc().begin_range();

            diagnostics.push(Diagnostic::error(
                pos,
                "code block missing for no catch".to_string(),
            ));
            return Err(());
        }
    };

    let mut args = match &fcall {
        Expression::ExternalFunctionCall {
            returns: func_returns,
            ..
        } => {
            let mut func_returns = func_returns.clone();

            if func_returns == vec![Type::Void] {
                func_returns = vec![];
            }

            if !returns.is_empty() && returns.len() != func_returns.len() {
                diagnostics.push(Diagnostic::error(
                    expr.loc(),
                    format!(
                        "try returns list has {} entries while function returns {} values",
                        returns.len(),
                        func_returns.len()
                    ),
                ));
                return Err(());
            }

            func_returns
        }
        Expression::Constructor { contract_no, .. } => match returns.len() {
            0 => Vec::new(),
            1 => vec![Type::Contract(*contract_no)],
            _ => {
                diagnostics.push(Diagnostic::error(
                    expr.loc(),
                    format!(
                        "constructor returns single contract, not {} values",
                        returns.len()
                    ),
                ));
                return Err(());
            }
        },
        _ => {
            diagnostics.push(Diagnostic::error(
                expr.loc(),
                "try only supports external calls or constructor calls".to_string(),
            ));
            return Err(());
        }
    };

    context.enter_scope();

    let mut params = Vec::new();
    let mut broken = false;
    for param in returns {
        let arg_ty = args.remove(0);

        match &param.1 {
            Some(pt::Parameter {
                annotation: Some(annotation),
                ..
            }) => {
                diagnostics.push(unexpected_parameter_annotation(annotation.loc));
                broken = true;
            }

            Some(pt::Parameter {
                ty, storage, name, ..
            }) => {
                let (ret_ty, ty_loc) = resolve_var_decl_ty(ty, storage, context, ns, diagnostics)?;

                if arg_ty != ret_ty {
                    diagnostics.push(Diagnostic::error(
                        ty.loc(),
                        format!(
                            "type '{}' does not match return value of function '{}'",
                            ret_ty.to_string(ns),
                            arg_ty.to_string(ns)
                        ),
                    ));
                    broken = true;
                }

                if let Some(name) = name {
                    if let Some(pos) = symtable.add(
                        name,
                        ret_ty.clone(),
                        ns,
                        VariableInitializer::Solidity(None),
                        VariableUsage::TryCatchReturns,
                        storage.clone(),
                        context,
                    ) {
                        ns.check_shadowing(context.file_no, context.contract_no, name);
                        params.push((
                            Some(pos),
                            Parameter {
                                loc: param.0,
                                ty: ret_ty,
                                ty_loc: Some(ty_loc),
                                id: Some(name.clone()),
                                indexed: false,
                                readonly: false,
                                infinite_size: false,
                                recursive: false,
                                annotation: None,
                            },
                        ));
                    }
                } else {
                    params.push((
                        None,
                        Parameter {
                            loc: param.0,
                            ty: ret_ty,
                            ty_loc: Some(ty_loc),
                            indexed: false,
                            id: None,
                            readonly: false,
                            infinite_size: false,
                            recursive: false,
                            annotation: None,
                        },
                    ));
                }
            }
            None => {
                diagnostics.push(Diagnostic::error(
                    param.0,
                    "missing return type".to_string(),
                ));
                broken = true;
            }
        }
    }

    if broken {
        return Err(());
    }

    let mut ok_resolved = Vec::new();

    let mut finally_reachable =
        statement(ok, &mut ok_resolved, context, symtable, ns, diagnostics)?;

    context.leave_scope(symtable, *loc);

    let mut clauses_unique = HashSet::new();
    let mut errors_resolved = Vec::new();

    let mut catch_all = None;

    clause_stmts.iter().try_for_each(|clause_stmt| {
        let (loc, name) = match clause_stmt {
            CatchClause::Simple(loc, _, _) => (loc, ""),
            CatchClause::Named(loc, id, _, _) => (loc, id.name.as_str()),
        };
        if !clauses_unique.insert(name) {
            ns.diagnostics.push(Diagnostic::error(
                *loc,
                if name.is_empty() {
                    "duplicate catch clause".to_string()
                } else {
                    format!("duplicate '{name}' catch clause")
                },
            ));
            return Err(());
        }

        match clause_stmt {
            CatchClause::Simple(catch_loc, param, stmt) => {
                context.enter_scope();

                let mut catch_param = None;
                let mut catch_param_pos = None;
                let mut catch_stmt_resolved = vec![];

                if let Some(param) = param {
                    let (catch_ty, ty_loc) =
                        resolve_var_decl_ty(&param.ty, &param.storage, context, ns, diagnostics)?;

                    if catch_ty != Type::DynamicBytes {
                        diagnostics.push(Diagnostic::error(
                            param.ty.loc(),
                            format!(
                                "catch can only take 'bytes memory', not '{}'",
                                catch_ty.to_string(ns)
                            ),
                        ));
                        return Err(());
                    } else if let Some(annotation) = &param.annotation {
                        diagnostics.push(unexpected_parameter_annotation(annotation.loc));
                    }

                    let mut result = Parameter {
                        loc: param.loc,
                        ty: Type::DynamicBytes,
                        ty_loc: Some(ty_loc),
                        id: None,
                        indexed: false,
                        readonly: false,
                        infinite_size: false,
                        recursive: false,
                        annotation: None,
                    };

                    if let Some(name) = &param.name {
                        if let Some(pos) = symtable.add(
                            name,
                            catch_ty,
                            ns,
                            VariableInitializer::Solidity(None),
                            VariableUsage::TryCatchErrorBytes,
                            param.storage.clone(),
                            context,
                        ) {
                            ns.check_shadowing(context.file_no, context.contract_no, name);
                            catch_param_pos = Some(pos);
                            result.id = Some(name.clone());
                        }
                    }

                    catch_param = Some(result);
                }

                let reachable = statement(
                    stmt,
                    &mut catch_stmt_resolved,
                    context,
                    symtable,
                    ns,
                    diagnostics,
                )?;

                finally_reachable |= reachable;

                context.leave_scope(symtable, *catch_loc);

                catch_all = Some(super::ast::CatchClause {
                    param: catch_param,
                    param_pos: catch_param_pos,
                    stmt: catch_stmt_resolved,
                });

                Ok(())
            }
            CatchClause::Named(catch_loc, id, param, stmt) => {
                if id.name != "Error" && id.name != "Panic" {
                    let message = format!(
                        "only catch 'Error' and 'Panic' are supported, not '{}'",
                        id.name
                    );
                    ns.diagnostics.push(Diagnostic::error(id.loc, message));
                    return Err(());
                }
                if let Some(annotation) = &param.annotation {
                    ns.diagnostics
                        .push(unexpected_parameter_annotation(annotation.loc))
                }

                let (error_ty, ty_loc) =
                    resolve_var_decl_ty(&param.ty, &param.storage, context, ns, diagnostics)?;

                if id.name == "Error" && error_ty != Type::String {
                    ns.diagnostics.push(Diagnostic::error(
                        param.ty.loc(),
                        format!(
                            "catch Error(...) can only take 'string memory', not '{}'",
                            error_ty.to_string(ns)
                        ),
                    ));
                }
                if id.name == "Panic" && error_ty != Type::Uint(256) {
                    ns.diagnostics.push(Diagnostic::error(
                        param.ty.loc(),
                        format!(
                            "catch Panic(...) can only take 'uint256', not '{}'",
                            error_ty.to_string(ns)
                        ),
                    ));
                }

                context.enter_scope();

                let mut error_pos = None;
                let mut error_stmt_resolved = Vec::new();
                let mut error_param = Parameter {
                    loc: id.loc,
                    ty: error_ty.clone(),
                    ty_loc: Some(ty_loc),
                    id: None,
                    indexed: false,
                    readonly: false,
                    infinite_size: false,
                    recursive: false,
                    annotation: None,
                };

                if let Some(name) = &param.name {
                    if let Some(pos) = symtable.add(
                        name,
                        error_ty,
                        ns,
                        VariableInitializer::Solidity(None),
                        VariableUsage::TryCatchErrorString,
                        param.storage.clone(),
                        context,
                    ) {
                        ns.check_shadowing(context.file_no, context.contract_no, name);

                        error_pos = Some(pos);
                        error_param.id = Some(name.clone());
                    }
                }

                let reachable = statement(
                    stmt,
                    &mut error_stmt_resolved,
                    context,
                    symtable,
                    ns,
                    diagnostics,
                )?;

                finally_reachable |= reachable;

                context.leave_scope(symtable, *catch_loc);

                errors_resolved.push((error_pos, error_param, error_stmt_resolved));

                Ok(())
            }
        }
    })?;

    let stmt = Statement::TryCatch(
        *loc,
        finally_reachable,
        TryCatch {
            expr: fcall,
            returns: params,
            errors: errors_resolved
                .iter()
                .map(|(pos, param, stmt)| super::ast::CatchClause {
                    param: param.clone().into(),
                    param_pos: *pos,
                    stmt: stmt.clone(),
                })
                .collect(),
            ok_stmt: ok_resolved,
            catch_all,
        },
    );

    Ok((stmt, finally_reachable))
}
