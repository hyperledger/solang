// SPDX-License-Identifier: Apache-2.0

use super::encoding::{abi_decode, abi_encode};
use super::revert::{
    assert_failure, expr_assert, log_runtime_error, require, PanicCode, SolidityError,
};
use super::storage::{
    array_offset, array_pop, array_push, storage_slots_array_pop, storage_slots_array_push,
};
use super::{
    cfg::{ControlFlowGraph, Instr, InternalCallTy},
    vartable::Vartable,
};
use super::{polkadot, Options};
use crate::codegen::array_boundary::handle_array_assign;
use crate::codegen::constructor::call_constructor;
use crate::codegen::events::new_event_emitter;
use crate::codegen::unused_variable::should_remove_assignment;
use crate::codegen::{Builtin, Expression};
use crate::sema::ast::ExternalCallAccounts;
use crate::sema::{
    ast,
    ast::{
        ArrayLength, CallTy, FormatArg, Function, Namespace, RetrieveType, StringLocation,
        StructType, Type,
    },
    diagnostics::Diagnostics,
    eval::{eval_const_number, eval_const_rational, eval_constants_in_expression},
    expression::integers::bigint_to_expression,
    expression::ResolveTo,
};
use crate::Target;
use num_bigint::{BigInt, Sign};
use num_traits::{FromPrimitive, One, ToPrimitive, Zero};
use solang_parser::pt::{self, CodeLocation, Loc};
use std::{cmp::Ordering, ops::Mul};

pub fn expression(
    expr: &ast::Expression,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
) -> Expression {
    let evaluated = eval_constants_in_expression(expr, &mut Diagnostics::default());
    let expr = evaluated.0.as_ref().unwrap_or(expr);

    match &expr {
        ast::Expression::StorageVariable {
            loc,
            contract_no: var_contract_no,
            var_no,
            ..
        } => {
            // base storage variables should precede contract variables, not overlap
            ns.contracts[contract_no].get_storage_slot(*loc, *var_contract_no, *var_no, ns, None)
        }
        ast::Expression::StorageLoad { loc, ty, expr } => {
            let storage_type = storage_type(expr, ns);
            let storage = expression(expr, cfg, contract_no, func, ns, vartab, opt);

            load_storage(loc, ty, storage, cfg, vartab, storage_type)
        }
        ast::Expression::Add {
            loc,
            ty,
            unchecked,
            left,
            right,
        } => add(
            loc,
            ty,
            *unchecked,
            left,
            cfg,
            contract_no,
            func,
            ns,
            vartab,
            right,
            opt,
        ),
        ast::Expression::Subtract {
            loc,
            ty,
            unchecked,
            left,
            right,
        } => subtract(
            loc,
            ty,
            *unchecked,
            left,
            cfg,
            contract_no,
            func,
            ns,
            vartab,
            right,
            opt,
        ),
        ast::Expression::Multiply {
            loc,
            ty,
            unchecked,
            left,
            right,
        } => {
            if ty.is_rational() {
                let (_, r) = eval_const_rational(expr, ns).unwrap();

                Expression::NumberLiteral {
                    loc: *loc,
                    ty: ty.clone(),
                    value: r.to_integer(),
                }
            } else {
                Expression::Multiply {
                    loc: *loc,
                    ty: ty.clone(),
                    overflowing: *unchecked,
                    left: Box::new(expression(left, cfg, contract_no, func, ns, vartab, opt)),
                    right: Box::new(expression(right, cfg, contract_no, func, ns, vartab, opt)),
                }
            }
        }
        ast::Expression::Divide {
            loc,
            ty,
            left,
            right,
        } => {
            let l = expression(left, cfg, contract_no, func, ns, vartab, opt);
            let r = expression(right, cfg, contract_no, func, ns, vartab, opt);
            if ty.is_signed_int(ns) {
                Expression::SignedDivide {
                    loc: *loc,
                    ty: ty.clone(),
                    left: Box::new(l),
                    right: Box::new(r),
                }
            } else {
                Expression::UnsignedDivide {
                    loc: *loc,
                    ty: ty.clone(),
                    left: Box::new(l),
                    right: Box::new(r),
                }
            }
        }
        ast::Expression::Modulo {
            loc,
            ty,
            left,
            right,
        } => {
            let l = expression(left, cfg, contract_no, func, ns, vartab, opt);
            let r = expression(right, cfg, contract_no, func, ns, vartab, opt);
            if ty.is_signed_int(ns) {
                Expression::SignedModulo {
                    loc: *loc,
                    ty: ty.clone(),
                    left: Box::new(l),
                    right: Box::new(r),
                }
            } else {
                Expression::UnsignedModulo {
                    loc: *loc,
                    ty: ty.clone(),
                    left: Box::new(l),
                    right: Box::new(r),
                }
            }
        }
        ast::Expression::Power {
            loc,
            ty,
            unchecked,
            base,
            exp,
        } => Expression::Power {
            loc: *loc,
            ty: ty.clone(),
            overflowing: *unchecked,
            base: Box::new(expression(base, cfg, contract_no, func, ns, vartab, opt)),
            exp: Box::new(expression(exp, cfg, contract_no, func, ns, vartab, opt)),
        },
        ast::Expression::BitwiseOr {
            loc,
            ty,
            left,
            right,
        } => Expression::BitwiseOr {
            loc: *loc,
            ty: ty.clone(),
            left: Box::new(expression(left, cfg, contract_no, func, ns, vartab, opt)),
            right: Box::new(expression(right, cfg, contract_no, func, ns, vartab, opt)),
        },
        ast::Expression::BitwiseAnd {
            loc,
            ty,
            left,
            right,
        } => Expression::BitwiseAnd {
            loc: *loc,
            ty: ty.clone(),
            left: Box::new(expression(left, cfg, contract_no, func, ns, vartab, opt)),
            right: Box::new(expression(right, cfg, contract_no, func, ns, vartab, opt)),
        },
        ast::Expression::BitwiseXor {
            loc,
            ty,
            left,
            right,
        } => Expression::BitwiseXor {
            loc: *loc,
            ty: ty.clone(),
            left: Box::new(expression(left, cfg, contract_no, func, ns, vartab, opt)),
            right: Box::new(expression(right, cfg, contract_no, func, ns, vartab, opt)),
        },
        ast::Expression::ShiftLeft {
            loc,
            ty,
            left,
            right,
        } => Expression::ShiftLeft {
            loc: *loc,
            ty: ty.clone(),
            left: Box::new(expression(left, cfg, contract_no, func, ns, vartab, opt)),
            right: Box::new(expression(right, cfg, contract_no, func, ns, vartab, opt)),
        },
        ast::Expression::ShiftRight {
            loc,
            ty,
            left,
            right,
            sign,
        } => Expression::ShiftRight {
            loc: *loc,
            ty: ty.clone(),
            left: Box::new(expression(left, cfg, contract_no, func, ns, vartab, opt)),
            right: Box::new(expression(right, cfg, contract_no, func, ns, vartab, opt)),
            signed: *sign,
        },
        ast::Expression::Equal { loc, left, right } => Expression::Equal {
            loc: *loc,
            left: Box::new(expression(left, cfg, contract_no, func, ns, vartab, opt)),
            right: Box::new(expression(right, cfg, contract_no, func, ns, vartab, opt)),
        },
        ast::Expression::NotEqual { loc, left, right } => Expression::NotEqual {
            loc: *loc,
            left: Box::new(expression(left, cfg, contract_no, func, ns, vartab, opt)),
            right: Box::new(expression(right, cfg, contract_no, func, ns, vartab, opt)),
        },
        ast::Expression::More { loc, left, right } => {
            let l = expression(left, cfg, contract_no, func, ns, vartab, opt);
            let r = expression(right, cfg, contract_no, func, ns, vartab, opt);

            Expression::More {
                loc: *loc,
                signed: l.ty().is_signed_int(ns),
                left: Box::new(l),
                right: Box::new(r),
            }
        }
        ast::Expression::MoreEqual { loc, left, right } => Expression::MoreEqual {
            loc: *loc,
            signed: left.ty().is_signed_int(ns),
            left: Box::new(expression(left, cfg, contract_no, func, ns, vartab, opt)),
            right: Box::new(expression(right, cfg, contract_no, func, ns, vartab, opt)),
        },
        ast::Expression::Less { loc, left, right } => {
            let l = expression(left, cfg, contract_no, func, ns, vartab, opt);
            let r = expression(right, cfg, contract_no, func, ns, vartab, opt);
            Expression::Less {
                loc: *loc,
                signed: l.ty().is_signed_int(ns),
                left: Box::new(l),
                right: Box::new(r),
            }
        }
        ast::Expression::LessEqual { loc, left, right } => Expression::LessEqual {
            loc: *loc,
            signed: left.ty().is_signed_int(ns),
            left: Box::new(expression(left, cfg, contract_no, func, ns, vartab, opt)),
            right: Box::new(expression(right, cfg, contract_no, func, ns, vartab, opt)),
        },
        ast::Expression::ConstantVariable {
            contract_no: Some(var_contract_no),
            var_no,
            ..
        } => expression(
            ns.contracts[*var_contract_no].variables[*var_no]
                .initializer
                .as_ref()
                .unwrap(),
            cfg,
            contract_no,
            func,
            ns,
            vartab,
            opt,
        ),
        ast::Expression::ConstantVariable {
            contract_no: None,
            var_no,
            ..
        } => expression(
            ns.constants[*var_no].initializer.as_ref().unwrap(),
            cfg,
            contract_no,
            func,
            ns,
            vartab,
            opt,
        ),
        ast::Expression::Not { loc, expr } => Expression::Not {
            loc: *loc,
            expr: Box::new(expression(expr, cfg, contract_no, func, ns, vartab, opt)),
        },
        ast::Expression::BitwiseNot { loc, ty, expr } => Expression::BitwiseNot {
            loc: *loc,
            ty: ty.clone(),
            expr: Box::new(expression(expr, cfg, contract_no, func, ns, vartab, opt)),
        },
        ast::Expression::Negate {
            loc,
            ty,
            unchecked,
            expr,
        } => Expression::Negate {
            loc: *loc,
            ty: ty.clone(),
            overflowing: *unchecked,
            expr: Box::new(expression(expr, cfg, contract_no, func, ns, vartab, opt)),
        },
        ast::Expression::StructLiteral {
            loc, ty, values, ..
        } => Expression::StructLiteral {
            loc: *loc,
            ty: ty.clone(),
            values: values
                .iter()
                .map(|(_, e)| expression(e, cfg, contract_no, func, ns, vartab, opt))
                .collect(),
        },
        ast::Expression::ArrayLiteral {
            loc,
            ty,
            dimensions,
            values,
        } => Expression::ArrayLiteral {
            loc: *loc,
            ty: ty.clone(),
            dimensions: dimensions.clone(),
            values: values
                .iter()
                .map(|e| expression(e, cfg, contract_no, func, ns, vartab, opt))
                .collect(),
        },
        ast::Expression::ConstArrayLiteral {
            loc,
            ty,
            dimensions,
            values,
        } => Expression::ConstArrayLiteral {
            loc: *loc,
            ty: ty.clone(),
            dimensions: dimensions.clone(),
            values: values
                .iter()
                .map(|e| expression(e, cfg, contract_no, func, ns, vartab, opt))
                .collect(),
        },
        ast::Expression::Assign { left, right, .. } => {
            // If we reach this condition, the assignment is inside an expression.

            if let Some(function) = func {
                if should_remove_assignment(left, function, opt, ns) {
                    return expression(right, cfg, contract_no, func, ns, vartab, opt);
                }
            }

            let mut cfg_right = expression(right, cfg, contract_no, func, ns, vartab, opt);

            // If an assignment where the left hand side is an array, call a helper function that updates the temp variable.
            if let ast::Expression::Variable {
                ty: Type::Array(..),
                var_no,
                ..
            } = &**left
            {
                // If cfg_right is an AllocDynamicArray(_,_,size,_), update it such that it becomes AllocDynamicArray(_,_,temp_var,_) to avoid repetitive expressions in the cfg.
                cfg_right = handle_array_assign(cfg_right, cfg, vartab, *var_no);
            }

            assign_single(left, cfg_right, cfg, contract_no, func, ns, vartab, opt)
        }
        ast::Expression::PreDecrement {
            loc,
            ty,
            unchecked,
            expr: var,
        }
        | ast::Expression::PreIncrement {
            loc,
            ty,
            unchecked,
            expr: var,
        } => pre_incdec(
            vartab,
            ty,
            var,
            cfg,
            contract_no,
            func,
            ns,
            loc,
            expr,
            *unchecked,
            opt,
        ),
        ast::Expression::PostDecrement {
            loc,
            ty,
            unchecked,
            expr: var,
        }
        | ast::Expression::PostIncrement {
            loc,
            ty,
            unchecked,
            expr: var,
        } => post_incdec(
            vartab,
            ty,
            var,
            cfg,
            contract_no,
            func,
            ns,
            loc,
            expr,
            *unchecked,
            opt,
        ),
        ast::Expression::Constructor {
            loc,
            contract_no: constructor_contract,
            constructor_no,
            args,
            call_args,
        } => {
            let address_res = vartab.temp_anonymous(&Type::Contract(*constructor_contract));
            let success = ns
                .target
                .is_polkadot()
                .then(|| vartab.temp_name("success", &Type::Uint(32)));
            call_constructor(
                loc,
                *constructor_contract,
                contract_no,
                constructor_no,
                args,
                call_args,
                address_res,
                success,
                func,
                ns,
                vartab,
                cfg,
                opt,
            );
            if ns.target.is_polkadot() {
                polkadot::RetCodeCheckBuilder::default()
                    .loc(*loc)
                    .msg("contract creation failed")
                    .success_var(success.unwrap())
                    .insert(cfg, vartab)
                    .handle_cases(cfg, ns, opt, vartab);
            }
            Expression::Variable {
                loc: *loc,
                ty: Type::Contract(*constructor_contract),
                var_no: address_res,
            }
        }
        ast::Expression::InternalFunction {
            function_no,
            signature,
            ty,
            ..
        } => {
            let function_no = if let Some(signature) = signature {
                ns.contracts[contract_no].virtual_functions[signature]
                    .last()
                    .unwrap()
            } else {
                function_no
            };

            Expression::InternalFunctionCfg {
                ty: ty.clone(),
                cfg_no: ns.contracts[contract_no].all_functions[function_no],
            }
        }
        ast::Expression::StorageArrayLength {
            loc,
            ty,
            array,
            elem_ty,
        } => {
            let array_ty = array.ty().deref_into();
            let array = expression(array, cfg, contract_no, func, ns, vartab, opt);

            match array_ty {
                Type::Bytes(length) => {
                    let ast_expr = bigint_to_expression(
                        loc,
                        &BigInt::from_u8(length).unwrap(),
                        ns,
                        &mut Diagnostics::default(),
                        ResolveTo::Type(ty),
                        None,
                    )
                    .unwrap();
                    expression(&ast_expr, cfg, contract_no, func, ns, vartab, opt)
                }
                Type::DynamicBytes | Type::String => Expression::StorageArrayLength {
                    loc: *loc,
                    ty: ty.clone(),
                    array: Box::new(array),
                    elem_ty: elem_ty.clone(),
                },
                Type::Array(_, dim) => match dim.last().unwrap() {
                    ArrayLength::Dynamic => {
                        if ns.target == Target::Solana {
                            Expression::StorageArrayLength {
                                loc: *loc,
                                ty: ty.clone(),
                                array: Box::new(array),
                                elem_ty: elem_ty.clone(),
                            }
                        } else {
                            load_storage(loc, &ns.storage_type(), array, cfg, vartab, None)
                        }
                    }
                    ArrayLength::Fixed(length) => {
                        let ast_expr = bigint_to_expression(
                            loc,
                            length,
                            ns,
                            &mut Diagnostics::default(),
                            ResolveTo::Type(ty),
                            None,
                        )
                        .unwrap();
                        expression(&ast_expr, cfg, contract_no, func, ns, vartab, opt)
                    }
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            }
        }
        ast::Expression::Builtin {
            kind: ast::Builtin::ExternalFunctionAddress,
            args: func_expr,
            ..
        } => {
            if let ast::Expression::ExternalFunction { address, .. } = &func_expr[0] {
                expression(address, cfg, contract_no, func, ns, vartab, opt)
            } else {
                let func_expr = expression(&func_expr[0], cfg, contract_no, func, ns, vartab, opt);

                func_expr.external_function_address()
            }
        }
        ast::Expression::Builtin {
            loc,
            kind: ast::Builtin::FunctionSelector,
            args: func_expr,
            ..
        } => match &func_expr[0] {
            ast::Expression::ExternalFunction { function_no, .. }
            | ast::Expression::InternalFunction { function_no, .. } => {
                let selector = ns.functions[*function_no].selector(ns, &contract_no);
                Expression::BytesLiteral {
                    loc: *loc,
                    ty: Type::Bytes(selector.len() as u8),
                    value: selector,
                }
            }
            _ => {
                let func_expr = expression(&func_expr[0], cfg, contract_no, func, ns, vartab, opt);

                func_expr.external_function_selector()
            }
        },
        ast::Expression::EventSelector { loc, ty, event_no } => {
            let emitter = new_event_emitter(loc, *event_no, &[], ns);

            Expression::BytesLiteral {
                loc: *loc,
                ty: ty.clone(),
                value: emitter.selector(contract_no),
            }
        }
        ast::Expression::InternalFunctionCall { .. }
        | ast::Expression::ExternalFunctionCall { .. }
        | ast::Expression::ExternalFunctionCallRaw { .. }
        | ast::Expression::Builtin {
            kind: ast::Builtin::AbiDecode,
            ..
        } => {
            let mut returns = emit_function_call(expr, contract_no, cfg, func, ns, vartab, opt);

            returns.remove(0)
        }
        ast::Expression::ExternalFunction {
            loc,
            ty,
            address,
            function_no,
        } => {
            let address = expression(address, cfg, contract_no, func, ns, vartab, opt);
            let selector = Expression::BytesLiteral {
                loc: *loc,
                ty: Type::Uint(32),
                value: ns.functions[*function_no].selector(ns, &contract_no),
            };
            let struct_literal = Expression::StructLiteral {
                loc: *loc,
                ty: Type::Struct(StructType::ExternalFunction),
                values: vec![selector, address],
            };
            Expression::Cast {
                loc: *loc,
                ty: ty.clone(),
                expr: Box::new(struct_literal),
            }
        }
        ast::Expression::Subscript {
            loc,
            ty: elem_ty,
            array_ty,
            array,
            index,
        } => array_subscript(
            loc,
            elem_ty,
            array_ty,
            array,
            index,
            cfg,
            contract_no,
            func,
            ns,
            vartab,
            opt,
        ),
        ast::Expression::StructMember {
            loc,
            ty,
            expr: var,
            field: field_no,
        } if ty.is_contract_storage() => {
            if let Type::Struct(struct_ty) = var.ty().deref_any() {
                let offset = if ns.target == Target::Solana {
                    struct_ty.definition(ns).storage_offsets[*field_no].clone()
                } else {
                    struct_ty.definition(ns).fields[..*field_no]
                        .iter()
                        .filter(|field| !field.infinite_size)
                        .map(|field| field.ty.storage_slots(ns))
                        .sum()
                };

                Expression::Add {
                    loc: *loc,
                    ty: ns.storage_type(),
                    overflowing: true,
                    left: Box::new(expression(var, cfg, contract_no, func, ns, vartab, opt)),
                    right: Box::new(Expression::NumberLiteral {
                        loc: *loc,
                        ty: ns.storage_type(),
                        value: offset,
                    }),
                }
            } else {
                unreachable!();
            }
        }
        ast::Expression::StructMember {
            loc,
            ty,
            expr: var,
            field: member,
        } => {
            let readonly = if let Type::Struct(struct_type) = var.ty().deref_memory() {
                let definition = struct_type.definition(ns);
                definition.fields[*member].readonly
            } else {
                false
            };

            let member_ty = if readonly {
                Type::Ref(Box::new(ty.clone()))
            } else {
                ty.clone()
            };

            let member_ptr = Expression::StructMember {
                loc: *loc,
                ty: member_ty,
                expr: Box::new(expression(var, cfg, contract_no, func, ns, vartab, opt)),
                member: *member,
            };

            if readonly {
                Expression::Load {
                    loc: *loc,
                    ty: ty.clone(),
                    expr: Box::new(member_ptr),
                }
            } else {
                member_ptr
            }
        }
        ast::Expression::StringCompare { loc, left, right } => Expression::StringCompare {
            loc: *loc,
            left: string_location(left, cfg, contract_no, func, ns, vartab, opt),
            right: string_location(right, cfg, contract_no, func, ns, vartab, opt),
        },
        ast::Expression::Or { loc, left, right } => {
            expr_or(left, cfg, contract_no, func, ns, vartab, loc, right, opt)
        }
        ast::Expression::And { loc, left, right } => {
            and(left, cfg, contract_no, func, ns, vartab, loc, right, opt)
        }
        ast::Expression::CheckingTrunc { loc, to, expr } => {
            checking_trunc(loc, expr, to, cfg, contract_no, func, ns, vartab, opt)
        }
        ast::Expression::Trunc { loc, to, expr } => Expression::Trunc {
            loc: *loc,
            ty: to.clone(),
            expr: Box::new(expression(expr, cfg, contract_no, func, ns, vartab, opt)),
        },
        ast::Expression::ZeroExt { loc, to, expr } => Expression::ZeroExt {
            loc: *loc,
            ty: to.clone(),
            expr: Box::new(expression(expr, cfg, contract_no, func, ns, vartab, opt)),
        },
        ast::Expression::SignExt { loc, to, expr } => Expression::SignExt {
            loc: *loc,
            ty: to.clone(),
            expr: Box::new(expression(expr, cfg, contract_no, func, ns, vartab, opt)),
        },
        ast::Expression::Cast { loc, to, expr } if matches!(to, Type::Address(_)) => {
            let mut diagnostics = Diagnostics::default();
            if let Ok((_, address)) = eval_const_number(expr, ns, &mut diagnostics) {
                Expression::NumberLiteral {
                    loc: *loc,
                    ty: to.clone(),
                    value: address,
                }
            } else {
                Expression::Cast {
                    loc: *loc,
                    ty: to.clone(),
                    expr: Box::new(expression(expr, cfg, contract_no, func, ns, vartab, opt)),
                }
            }
        }
        ast::Expression::Cast { to, expr, .. }
            if matches!((expr.ty(), to), (Type::Address(_), Type::Contract(_))) =>
        {
            // Address and Contract have the same underlying type. CSE will create
            // a temporary to replace multiple casts from address to Contract, which have no
            // real purpose.
            expression(expr, cfg, contract_no, func, ns, vartab, opt)
        }
        ast::Expression::Cast { loc, to, expr }
            if matches!(to, Type::Array(..))
                && matches!(**expr, ast::Expression::ArrayLiteral { .. }) =>
        {
            let codegen_expr = expression(expr, cfg, contract_no, func, ns, vartab, opt);
            array_literal_to_memory_array(loc, &codegen_expr, to, cfg, vartab)
        }
        ast::Expression::Cast { loc, to, expr } => {
            if expr.ty() == Type::Rational {
                let (_, n) = eval_const_rational(expr, ns).unwrap();

                Expression::NumberLiteral {
                    loc: *loc,
                    ty: to.clone(),
                    value: n.to_integer(),
                }
            } else if matches!(to, Type::String | Type::DynamicBytes)
                && matches!(expr.ty(), Type::String | Type::DynamicBytes)
            {
                expression(expr, cfg, contract_no, func, ns, vartab, opt)
            } else {
                Expression::Cast {
                    loc: *loc,
                    ty: to.clone(),
                    expr: Box::new(expression(expr, cfg, contract_no, func, ns, vartab, opt)),
                }
            }
        }
        ast::Expression::BytesCast {
            loc,
            to,
            from,
            expr,
        } => Expression::BytesCast {
            loc: *loc,
            ty: to.clone(),
            from: from.clone(),
            expr: Box::new(expression(expr, cfg, contract_no, func, ns, vartab, opt)),
        },
        ast::Expression::Load { loc, ty, expr: e } => Expression::Load {
            loc: *loc,
            ty: ty.clone(),
            expr: Box::new(expression(e, cfg, contract_no, func, ns, vartab, opt)),
        },
        // for some built-ins, we have to inline special case code
        ast::Expression::Builtin {
            kind: ast::Builtin::UserTypeWrap,
            args,
            ..
        }
        | ast::Expression::Builtin {
            kind: ast::Builtin::UserTypeUnwrap,
            args,
            ..
        } => expression(&args[0], cfg, contract_no, func, ns, vartab, opt),
        ast::Expression::Builtin {
            loc,
            tys: ty,
            kind: ast::Builtin::ArrayPush,
            args,
        } => {
            if args[0].ty().is_contract_storage() {
                if ns.target == Target::Solana || args[0].ty().is_storage_bytes() {
                    array_push(loc, args, cfg, contract_no, func, ns, vartab, opt)
                } else {
                    storage_slots_array_push(loc, args, cfg, contract_no, func, ns, vartab, opt)
                }
            } else {
                let second_arg = if args.len() > 1 {
                    expression(&args[1], cfg, contract_no, func, ns, vartab, opt)
                } else {
                    ty[0].default(ns).unwrap()
                };
                memory_array_push(
                    &ty[0],
                    vartab,
                    &args[0],
                    cfg,
                    contract_no,
                    func,
                    ns,
                    second_arg,
                    loc,
                    opt,
                )
            }
        }
        ast::Expression::Builtin {
            loc,
            tys: ty,
            kind: ast::Builtin::ArrayPop,
            args,
        } => {
            if args[0].ty().is_contract_storage() {
                if ns.target == Target::Solana || args[0].ty().is_storage_bytes() {
                    array_pop(loc, args, &ty[0], cfg, contract_no, func, ns, vartab, opt)
                } else {
                    storage_slots_array_pop(
                        loc,
                        args,
                        &ty[0],
                        cfg,
                        contract_no,
                        func,
                        ns,
                        vartab,
                        opt,
                    )
                }
            } else {
                let address_res = vartab.temp_anonymous(&ty[0]);

                let array_pos = match expression(&args[0], cfg, contract_no, func, ns, vartab, opt)
                {
                    Expression::Variable { var_no, .. } => {
                        vartab.set_dirty(var_no);

                        var_no
                    }
                    _ => unreachable!(),
                };

                cfg.add(
                    vartab,
                    Instr::PopMemory {
                        res: address_res,
                        ty: args[0].ty(),
                        array: array_pos,
                        loc: *loc,
                    },
                );
                cfg.modify_temp_array_length(*loc, true, array_pos, vartab);

                Expression::Variable {
                    loc: *loc,
                    ty: ty[0].clone(),
                    var_no: address_res,
                }
            }
        }
        ast::Expression::Builtin {
            kind: ast::Builtin::Assert,
            args,
            ..
        } => expr_assert(cfg, &args[0], contract_no, func, ns, vartab, opt),
        ast::Expression::Builtin {
            kind: ast::Builtin::Print,
            args,
            ..
        } => {
            if opt.log_prints {
                let expr = expression(&args[0], cfg, contract_no, func, ns, vartab, opt);

                let to_print = if ns.target.is_polkadot() {
                    add_prefix_and_delimiter_to_print(expr)
                } else {
                    expr
                };

                let res = if let Expression::AllocDynamicBytes {
                    loc,
                    ty,
                    size: _,
                    initializer: Some(initializer),
                } = &to_print
                {
                    Expression::BytesLiteral {
                        loc: *loc,
                        ty: ty.clone(),
                        value: initializer.to_vec(),
                    }
                } else {
                    to_print
                };

                cfg.add(vartab, Instr::Print { expr: res });
            }

            Expression::Poison
        }
        ast::Expression::Builtin {
            kind: ast::Builtin::Require,
            args,
            ..
        } => require(cfg, args, contract_no, func, ns, vartab, opt, expr.loc()),
        ast::Expression::Builtin {
            kind: ast::Builtin::SelfDestruct,
            args,
            ..
        } => self_destruct(args, cfg, contract_no, func, ns, vartab, opt),
        ast::Expression::Builtin {
            loc,
            kind: ast::Builtin::PayableSend,
            args,
            ..
        } => payable_send(args, cfg, contract_no, func, ns, vartab, loc, opt),
        ast::Expression::Builtin {
            loc,
            kind: ast::Builtin::PayableTransfer,
            args,
            ..
        } => payable_transfer(args, cfg, contract_no, func, ns, vartab, loc, opt),
        ast::Expression::Builtin {
            loc,
            kind: ast::Builtin::AbiEncode,
            args,
            ..
        } => abi_encode_many(args, cfg, contract_no, func, ns, vartab, loc, opt),
        ast::Expression::Builtin {
            loc,
            kind: ast::Builtin::AbiEncodePacked,
            args,
            ..
        } => abi_encode_packed(args, cfg, contract_no, func, ns, vartab, loc, opt),
        ast::Expression::Builtin {
            loc,
            kind: ast::Builtin::AbiEncodeWithSelector,
            args,
            ..
        } => abi_encode_with_selector(args, cfg, contract_no, func, ns, vartab, loc, opt),
        ast::Expression::Builtin {
            loc,
            kind: ast::Builtin::AbiEncodeWithSignature,
            args,
            ..
        } => abi_encode_with_signature(args, loc, cfg, contract_no, func, ns, vartab, opt),
        ast::Expression::Builtin {
            loc,
            kind: ast::Builtin::AbiEncodeCall,
            args,
            ..
        } => abi_encode_call(args, cfg, contract_no, func, ns, vartab, loc, opt),
        // The Polkadot gas price builtin takes an argument; the others do not
        ast::Expression::Builtin {
            loc,
            kind: ast::Builtin::Gasprice,
            args: expr,
            ..
        } if expr.len() == 1 && ns.target == Target::EVM => {
            builtin_evm_gasprice(loc, expr, cfg, contract_no, func, ns, vartab, opt)
        }
        ast::Expression::Builtin {
            loc,
            tys,
            kind: ast::Builtin::TypeMin | ast::Builtin::TypeMax,
            ..
        } => {
            let Ok((_, value)) = eval_const_number(expr, ns, &mut Diagnostics::default()) else {
                unreachable!();
            };

            Expression::NumberLiteral {
                loc: *loc,
                ty: tys[0].clone(),
                value,
            }
        }
        ast::Expression::Builtin {
            loc,
            tys,
            kind,
            args,
        } => expr_builtin(
            args,
            cfg,
            contract_no,
            func,
            ns,
            vartab,
            loc,
            tys,
            *kind,
            opt,
        ),
        ast::Expression::FormatString { loc, format: args } => {
            format_string(args, cfg, contract_no, func, ns, vartab, loc, opt)
        }
        ast::Expression::AllocDynamicBytes {
            loc,
            ty,
            length: size,
            init,
        } => alloc_dynamic_array(size, cfg, contract_no, func, ns, vartab, loc, ty, init, opt),
        ast::Expression::ConditionalOperator {
            loc,
            ty,
            cond,
            true_option: left,
            false_option: right,
        } => conditional_operator(
            loc,
            ty,
            cond,
            cfg,
            contract_no,
            func,
            ns,
            vartab,
            left,
            right,
            opt,
        ),
        ast::Expression::BoolLiteral { loc, value } => Expression::BoolLiteral {
            loc: *loc,
            value: *value,
        },
        ast::Expression::BytesLiteral { loc, ty, value } => Expression::BytesLiteral {
            loc: *loc,
            ty: ty.clone(),
            value: value.clone(),
        },
        ast::Expression::NumberLiteral { loc, ty, value } => Expression::NumberLiteral {
            loc: *loc,
            ty: ty.clone(),
            value: value.clone(),
        },
        ast::Expression::RationalNumberLiteral { loc, ty, value } => {
            Expression::RationalNumberLiteral {
                loc: *loc,
                ty: ty.clone(),
                rational: value.clone(),
            }
        }
        ast::Expression::Variable { loc, ty, var_no } => Expression::Variable {
            loc: *loc,
            ty: ty.clone(),
            var_no: *var_no,
        },
        ast::Expression::GetRef { loc, ty, expr: exp } => Expression::GetRef {
            loc: *loc,
            ty: ty.clone(),
            expr: Box::new(expression(exp, cfg, contract_no, func, ns, vartab, opt)),
        },
        ast::Expression::UserDefinedOperator {
            loc,
            ty,
            function_no,
            args,
            ..
        } => {
            let var = vartab.temp_anonymous(ty);
            let cfg_no = ns.contracts[contract_no].all_functions[function_no];
            let args = args
                .iter()
                .map(|a| expression(a, cfg, contract_no, func, ns, vartab, opt))
                .collect::<Vec<Expression>>();

            cfg.add(
                vartab,
                Instr::Call {
                    res: vec![var],
                    call: InternalCallTy::Static { cfg_no },
                    args,
                    return_tys: vec![ty.clone()],
                },
            );
            Expression::Variable {
                loc: *loc,
                ty: ty.clone(),
                var_no: var,
            }
        }
        ast::Expression::NamedMember {
            loc, array, name, ..
        } => {
            // This expression should only exist for Solana's AccountInfo array
            assert_eq!(
                array.ty().deref_memory(),
                &Type::Array(
                    Box::new(Type::Struct(StructType::AccountInfo)),
                    vec![ArrayLength::Dynamic]
                )
            );
            // Variables do not really occupy space in the stack. We forward expressions in emit
            // without allocating memory whenever we use a variable.
            let ty = Type::Ref(Box::new(Type::Struct(StructType::AccountInfo)));
            let var_placeholder = vartab.temp_anonymous(&ty);
            cfg.add(
                vartab,
                Instr::AccountAccess {
                    loc: *loc,
                    name: name.clone(),
                    var_no: var_placeholder,
                },
            );

            Expression::Variable {
                loc: *loc,
                var_no: var_placeholder,
                ty,
            }
        }
        ast::Expression::TypeOperator { .. } | ast::Expression::List { .. } => {
            unreachable!("List and Type Operator shall not appear in the CFG")
        }
    }
}

fn memory_array_push(
    ty: &Type,
    vartab: &mut Vartable,
    array: &ast::Expression,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    value: Expression,
    loc: &pt::Loc,
    opt: &Options,
) -> Expression {
    let address_res = vartab.temp_anonymous(ty);
    let array_pos = match expression(array, cfg, contract_no, func, ns, vartab, opt) {
        Expression::Variable { var_no, .. } => {
            vartab.set_dirty(var_no);

            var_no
        }
        _ => unreachable!(),
    };
    cfg.add(
        vartab,
        Instr::PushMemory {
            res: address_res,
            ty: array.ty(),
            array: array_pos,
            value: Box::new(value),
        },
    );
    cfg.modify_temp_array_length(*loc, false, array_pos, vartab);

    Expression::Variable {
        loc: *loc,
        ty: ty.clone(),
        var_no: address_res,
    }
}

fn post_incdec(
    vartab: &mut Vartable,
    ty: &Type,
    var: &ast::Expression,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    loc: &pt::Loc,
    expr: &ast::Expression,
    overflowing: bool,
    opt: &Options,
) -> Expression {
    let res = vartab.temp_anonymous(ty);
    let v = expression(var, cfg, contract_no, func, ns, vartab, opt);

    let storage_type = storage_type(var, ns);

    let v = match var.ty() {
        Type::Ref(ty) => Expression::Load {
            loc: var.loc(),
            ty: ty.as_ref().clone(),
            expr: Box::new(v),
        },
        Type::StorageRef(_, ty) => load_storage(
            &var.loc(),
            ty.as_ref(),
            v,
            cfg,
            vartab,
            storage_type.clone(),
        ),
        _ => v,
    };
    cfg.add(
        vartab,
        Instr::Set {
            loc: v.loc(),
            res,
            expr: v,
        },
    );
    let one = Box::new(Expression::NumberLiteral {
        loc: *loc,
        ty: ty.clone(),
        value: BigInt::one(),
    });
    let expr = match expr {
        ast::Expression::PostDecrement { .. } => Expression::Subtract {
            loc: *loc,
            ty: ty.clone(),
            overflowing,
            left: Box::new(Expression::Variable {
                loc: *loc,
                ty: ty.clone(),
                var_no: res,
            }),
            right: one,
        },
        ast::Expression::PostIncrement { .. } => Expression::Add {
            loc: *loc,
            ty: ty.clone(),
            overflowing,
            left: Box::new(Expression::Variable {
                loc: *loc,
                ty: ty.clone(),
                var_no: res,
            }),
            right: one,
        },
        _ => unreachable!(),
    };
    match var {
        ast::Expression::Variable { var_no, .. } => {
            cfg.add(
                vartab,
                Instr::Set {
                    loc: expr.loc(),
                    res: *var_no,
                    expr,
                },
            );
        }
        _ => {
            let dest = expression(var, cfg, contract_no, func, ns, vartab, opt);
            let res = vartab.temp_anonymous(ty);
            cfg.add(
                vartab,
                Instr::Set {
                    loc: expr.loc(),
                    res,
                    expr,
                },
            );

            match var.ty() {
                Type::StorageRef(..) => {
                    cfg.add(
                        vartab,
                        Instr::SetStorage {
                            value: Expression::Variable {
                                loc: *loc,
                                ty: ty.clone(),
                                var_no: res,
                            },
                            ty: ty.clone(),
                            storage: dest,
                            storage_type,
                        },
                    );
                }
                Type::Ref(_) => {
                    cfg.add(
                        vartab,
                        Instr::Store {
                            dest,
                            data: Expression::Variable {
                                loc: Loc::Codegen,
                                ty: ty.clone(),
                                var_no: res,
                            },
                        },
                    );
                }
                _ => unreachable!(),
            }
        }
    }
    Expression::Variable {
        loc: *loc,
        ty: ty.clone(),
        var_no: res,
    }
}

fn pre_incdec(
    vartab: &mut Vartable,
    ty: &Type,
    var: &ast::Expression,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    loc: &pt::Loc,
    expr: &ast::Expression,
    overflowing: bool,
    opt: &Options,
) -> Expression {
    let res = vartab.temp_anonymous(ty);
    let v = expression(var, cfg, contract_no, func, ns, vartab, opt);
    let storage_type = storage_type(var, ns);
    let v = match var.ty() {
        Type::Ref(ty) => Expression::Load {
            loc: var.loc(),
            ty: ty.as_ref().clone(),
            expr: Box::new(v),
        },
        Type::StorageRef(_, ty) => load_storage(
            &var.loc(),
            ty.as_ref(),
            v,
            cfg,
            vartab,
            storage_type.clone(),
        ),
        _ => v,
    };
    let one = Box::new(Expression::NumberLiteral {
        loc: *loc,
        ty: ty.clone(),
        value: BigInt::one(),
    });
    let expr = match expr {
        ast::Expression::PreDecrement { .. } => Expression::Subtract {
            loc: *loc,
            ty: ty.clone(),
            overflowing,
            left: Box::new(v),
            right: one,
        },
        ast::Expression::PreIncrement { .. } => Expression::Add {
            loc: *loc,
            ty: ty.clone(),
            overflowing,
            left: Box::new(v),
            right: one,
        },
        _ => unreachable!(),
    };
    cfg.add(
        vartab,
        Instr::Set {
            loc: expr.loc(),
            res,
            expr,
        },
    );

    match var {
        ast::Expression::Variable { loc, var_no, .. } => {
            cfg.add(
                vartab,
                Instr::Set {
                    loc: *loc,
                    res: *var_no,
                    expr: Expression::Variable {
                        loc: *loc,
                        ty: ty.clone(),
                        var_no: res,
                    },
                },
            );
        }
        _ => {
            let dest = expression(var, cfg, contract_no, func, ns, vartab, opt);

            match var.ty() {
                Type::StorageRef(..) => {
                    cfg.add(
                        vartab,
                        Instr::SetStorage {
                            value: Expression::Variable {
                                loc: *loc,
                                ty: ty.clone(),
                                var_no: res,
                            },
                            ty: ty.clone(),
                            storage: dest,
                            storage_type: storage_type.clone(),
                        },
                    );
                }
                Type::Ref(_) => {
                    cfg.add(
                        vartab,
                        Instr::Store {
                            dest,
                            data: Expression::Variable {
                                loc: Loc::Codegen,
                                ty: ty.clone(),
                                var_no: res,
                            },
                        },
                    );
                }
                _ => unreachable!(),
            }
        }
    }
    Expression::Variable {
        loc: *loc,
        ty: ty.clone(),
        var_no: res,
    }
}

fn expr_or(
    left: &ast::Expression,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    loc: &pt::Loc,
    right: &ast::Expression,
    opt: &Options,
) -> Expression {
    let l = expression(left, cfg, contract_no, func, ns, vartab, opt);
    let pos = vartab.temp(
        &pt::Identifier {
            name: "or".to_owned(),
            loc: *loc,
        },
        &Type::Bool,
    );
    vartab.new_dirty_tracker();
    let right_side = cfg.new_basic_block("or_right_side".to_string());
    let end_or = cfg.new_basic_block("or_end".to_string());
    cfg.add(
        vartab,
        Instr::Set {
            loc: *loc,
            res: pos,
            expr: Expression::BoolLiteral {
                loc: *loc,
                value: true,
            },
        },
    );
    cfg.add(
        vartab,
        Instr::BranchCond {
            cond: l,
            true_block: end_or,
            false_block: right_side,
        },
    );
    cfg.set_basic_block(right_side);
    let r = expression(right, cfg, contract_no, func, ns, vartab, opt);
    cfg.add(
        vartab,
        Instr::Set {
            loc: r.loc(),
            res: pos,
            expr: r,
        },
    );
    cfg.add(vartab, Instr::Branch { block: end_or });
    cfg.set_basic_block(end_or);
    cfg.set_phis(end_or, vartab.pop_dirty_tracker());
    Expression::Variable {
        loc: *loc,
        ty: Type::Bool,
        var_no: pos,
    }
}

fn and(
    left: &ast::Expression,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    loc: &pt::Loc,
    right: &ast::Expression,
    opt: &Options,
) -> Expression {
    let l = expression(left, cfg, contract_no, func, ns, vartab, opt);
    let pos = vartab.temp(
        &pt::Identifier {
            name: "and".to_owned(),
            loc: *loc,
        },
        &Type::Bool,
    );
    vartab.new_dirty_tracker();
    let right_side = cfg.new_basic_block("and_right_side".to_string());
    let end_and = cfg.new_basic_block("and_end".to_string());
    cfg.add(
        vartab,
        Instr::Set {
            loc: *loc,
            res: pos,
            expr: Expression::BoolLiteral {
                loc: *loc,
                value: false,
            },
        },
    );
    cfg.add(
        vartab,
        Instr::BranchCond {
            cond: l,
            true_block: right_side,
            false_block: end_and,
        },
    );
    cfg.set_basic_block(right_side);
    let r = expression(right, cfg, contract_no, func, ns, vartab, opt);
    cfg.add(
        vartab,
        Instr::Set {
            loc: r.loc(),
            res: pos,
            expr: r,
        },
    );
    cfg.add(vartab, Instr::Branch { block: end_and });
    cfg.set_basic_block(end_and);
    cfg.set_phis(end_and, vartab.pop_dirty_tracker());
    Expression::Variable {
        loc: *loc,
        ty: Type::Bool,
        var_no: pos,
    }
}

fn self_destruct(
    args: &[ast::Expression],
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
) -> Expression {
    let recipient = expression(&args[0], cfg, contract_no, func, ns, vartab, opt);
    cfg.add(vartab, Instr::SelfDestruct { recipient });
    Expression::Poison
}

fn payable_send(
    args: &[ast::Expression],
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    loc: &pt::Loc,
    opt: &Options,
) -> Expression {
    let address = expression(&args[0], cfg, contract_no, func, ns, vartab, opt);
    let value = expression(&args[1], cfg, contract_no, func, ns, vartab, opt);
    let success = vartab.temp(
        &pt::Identifier {
            loc: *loc,
            name: "success".to_owned(),
        },
        &Type::Uint(32),
    );

    // Ethereum can only transfer via external call
    if ns.target == Target::EVM {
        cfg.add(
            vartab,
            Instr::ExternalCall {
                loc: *loc,
                success: Some(success),
                address: Some(address),
                accounts: ExternalCallAccounts::AbsentArgument,
                seeds: None,
                payload: Expression::AllocDynamicBytes {
                    loc: *loc,
                    ty: Type::DynamicBytes,
                    size: Box::new(Expression::NumberLiteral {
                        loc: *loc,
                        ty: Type::Uint(32),
                        value: BigInt::from(0),
                    }),
                    initializer: Some(vec![]),
                },
                value,
                gas: Expression::NumberLiteral {
                    loc: *loc,
                    ty: Type::Uint(64),
                    value: BigInt::from(i64::MAX),
                },
                callty: CallTy::Regular,
                contract_function_no: None,
                flags: None,
            },
        );
        return Expression::Variable {
            loc: *loc,
            ty: Type::Bool,
            var_no: success,
        };
    }

    cfg.add(
        vartab,
        Instr::ValueTransfer {
            success: Some(success),
            address,
            value,
        },
    );

    if ns.target != Target::Solana {
        polkadot::check_transfer_ret(loc, success, cfg, ns, opt, vartab, false).unwrap()
    } else {
        unreachable!("Value transfer does not exist on Solana");
    }
}

fn payable_transfer(
    args: &[ast::Expression],
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    loc: &pt::Loc,
    opt: &Options,
) -> Expression {
    let address = expression(&args[0], cfg, contract_no, func, ns, vartab, opt);
    let value = expression(&args[1], cfg, contract_no, func, ns, vartab, opt);
    if ns.target == Target::EVM {
        // Ethereum can only transfer via external call
        cfg.add(
            vartab,
            Instr::ExternalCall {
                loc: *loc,
                success: None,
                accounts: ExternalCallAccounts::AbsentArgument,
                seeds: None,
                address: Some(address),
                payload: Expression::AllocDynamicBytes {
                    loc: *loc,
                    ty: Type::DynamicBytes,
                    size: Box::new(Expression::NumberLiteral {
                        loc: *loc,
                        ty: Type::Uint(32),
                        value: BigInt::from(0),
                    }),
                    initializer: Some(vec![]),
                },
                value,
                gas: Expression::NumberLiteral {
                    loc: *loc,
                    ty: Type::Uint(64),
                    value: BigInt::from(i64::MAX),
                },
                callty: CallTy::Regular,
                contract_function_no: None,
                flags: None,
            },
        );
        return Expression::Poison;
    }

    let success = ns
        .target
        .is_polkadot()
        .then(|| vartab.temp_name("success", &Type::Uint(32)));
    let ins = Instr::ValueTransfer {
        success,
        address,
        value,
    };
    cfg.add(vartab, ins);

    if ns.target.is_polkadot() {
        polkadot::check_transfer_ret(loc, success.unwrap(), cfg, ns, opt, vartab, true);
    }

    Expression::Poison
}

fn abi_encode_many(
    args: &[ast::Expression],
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    loc: &pt::Loc,
    opt: &Options,
) -> Expression {
    let args = args
        .iter()
        .map(|v| expression(v, cfg, contract_no, func, ns, vartab, opt))
        .collect::<Vec<Expression>>();

    abi_encode(loc, args, ns, vartab, cfg, false).0
}

fn abi_encode_packed(
    args: &[ast::Expression],
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    loc: &pt::Loc,
    opt: &Options,
) -> Expression {
    let packed = args
        .iter()
        .map(|v| expression(v, cfg, contract_no, func, ns, vartab, opt))
        .collect::<Vec<Expression>>();

    let (encoded, _) = abi_encode(loc, packed, ns, vartab, cfg, true);
    encoded
}

fn encode_many_with_selector(
    loc: &pt::Loc,
    selector: Expression,
    mut args: Vec<Expression>,
    ns: &Namespace,
    vartab: &mut Vartable,
    cfg: &mut ControlFlowGraph,
) -> Expression {
    let mut encoder_args: Vec<Expression> = Vec::with_capacity(args.len() + 1);
    encoder_args.push(selector);
    encoder_args.append(&mut args);
    abi_encode(loc, encoder_args, ns, vartab, cfg, false).0
}

fn abi_encode_with_selector(
    args: &[ast::Expression],
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    loc: &pt::Loc,
    opt: &Options,
) -> Expression {
    let mut args_iter = args.iter();
    let selector = expression(
        args_iter.next().unwrap(),
        cfg,
        contract_no,
        func,
        ns,
        vartab,
        opt,
    );
    let args = args_iter
        .map(|v| expression(v, cfg, contract_no, func, ns, vartab, opt))
        .collect::<Vec<Expression>>();
    encode_many_with_selector(loc, selector, args, ns, vartab, cfg)
}

fn abi_encode_with_signature(
    args: &[ast::Expression],
    loc: &pt::Loc,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
) -> Expression {
    let mut args_iter = args.iter();
    let hash_algorithm = if ns.target == Target::Solana {
        ast::Builtin::Sha256
    } else {
        ast::Builtin::Keccak256
    };

    let hash = ast::Expression::Builtin {
        loc: *loc,
        tys: vec![Type::Bytes(32)],
        kind: hash_algorithm,
        args: vec![args_iter.next().unwrap().clone()],
    };
    let hash = expression(&hash, cfg, contract_no, func, ns, vartab, opt);
    let selector = hash.cast(&Type::FunctionSelector, ns);
    let args = args_iter
        .map(|v| expression(v, cfg, contract_no, func, ns, vartab, opt))
        .collect::<Vec<Expression>>();
    encode_many_with_selector(loc, selector, args, ns, vartab, cfg)
}

fn abi_encode_call(
    args: &[ast::Expression],
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    loc: &pt::Loc,
    opt: &Options,
) -> Expression {
    let mut args_iter = args.iter();
    let selector = expression(
        &ast::Expression::Builtin {
            loc: *loc,
            tys: vec![Type::FunctionSelector],
            kind: ast::Builtin::FunctionSelector,
            args: vec![args_iter.next().unwrap().clone()],
        },
        cfg,
        contract_no,
        func,
        ns,
        vartab,
        opt,
    );
    let args = args_iter
        .map(|v| expression(v, cfg, contract_no, func, ns, vartab, opt))
        .collect::<Vec<Expression>>();
    encode_many_with_selector(loc, selector, args, ns, vartab, cfg)
}

fn builtin_evm_gasprice(
    loc: &pt::Loc,
    expr: &[ast::Expression],
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
) -> Expression {
    let ty = Type::Value;
    let gasprice = Expression::Builtin {
        loc: *loc,
        tys: vec![ty.clone()],
        kind: Builtin::Gasprice,
        args: vec![],
    };
    let units = expression(&expr[0], cfg, contract_no, func, ns, vartab, opt);
    Expression::Multiply {
        loc: *loc,
        ty,
        overflowing: true,
        left: Box::new(units),
        right: Box::new(gasprice),
    }
}

fn expr_builtin(
    args: &[ast::Expression],
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    loc: &pt::Loc,
    tys: &[Type],
    builtin: ast::Builtin,
    opt: &Options,
) -> Expression {
    match builtin {
        ast::Builtin::WriteInt8
        | ast::Builtin::WriteInt16LE
        | ast::Builtin::WriteInt32LE
        | ast::Builtin::WriteInt64LE
        | ast::Builtin::WriteInt128LE
        | ast::Builtin::WriteInt256LE
        | ast::Builtin::WriteAddress
        | ast::Builtin::WriteUint16LE
        | ast::Builtin::WriteUint32LE
        | ast::Builtin::WriteUint64LE
        | ast::Builtin::WriteUint128LE
        | ast::Builtin::WriteUint256LE => {
            let buf = expression(&args[0], cfg, contract_no, func, ns, vartab, opt);
            let offset = expression(&args[2], cfg, contract_no, func, ns, vartab, opt);

            // range check
            let cond = Expression::LessEqual {
                loc: *loc,
                signed: false,
                left: Box::new(Expression::Add {
                    loc: *loc,
                    ty: Type::Uint(32),
                    overflowing: false,
                    left: Box::new(offset.clone()),
                    right: Box::new(Expression::NumberLiteral {
                        loc: *loc,
                        ty: Type::Uint(32),
                        value: BigInt::from(args[1].ty().bits(ns) / 8),
                    }),
                }),
                right: Box::new(Expression::Builtin {
                    loc: *loc,
                    tys: vec![Type::Uint(32)],
                    kind: Builtin::ArrayLength,
                    args: vec![buf.clone()],
                }),
            };

            let out_of_bounds = cfg.new_basic_block("out_of_bounds".to_string());
            let in_bounds = cfg.new_basic_block("in_bounds".to_string());

            cfg.add(
                vartab,
                Instr::BranchCond {
                    cond,
                    true_block: in_bounds,
                    false_block: out_of_bounds,
                },
            );

            cfg.set_basic_block(out_of_bounds);
            log_runtime_error(
                opt.log_runtime_errors,
                "integer too large to write in buffer",
                *loc,
                cfg,
                vartab,
                ns,
            );
            let error = SolidityError::Panic(PanicCode::ArrayIndexOob);
            assert_failure(loc, error, ns, cfg, vartab);

            cfg.set_basic_block(in_bounds);

            let value = expression(&args[1], cfg, contract_no, func, ns, vartab, opt);
            cfg.add(vartab, Instr::WriteBuffer { buf, value, offset });

            Expression::Undefined { ty: tys[0].clone() }
        }
        ast::Builtin::WriteBytes | ast::Builtin::WriteString => {
            let buffer = expression(&args[0], cfg, contract_no, func, ns, vartab, opt);
            let data = expression(&args[1], cfg, contract_no, func, ns, vartab, opt);
            let offset = expression(&args[2], cfg, contract_no, func, ns, vartab, opt);

            let size = Expression::Builtin {
                loc: *loc,
                tys: vec![Type::Uint(32)],
                kind: Builtin::ArrayLength,
                args: vec![data.clone()],
            };

            let cond = Expression::LessEqual {
                loc: *loc,
                signed: false,
                left: Box::new(Expression::Add {
                    loc: *loc,
                    ty: Type::Uint(32),
                    overflowing: false,
                    left: Box::new(offset.clone()),
                    right: Box::new(size.clone()),
                }),
                right: Box::new(Expression::Builtin {
                    loc: *loc,
                    tys: vec![Type::Uint(32)],
                    kind: Builtin::ArrayLength,
                    args: vec![buffer.clone()],
                }),
            };

            let in_bounds = cfg.new_basic_block("in_bounds".to_string());
            let out_ouf_bounds = cfg.new_basic_block("out_of_bounds".to_string());

            cfg.add(
                vartab,
                Instr::BranchCond {
                    cond,
                    true_block: in_bounds,
                    false_block: out_ouf_bounds,
                },
            );

            cfg.set_basic_block(out_ouf_bounds);
            log_runtime_error(
                opt.log_runtime_errors,
                "data does not fit into buffer",
                *loc,
                cfg,
                vartab,
                ns,
            );
            let error = SolidityError::Panic(PanicCode::ArrayIndexOob);
            assert_failure(loc, error, ns, cfg, vartab);

            cfg.set_basic_block(in_bounds);
            let advanced_ptr = Expression::AdvancePointer {
                pointer: Box::new(buffer),
                bytes_offset: Box::new(offset),
            };

            cfg.add(
                vartab,
                Instr::MemCopy {
                    source: data,
                    destination: advanced_ptr,
                    bytes: size,
                },
            );
            Expression::Undefined { ty: tys[0].clone() }
        }
        ast::Builtin::ReadInt8
        | ast::Builtin::ReadInt16LE
        | ast::Builtin::ReadInt32LE
        | ast::Builtin::ReadInt64LE
        | ast::Builtin::ReadInt128LE
        | ast::Builtin::ReadInt256LE
        | ast::Builtin::ReadAddress
        | ast::Builtin::ReadUint16LE
        | ast::Builtin::ReadUint32LE
        | ast::Builtin::ReadUint64LE
        | ast::Builtin::ReadUint128LE
        | ast::Builtin::ReadUint256LE => {
            let buf = expression(&args[0], cfg, contract_no, func, ns, vartab, opt);
            let offset = expression(&args[1], cfg, contract_no, func, ns, vartab, opt);

            // range check
            let cond = Expression::LessEqual {
                loc: *loc,
                signed: false,
                left: Box::new(Expression::Add {
                    loc: *loc,
                    ty: Type::Uint(32),
                    overflowing: false,
                    left: Box::new(offset.clone()),
                    right: Box::new(Expression::NumberLiteral {
                        loc: *loc,
                        ty: Type::Uint(32),
                        value: BigInt::from(tys[0].bits(ns) / 8),
                    }),
                }),
                right: Box::new(Expression::Builtin {
                    loc: *loc,
                    tys: vec![Type::Uint(32)],
                    kind: Builtin::ArrayLength,
                    args: vec![buf.clone()],
                }),
            };

            let out_of_bounds = cfg.new_basic_block("out_of_bounds".to_string());
            let in_bounds = cfg.new_basic_block("in_bounds".to_string());

            cfg.add(
                vartab,
                Instr::BranchCond {
                    cond,
                    true_block: in_bounds,
                    false_block: out_of_bounds,
                },
            );

            cfg.set_basic_block(out_of_bounds);
            log_runtime_error(
                opt.log_runtime_errors,
                "read integer out of bounds",
                *loc,
                cfg,
                vartab,
                ns,
            );
            let error = SolidityError::Panic(PanicCode::ArrayIndexOob);
            assert_failure(loc, error, ns, cfg, vartab);

            cfg.set_basic_block(in_bounds);

            Expression::Builtin {
                loc: *loc,
                tys: tys.to_vec(),
                kind: (&builtin).into(),
                args: vec![buf, offset],
            }
        }
        ast::Builtin::AddMod | ast::Builtin::MulMod => {
            let arguments: Vec<Expression> = args
                .iter()
                .map(|v| expression(v, cfg, contract_no, func, ns, vartab, opt))
                .collect();

            let temp = vartab.temp_anonymous(&tys[0]);
            let zero = Expression::NumberLiteral {
                loc: *loc,
                ty: tys[0].clone(),
                value: BigInt::zero(),
            };
            let cond = Expression::NotEqual {
                loc: *loc,
                left: Box::new(zero.clone()),
                right: Box::new(arguments[2].clone()),
            };

            let true_block = cfg.new_basic_block("builtin_call".to_string());
            let false_block = cfg.new_basic_block("zero".to_string());
            let end_if = cfg.new_basic_block("end_if".to_string());

            cfg.add(
                vartab,
                Instr::BranchCond {
                    cond,
                    true_block,
                    false_block,
                },
            );

            cfg.set_basic_block(true_block);
            vartab.new_dirty_tracker();

            cfg.add(
                vartab,
                Instr::Set {
                    loc: *loc,
                    res: temp,
                    expr: Expression::Builtin {
                        loc: *loc,
                        tys: tys.to_vec(),
                        kind: (&builtin).into(),
                        args: arguments,
                    },
                },
            );
            cfg.add(vartab, Instr::Branch { block: end_if });

            cfg.set_basic_block(false_block);
            cfg.add(
                vartab,
                Instr::Set {
                    loc: *loc,
                    res: temp,
                    expr: zero,
                },
            );
            cfg.add(vartab, Instr::Branch { block: end_if });

            cfg.set_phis(end_if, vartab.pop_dirty_tracker());
            cfg.set_basic_block(end_if);
            Expression::Variable {
                loc: *loc,
                ty: tys[0].clone(),
                var_no: temp,
            }
        }
        ast::Builtin::GetAddress => {
            if let Some(constant_id) = &ns.contracts[contract_no].program_id {
                return Expression::NumberLiteral {
                    loc: *loc,
                    ty: Type::Address(false),
                    value: BigInt::from_bytes_be(Sign::Plus, constant_id),
                };
            }

            // In emit, GetAddress returns a pointer to the address
            let codegen_expr = Expression::Builtin {
                loc: *loc,
                tys: vec![Type::Ref(Box::new(Type::Address(false)))],
                kind: Builtin::GetAddress,
                args: vec![],
            };

            Expression::Load {
                loc: *loc,
                ty: Type::Address(false),
                expr: Box::new(codegen_expr),
            }
        }
        ast::Builtin::ECRecover => {
            // TODO:
            // EVM: call precompile 1 (code below is untested)
            // let args = args
            //     .iter()
            //     .map(|v| expression(v, cfg, contract_no, func, ns, vartab, opt))
            //     .collect::<Vec<Expression>>();
            //
            // let payload = abi_encode(loc, args, ns, vartab, cfg, false).0;
            //
            // let instr = Instr::ExternalCall {
            //     loc: *loc,
            //     contract_function_no: None,
            //     address: Some(Expression::NumberLiteral {
            //         loc: *loc,
            //         ty: Type::Address(false),
            //         value: BigInt::one(),
            //     }),
            //     accounts: None,
            //     seeds: None,
            //     payload,
            //     value: Expression::NumberLiteral {
            //         loc: *loc,
            //         ty: ns.value_type(),
            //         value: 0.into(),
            //     },
            //     success: None,
            //     gas: Expression::NumberLiteral {
            //         loc: *loc,
            //         ty: ns.value_type(),
            //         value: 0.into(),
            //     },
            //     callty: CallTy::Regular,
            //     flags: None,
            // };
            //
            // cfg.add(vartab, instr);
            //
            // let mut res = abi_decode(
            //     loc,
            //     &Expression::ReturnData { loc: *loc },
            //     &[Type::Address(false)],
            //     ns,
            //     vartab,
            //     cfg,
            //     None,
            // );
            //
            // res.remove(0)

            // Polkadot: call ecdsa_recover(): https://docs.rs/pallet-contracts/latest/pallet_contracts/api_doc/trait.Version0.html#tymethod.ecdsa_recover
            // Solana: see how neon implements this
            cfg.add(vartab, Instr::Unimplemented { reachable: true });

            Expression::NumberLiteral {
                loc: *loc,
                ty: Type::Bool,
                value: 0.into(),
            }
        }
        ast::Builtin::TypeName => {
            let ast::Expression::TypeOperator {
                ty: Type::Contract(no),
                ..
            } = &args[0]
            else {
                unreachable!();
            };

            let value = ns.contracts[*no].id.name.as_bytes().to_owned();

            Expression::BytesLiteral {
                loc: *loc,
                ty: Type::String,
                value,
            }
        }
        ast::Builtin::TypeInterfaceId => {
            let ast::Expression::TypeOperator {
                ty: Type::Contract(contract_no),
                ..
            } = &args[0]
            else {
                unreachable!();
            };

            interfaceid(ns, *contract_no, loc)
        }
        ast::Builtin::TypeRuntimeCode | ast::Builtin::TypeCreatorCode => {
            let ast::Expression::TypeOperator {
                ty: Type::Contract(contract_no),
                ..
            } = &args[0]
            else {
                unreachable!();
            };

            code(loc, *contract_no, ns, opt)
        }
        _ => {
            let arguments: Vec<Expression> = args
                .iter()
                .map(|v| expression(v, cfg, contract_no, func, ns, vartab, opt))
                .collect();

            if !arguments.is_empty() && builtin == ast::Builtin::ArrayLength {
                // If an array length instruction is called
                // Get the variable it is assigned with
                if let Expression::Variable { var_no, .. } = &arguments[0] {
                    // Now that we have its temp in the map, retrieve the temp var res from the map
                    if let Some(array_length_var) = cfg.array_lengths_temps.get(var_no) {
                        // If it's there, replace ArrayLength with the temp var
                        return Expression::Variable {
                            loc: *loc,
                            ty: Type::Uint(32),
                            var_no: *array_length_var,
                        };
                    }
                }
            }
            Expression::Builtin {
                loc: *loc,
                tys: tys.to_vec(),
                kind: (&builtin).into(),
                args: arguments,
            }
        }
    }
}

fn alloc_dynamic_array(
    size: &ast::Expression,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    loc: &pt::Loc,
    ty: &Type,
    init: &Option<Vec<u8>>,
    opt: &Options,
) -> Expression {
    let size = expression(size, cfg, contract_no, func, ns, vartab, opt);
    Expression::AllocDynamicBytes {
        loc: *loc,
        ty: ty.clone(),
        size: Box::new(size),
        initializer: init.clone(),
    }
}

fn add(
    loc: &pt::Loc,
    ty: &Type,
    overflowing: bool,
    left: &ast::Expression,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    right: &ast::Expression,
    opt: &Options,
) -> Expression {
    Expression::Add {
        loc: *loc,
        ty: ty.clone(),
        overflowing,
        left: Box::new(expression(left, cfg, contract_no, func, ns, vartab, opt)),
        right: Box::new(expression(right, cfg, contract_no, func, ns, vartab, opt)),
    }
}

fn subtract(
    loc: &pt::Loc,
    ty: &Type,
    overflowing: bool,
    left: &ast::Expression,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    right: &ast::Expression,
    opt: &Options,
) -> Expression {
    Expression::Subtract {
        loc: *loc,
        ty: ty.clone(),
        overflowing,
        left: Box::new(expression(left, cfg, contract_no, func, ns, vartab, opt)),
        right: Box::new(expression(right, cfg, contract_no, func, ns, vartab, opt)),
    }
}

fn checking_trunc(
    loc: &pt::Loc,
    expr: &ast::Expression,
    ty: &Type,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
) -> Expression {
    let bits = match ty {
        Type::Uint(bits) => *bits as u32,
        Type::Value => ns.value_length as u32 * 8,
        _ => unreachable!(),
    };

    let source_ty = expr.ty();

    let overflow = Expression::NumberLiteral {
        loc: *loc,
        ty: source_ty.clone(),
        value: BigInt::from(2u32).pow(bits),
    };

    let pos = vartab.temp(
        &pt::Identifier {
            name: "value".to_owned(),
            loc: *loc,
        },
        &source_ty,
    );

    let expr = expression(expr, cfg, contract_no, func, ns, vartab, opt);

    cfg.add(
        vartab,
        Instr::Set {
            loc: expr.loc(),
            res: pos,
            expr,
        },
    );

    let out_of_bounds = cfg.new_basic_block("out_of_bounds".to_string());
    let in_bounds = cfg.new_basic_block("in_bounds".to_string());

    cfg.add(
        vartab,
        Instr::BranchCond {
            cond: Expression::MoreEqual {
                loc: *loc,
                signed: false,
                left: Box::new(Expression::Variable {
                    loc: *loc,
                    ty: source_ty.clone(),
                    var_no: pos,
                }),
                right: Box::new(overflow),
            },
            true_block: out_of_bounds,
            false_block: in_bounds,
        },
    );

    cfg.set_basic_block(out_of_bounds);
    log_runtime_error(
        opt.log_runtime_errors,
        "truncated type overflows",
        *loc,
        cfg,
        vartab,
        ns,
    );
    let error = SolidityError::Panic(PanicCode::MathOverflow);
    assert_failure(loc, error, ns, cfg, vartab);

    cfg.set_basic_block(in_bounds);

    Expression::Trunc {
        loc: *loc,
        ty: ty.clone(),
        expr: Box::new(Expression::Variable {
            loc: *loc,
            ty: source_ty,
            var_no: pos,
        }),
    }
}

fn format_string(
    args: &[(FormatArg, ast::Expression)],
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    loc: &pt::Loc,
    opt: &Options,
) -> Expression {
    let args = args
        .iter()
        .map(|(spec, arg)| {
            (
                *spec,
                expression(arg, cfg, contract_no, func, ns, vartab, opt),
            )
        })
        .collect();
    Expression::FormatString { loc: *loc, args }
}

fn conditional_operator(
    loc: &pt::Loc,
    ty: &Type,
    cond: &ast::Expression,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    left: &ast::Expression,
    right: &ast::Expression,
    opt: &Options,
) -> Expression {
    let cond = expression(cond, cfg, contract_no, func, ns, vartab, opt);

    let pos = vartab.temp(
        &pt::Identifier {
            name: "ternary_result".to_owned(),
            loc: *loc,
        },
        ty,
    );

    vartab.new_dirty_tracker();

    let left_block = cfg.new_basic_block("left_value".to_string());
    let right_block = cfg.new_basic_block("right_value".to_string());
    let done_block = cfg.new_basic_block("conditional_done".to_string());

    cfg.add(
        vartab,
        Instr::BranchCond {
            cond,
            true_block: left_block,
            false_block: right_block,
        },
    );

    cfg.set_basic_block(left_block);

    let expr = expression(left, cfg, contract_no, func, ns, vartab, opt);

    cfg.add(
        vartab,
        Instr::Set {
            loc: expr.loc(),
            res: pos,
            expr,
        },
    );

    cfg.add(vartab, Instr::Branch { block: done_block });

    cfg.set_basic_block(right_block);

    let expr = expression(right, cfg, contract_no, func, ns, vartab, opt);

    cfg.add(
        vartab,
        Instr::Set {
            loc: expr.loc(),
            res: pos,
            expr,
        },
    );

    cfg.add(vartab, Instr::Branch { block: done_block });

    cfg.set_basic_block(done_block);

    cfg.set_phis(done_block, vartab.pop_dirty_tracker());

    Expression::Variable {
        loc: *loc,
        ty: ty.clone(),
        var_no: pos,
    }
}

fn interfaceid(ns: &Namespace, contract_no: usize, loc: &pt::Loc) -> Expression {
    let selector_len = ns.target.selector_length();
    let mut id = vec![0u8; selector_len as usize];
    for func_no in &ns.contracts[contract_no].functions {
        let func = &ns.functions[*func_no];

        if func.ty == pt::FunctionTy::Function {
            let selector = func.selector(ns, &contract_no);
            debug_assert_eq!(id.len(), selector.len());

            for (i, e) in id.iter_mut().enumerate() {
                *e ^= selector[i];
            }
        }
    }
    Expression::BytesLiteral {
        loc: *loc,
        ty: Type::Bytes(selector_len),
        value: id.clone(),
    }
}

pub fn assign_single(
    left: &ast::Expression,
    cfg_right: Expression,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
) -> Expression {
    match left {
        ast::Expression::Variable { loc, ty, var_no } => {
            cfg.add(
                vartab,
                Instr::Set {
                    loc: *loc,
                    res: *var_no,
                    expr: cfg_right,
                },
            );

            Expression::Variable {
                loc: *loc,
                ty: ty.clone(),
                var_no: *var_no,
            }
        }
        _ => {
            let left_ty = left.ty();
            let ty = cfg_right.ty();

            let pos = vartab.temp_anonymous(&ty);

            // Set a subscript in storage bytes needs special handling
            let set_storage_bytes = if let ast::Expression::Subscript { array_ty, .. } = &left {
                array_ty.is_storage_bytes()
            } else {
                false
            };

            let dest = expression(left, cfg, contract_no, func, ns, vartab, opt);

            let cfg_right =
                if !left_ty.is_contract_storage() && cfg_right.ty().is_fixed_reference_type(ns) {
                    Expression::Load {
                        loc: pt::Loc::Codegen,
                        ty: cfg_right.ty(),
                        expr: Box::new(cfg_right),
                    }
                } else {
                    cfg_right
                };

            cfg.add(
                vartab,
                Instr::Set {
                    loc: pt::Loc::Codegen,
                    res: pos,
                    expr: cfg_right,
                },
            );

            let storage_type = storage_type(left, ns);

            match left_ty {
                Type::StorageRef(..) if set_storage_bytes => {
                    if let Expression::Subscript {
                        expr: array, index, ..
                    } = dest
                    {
                        // Set a byte in a byte array
                        cfg.add(
                            vartab,
                            Instr::SetStorageBytes {
                                value: Expression::Variable {
                                    loc: left.loc(),
                                    ty: ty.clone(),
                                    var_no: pos,
                                },
                                storage: *array,
                                offset: *index,
                            },
                        );
                    } else {
                        unreachable!();
                    }
                }
                Type::StorageRef(..) => {
                    cfg.add(
                        vartab,
                        Instr::SetStorage {
                            value: Expression::Variable {
                                loc: left.loc(),
                                ty: ty.clone(),
                                var_no: pos,
                            },
                            ty: ty.deref_any().clone(),
                            storage: dest,
                            storage_type,
                        },
                    );
                }
                Type::Ref(_) => {
                    cfg.add(
                        vartab,
                        Instr::Store {
                            dest,
                            data: Expression::Variable {
                                loc: Loc::Codegen,
                                ty: ty.clone(),
                                var_no: pos,
                            },
                        },
                    );
                }
                _ => unreachable!(),
            }

            Expression::Variable {
                loc: left.loc(),
                ty,
                var_no: pos,
            }
        }
    }
}

/// Convert a function call expression to CFG in expression context
pub fn emit_function_call(
    expr: &ast::Expression,
    caller_contract_no: usize,
    cfg: &mut ControlFlowGraph,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
) -> Vec<Expression> {
    match expr {
        ast::Expression::InternalFunctionCall { function, args, .. } => {
            if let ast::Expression::InternalFunction {
                function_no,
                signature,
                ..
            } = function.as_ref()
            {
                let args = args
                    .iter()
                    .map(|a| expression(a, cfg, caller_contract_no, func, ns, vartab, opt))
                    .collect();

                let function_no = if let Some(signature) = signature {
                    ns.contracts[caller_contract_no].virtual_functions[signature]
                        .last()
                        .copied()
                        .unwrap()
                } else {
                    *function_no
                };

                let ftype = &ns.functions[function_no];

                let call = if ns.functions[function_no].loc_prototype == pt::Loc::Builtin {
                    InternalCallTy::Builtin {
                        ast_func_no: function_no,
                    }
                } else {
                    let cfg_no = ns.contracts[caller_contract_no].all_functions[&function_no];

                    InternalCallTy::Static { cfg_no }
                };

                if !ftype.returns.is_empty() {
                    let mut res = Vec::new();
                    let mut returns = Vec::new();
                    let mut return_tys = Vec::new();

                    for ret in &*ftype.returns {
                        let id = pt::Identifier {
                            loc: ret.loc,
                            name: ret.name_as_str().to_owned(),
                        };

                        let temp_pos = vartab.temp(&id, &ret.ty);
                        return_tys.push(ret.ty.clone());
                        res.push(temp_pos);
                        returns.push(Expression::Variable {
                            loc: id.loc,
                            ty: ret.ty.clone(),
                            var_no: temp_pos,
                        });
                    }

                    cfg.add(
                        vartab,
                        Instr::Call {
                            res,
                            call,
                            args,
                            return_tys,
                        },
                    );

                    returns
                } else {
                    cfg.add(
                        vartab,
                        Instr::Call {
                            res: Vec::new(),
                            return_tys: Vec::new(),
                            call,
                            args,
                        },
                    );

                    vec![Expression::Poison]
                }
            } else if let Type::InternalFunction { returns, .. } = function.ty().deref_any() {
                let cfg_expr = expression(function, cfg, caller_contract_no, func, ns, vartab, opt);

                let args = args
                    .iter()
                    .map(|a| expression(a, cfg, caller_contract_no, func, ns, vartab, opt))
                    .collect();

                if !returns.is_empty() {
                    let mut res = Vec::new();
                    let mut return_values = Vec::new();
                    let mut return_tys = Vec::new();

                    for ty in returns {
                        let id = pt::Identifier {
                            loc: pt::Loc::Codegen,
                            name: String::new(),
                        };

                        let temp_pos = vartab.temp(&id, ty);
                        res.push(temp_pos);
                        return_tys.push(ty.clone());
                        return_values.push(Expression::Variable {
                            loc: id.loc,
                            ty: ty.clone(),
                            var_no: temp_pos,
                        });
                    }

                    cfg.add(
                        vartab,
                        Instr::Call {
                            res,
                            call: InternalCallTy::Dynamic(cfg_expr),
                            return_tys,
                            args,
                        },
                    );

                    return_values
                } else {
                    cfg.add(
                        vartab,
                        Instr::Call {
                            res: Vec::new(),
                            return_tys: Vec::new(),
                            call: InternalCallTy::Dynamic(cfg_expr),
                            args,
                        },
                    );

                    vec![Expression::Poison]
                }
            } else {
                unreachable!();
            }
        }
        ast::Expression::ExternalFunctionCallRaw {
            loc,
            address,
            args,
            call_args,
            ty,
        } => {
            let args = expression(args, cfg, caller_contract_no, func, ns, vartab, opt);
            let address = expression(address, cfg, caller_contract_no, func, ns, vartab, opt);
            let gas = if let Some(gas) = &call_args.gas {
                expression(gas, cfg, caller_contract_no, func, ns, vartab, opt)
            } else {
                default_gas(ns)
            };
            let value = if let Some(value) = &call_args.value {
                expression(value, cfg, caller_contract_no, func, ns, vartab, opt)
            } else {
                Expression::NumberLiteral {
                    loc: pt::Loc::Codegen,
                    ty: Type::Value,
                    value: BigInt::zero(),
                }
            };
            let accounts = call_args
                .accounts
                .map(|expr| expression(expr, cfg, caller_contract_no, func, ns, vartab, opt));
            let seeds = call_args
                .seeds
                .as_ref()
                .map(|expr| expression(expr, cfg, caller_contract_no, func, ns, vartab, opt));

            let success = vartab.temp_name("success", &Type::Uint(32));

            let flags = call_args
                .flags
                .as_ref()
                .map(|expr| expression(expr, cfg, caller_contract_no, func, ns, vartab, opt));

            cfg.add(
                vartab,
                Instr::ExternalCall {
                    loc: *loc,
                    success: Some(success),
                    address: Some(address),
                    payload: args,
                    value,
                    accounts,
                    seeds,
                    gas,
                    callty: ty.clone(),
                    contract_function_no: None,
                    flags,
                },
            );

            let success = if ns.target.is_polkadot() {
                let ret_code = Expression::Variable {
                    loc: *loc,
                    ty: Type::Uint(32),
                    var_no: success,
                };
                let ret_ok = Expression::NumberLiteral {
                    loc: *loc,
                    ty: Type::Uint(32),
                    value: 0.into(),
                };
                Expression::Equal {
                    loc: *loc,
                    left: ret_code.into(),
                    right: ret_ok.into(),
                }
            } else {
                Expression::Variable {
                    loc: *loc,
                    ty: Type::Uint(32),
                    var_no: success,
                }
            };
            vec![success, Expression::ReturnData { loc: *loc }]
        }
        ast::Expression::ExternalFunctionCall {
            loc,
            function,
            args,
            returns,
            call_args,
            ..
        } => {
            if let ast::Expression::ExternalFunction {
                function_no,
                address,
                ..
            } = function.as_ref()
            {
                let dest_func = &ns.functions[*function_no];
                let contract_function_no = dest_func
                    .contract_no
                    .map(|contract_no| (contract_no, *function_no));

                let mut tys: Vec<Type> = args.iter().map(|a| a.ty()).collect();
                let mut args: Vec<Expression> = args
                    .iter()
                    .map(|a| expression(a, cfg, caller_contract_no, func, ns, vartab, opt))
                    .collect();
                let address = expression(address, cfg, caller_contract_no, func, ns, vartab, opt);
                let gas = if let Some(gas) = &call_args.gas {
                    expression(gas, cfg, caller_contract_no, func, ns, vartab, opt)
                } else {
                    default_gas(ns)
                };
                let accounts = call_args
                    .accounts
                    .map(|expr| expression(expr, cfg, caller_contract_no, func, ns, vartab, opt));
                let seeds = call_args
                    .seeds
                    .as_ref()
                    .map(|expr| expression(expr, cfg, caller_contract_no, func, ns, vartab, opt));

                let value = if let Some(value) = &call_args.value {
                    expression(value, cfg, caller_contract_no, func, ns, vartab, opt)
                } else {
                    Expression::NumberLiteral {
                        loc: pt::Loc::Codegen,
                        ty: Type::Value,
                        value: BigInt::zero(),
                    }
                };

                let selector = dest_func.selector(ns, &caller_contract_no);

                tys.insert(0, Type::Bytes(selector.len() as u8));

                args.insert(
                    0,
                    Expression::BytesLiteral {
                        loc: *loc,
                        ty: Type::Bytes(selector.len() as u8),
                        value: selector,
                    },
                );

                let (payload, _) = abi_encode(loc, args, ns, vartab, cfg, false);

                let flags = call_args
                    .flags
                    .as_ref()
                    .map(|expr| expression(expr, cfg, caller_contract_no, func, ns, vartab, opt));

                let success = ns
                    .target
                    .is_polkadot()
                    .then(|| vartab.temp_name("success", &Type::Uint(32)));
                cfg.add(
                    vartab,
                    Instr::ExternalCall {
                        loc: *loc,
                        success,
                        accounts,
                        address: Some(address),
                        payload,
                        seeds,
                        value,
                        gas,
                        callty: CallTy::Regular,
                        contract_function_no,
                        flags,
                    },
                );

                if ns.target.is_polkadot() {
                    polkadot::RetCodeCheckBuilder::default()
                        .loc(*loc)
                        .msg("external call failed")
                        .success_var(success.unwrap())
                        .insert(cfg, vartab)
                        .handle_cases(cfg, ns, opt, vartab);
                }

                // If the first element of returns is Void, we can discard the returns
                if !dest_func.returns.is_empty() && returns[0] != Type::Void {
                    let tys = dest_func
                        .returns
                        .iter()
                        .map(|e| e.ty.clone())
                        .collect::<Vec<Type>>();
                    abi_decode(
                        loc,
                        &Expression::ReturnData { loc: *loc },
                        &tys,
                        ns,
                        vartab,
                        cfg,
                        None,
                    )
                } else {
                    vec![Expression::Poison]
                }
            } else if let Type::ExternalFunction {
                returns: func_returns,
                ..
            } = function.ty()
            {
                let mut tys: Vec<Type> = args.iter().map(|a| a.ty()).collect();
                let mut args = args
                    .iter()
                    .map(|a| expression(a, cfg, caller_contract_no, func, ns, vartab, opt))
                    .collect::<Vec<Expression>>();
                let function = expression(function, cfg, caller_contract_no, func, ns, vartab, opt);
                let gas = if let Some(gas) = &call_args.gas {
                    expression(gas, cfg, caller_contract_no, func, ns, vartab, opt)
                } else {
                    default_gas(ns)
                };
                let value = if let Some(value) = &call_args.value {
                    expression(value, cfg, caller_contract_no, func, ns, vartab, opt)
                } else {
                    Expression::NumberLiteral {
                        loc: pt::Loc::Codegen,
                        ty: Type::Value,
                        value: BigInt::zero(),
                    }
                };

                let selector = function.external_function_selector();
                let address = function.external_function_address();

                tys.insert(0, Type::Bytes(ns.target.selector_length()));
                args.insert(0, selector);

                let (payload, _) = abi_encode(loc, args, ns, vartab, cfg, false);

                let flags = call_args
                    .flags
                    .as_ref()
                    .map(|expr| expression(expr, cfg, caller_contract_no, func, ns, vartab, opt));
                let success = ns
                    .target
                    .is_polkadot()
                    .then(|| vartab.temp_name("success", &Type::Uint(32)));
                cfg.add(
                    vartab,
                    Instr::ExternalCall {
                        loc: *loc,
                        success,
                        accounts: ExternalCallAccounts::AbsentArgument,
                        seeds: None,
                        address: Some(address),
                        payload,
                        value,
                        gas,
                        callty: CallTy::Regular,
                        contract_function_no: None,
                        flags,
                    },
                );

                if ns.target.is_polkadot() {
                    polkadot::RetCodeCheckBuilder::default()
                        .loc(*loc)
                        .msg("external call failed")
                        .success_var(success.unwrap())
                        .insert(cfg, vartab)
                        .handle_cases(cfg, ns, opt, vartab);
                }

                if !func_returns.is_empty() && returns[0] != Type::Void {
                    abi_decode(
                        loc,
                        &Expression::ReturnData { loc: *loc },
                        returns,
                        ns,
                        vartab,
                        cfg,
                        None,
                    )
                } else {
                    vec![Expression::Poison]
                }
            } else {
                unreachable!();
            }
        }
        ast::Expression::Builtin {
            loc,
            tys,
            kind: ast::Builtin::AbiDecode,
            args,
        } => {
            let data = expression(&args[0], cfg, caller_contract_no, func, ns, vartab, opt);
            abi_decode(loc, &data, tys, ns, vartab, cfg, None)
        }
        _ => unreachable!(),
    }
}

pub fn default_gas(ns: &Namespace) -> Expression {
    Expression::NumberLiteral {
        loc: pt::Loc::Codegen,
        ty: Type::Uint(64),
        // See EIP150
        value: if ns.target == Target::EVM {
            BigInt::from(i64::MAX)
        } else {
            BigInt::zero()
        },
    }
}

/// Codegen for an array subscript expression
fn array_subscript(
    loc: &pt::Loc,
    elem_ty: &Type,
    array_ty: &Type,
    array: &ast::Expression,
    index: &ast::Expression,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
) -> Expression {
    if array_ty.is_storage_bytes() {
        return Expression::Subscript {
            loc: *loc,
            ty: elem_ty.clone(),
            array_ty: array_ty.clone(),
            expr: Box::new(expression(array, cfg, contract_no, func, ns, vartab, opt)),
            index: Box::new(expression(index, cfg, contract_no, func, ns, vartab, opt)),
        };
    }

    if array_ty.is_mapping() {
        let array = expression(array, cfg, contract_no, func, ns, vartab, opt);
        let index = expression(index, cfg, contract_no, func, ns, vartab, opt);

        return if ns.target == Target::Solana {
            Expression::Subscript {
                loc: *loc,
                ty: elem_ty.clone(),
                array_ty: array_ty.clone(),
                expr: Box::new(array),
                index: Box::new(index),
            }
        } else {
            Expression::Keccak256 {
                loc: *loc,
                ty: array_ty.clone(),
                exprs: vec![array, index],
            }
        };
    }

    let mut array = expression(array, cfg, contract_no, func, ns, vartab, opt);
    let index_ty = index.ty();
    let index = expression(index, cfg, contract_no, func, ns, vartab, opt);
    let index_loc = index.loc();

    let index_width = index_ty.bits(ns);

    let array_length = match array_ty.deref_any() {
        Type::Bytes(n) => {
            let ast_bigint = bigint_to_expression(
                &array.loc(),
                &BigInt::from(*n),
                ns,
                &mut Diagnostics::default(),
                ResolveTo::Unknown,
                None,
            )
            .unwrap();
            expression(&ast_bigint, cfg, contract_no, func, ns, vartab, opt)
        }
        Type::Array(..) => match array_ty.array_length() {
            None => {
                if let Type::StorageRef(..) = array_ty {
                    if ns.target == Target::Solana {
                        Expression::StorageArrayLength {
                            loc: *loc,
                            ty: ns.storage_type(),
                            array: Box::new(array.clone()),
                            elem_ty: array_ty.storage_array_elem().deref_into(),
                        }
                    } else {
                        // TODO(Soroban): Storage type here is None, since arrays are not yet supported in Soroban
                        let array_length =
                            load_storage(loc, &Type::Uint(256), array.clone(), cfg, vartab, None);

                        array = Expression::Keccak256 {
                            loc: *loc,
                            ty: Type::Uint(256),
                            exprs: vec![array],
                        };

                        array_length
                    }
                } else {
                    // If a subscript is encountered array length will be called

                    // Return array length by default
                    let mut returned = Expression::Builtin {
                        loc: *loc,
                        tys: vec![Type::Uint(32)],
                        kind: Builtin::ArrayLength,
                        args: vec![array.clone()],
                    };

                    if let Expression::Variable {
                        loc, var_no: num, ..
                    } = &array
                    {
                        // If the size is known (is in cfg.array_length_map), do the replacement

                        if let Some(array_length_var) = cfg.array_lengths_temps.get(num) {
                            returned = Expression::Variable {
                                loc: *loc,
                                ty: Type::Uint(32),
                                var_no: *array_length_var,
                            };
                        }
                    }
                    returned
                }
            }
            Some(l) => {
                let ast_big_int = bigint_to_expression(
                    loc,
                    l,
                    ns,
                    &mut Diagnostics::default(),
                    ResolveTo::Unknown,
                    None,
                )
                .unwrap();
                expression(&ast_big_int, cfg, contract_no, func, ns, vartab, opt)
            }
        },
        Type::DynamicBytes | Type::Slice(_) => Expression::Builtin {
            loc: *loc,
            tys: vec![Type::Uint(32)],
            kind: Builtin::ArrayLength,
            args: vec![array.clone()],
        },
        _ => {
            unreachable!();
        }
    };

    let array_width = array_length.ty().bits(ns);
    let width = std::cmp::max(array_width, index.ty().bits(ns));
    let coerced_ty = Type::Uint(width);

    let pos = vartab.temp(
        &pt::Identifier {
            name: "index".to_owned(),
            loc: *loc,
        },
        &coerced_ty,
    );

    let expr = index.cast(&coerced_ty, ns);
    cfg.add(
        vartab,
        Instr::Set {
            loc: expr.loc(),
            res: pos,
            expr,
        },
    );

    // If the array is fixed length and the index also constant, the
    // branch will be optimized away.
    let out_of_bounds = cfg.new_basic_block("out_of_bounds".to_string());
    let in_bounds = cfg.new_basic_block("in_bounds".to_string());

    cfg.add(
        vartab,
        Instr::BranchCond {
            cond: Expression::MoreEqual {
                loc: *loc,
                signed: false,
                left: Box::new(Expression::Variable {
                    loc: index_loc,
                    ty: coerced_ty.clone(),
                    var_no: pos,
                }),
                right: Box::new(array_length.cast(&coerced_ty, ns)),
            },
            true_block: out_of_bounds,
            false_block: in_bounds,
        },
    );

    cfg.set_basic_block(out_of_bounds);
    log_runtime_error(
        opt.log_runtime_errors,
        "array index out of bounds",
        *loc,
        cfg,
        vartab,
        ns,
    );
    let error = SolidityError::Panic(PanicCode::ArrayIndexOob);
    assert_failure(loc, error, ns, cfg, vartab);

    cfg.set_basic_block(in_bounds);

    if let Type::Bytes(array_length) = array_ty.deref_any() {
        let res_ty = Type::Bytes(1);
        let from_ty = Type::Bytes(*array_length);
        let index_ty = Type::Uint(*array_length as u16 * 8);

        let to_width = array_ty.bits(ns);
        let shift_arg_raw = Expression::Variable {
            loc: index_loc,
            ty: coerced_ty.clone(),
            var_no: pos,
        };

        let shift_arg = match index_width.cmp(&to_width) {
            Ordering::Equal => shift_arg_raw,
            Ordering::Less => Expression::ZeroExt {
                loc: *loc,
                ty: index_ty.clone(),
                expr: shift_arg_raw.into(),
            },
            Ordering::Greater => Expression::Trunc {
                loc: *loc,
                ty: index_ty.clone(),
                expr: shift_arg_raw.into(),
            },
        };

        return Expression::Trunc {
            loc: *loc,
            ty: res_ty,
            expr: Expression::ShiftRight {
                loc: *loc,
                ty: from_ty,
                left: array.into(),
                right: Expression::ShiftLeft {
                    loc: *loc,
                    ty: index_ty.clone(),
                    left: Box::new(Expression::Subtract {
                        loc: *loc,
                        ty: index_ty.clone(),
                        overflowing: true,
                        left: Expression::NumberLiteral {
                            loc: *loc,
                            ty: index_ty.clone(),
                            value: BigInt::from_u8(array_length - 1).unwrap(),
                        }
                        .into(),
                        right: shift_arg.into(),
                    }),
                    right: Expression::NumberLiteral {
                        loc: *loc,
                        ty: index_ty,
                        value: BigInt::from_u8(3).unwrap(),
                    }
                    .into(),
                }
                .into(),
                signed: false,
            }
            .into(),
        };
    }

    if let Type::StorageRef(_, ty) = &array_ty {
        let elem_ty = ty.storage_array_elem();
        let slot_ty = ns.storage_type();

        if ns.target == Target::Solana {
            if ty.array_length().is_some() && ty.is_sparse_solana(ns) {
                let index = Expression::Variable {
                    loc: index_loc,
                    ty: coerced_ty,
                    var_no: pos,
                }
                .cast(&Type::Uint(256), ns);

                Expression::Subscript {
                    loc: *loc,
                    ty: elem_ty,
                    array_ty: array_ty.clone(),
                    expr: Box::new(array),
                    index: Box::new(index),
                }
            } else {
                let index = Expression::Variable {
                    loc: index_loc,
                    ty: coerced_ty,
                    var_no: pos,
                }
                .cast(&slot_ty, ns);

                if ty.array_length().is_some() {
                    // fixed length array
                    let elem_size = elem_ty.deref_any().solana_storage_size(ns);

                    Expression::Add {
                        loc: *loc,
                        ty: elem_ty,
                        overflowing: true,
                        left: Box::new(array),
                        right: Box::new(Expression::Multiply {
                            loc: *loc,
                            ty: slot_ty.clone(),
                            overflowing: true,
                            left: Box::new(index),
                            right: Box::new(Expression::NumberLiteral {
                                loc: *loc,
                                ty: slot_ty,
                                value: elem_size,
                            }),
                        }),
                    }
                } else {
                    Expression::Subscript {
                        loc: *loc,
                        ty: elem_ty,
                        array_ty: array_ty.clone(),
                        expr: Box::new(array),
                        index: Box::new(index),
                    }
                }
            }
        } else {
            let elem_size = elem_ty.storage_slots(ns);

            if let Expression::NumberLiteral {
                value: arr_length, ..
            } = &array_length
            {
                if arr_length.mul(elem_size.clone()).to_u64().is_some() {
                    // we need to calculate the storage offset. If this can be done with 64 bit
                    // arithmetic it will be much more efficient on wasm
                    return Expression::Add {
                        loc: *loc,
                        ty: elem_ty,
                        overflowing: true,
                        left: Box::new(array),
                        right: Box::new(Expression::ZeroExt {
                            loc: *loc,
                            ty: slot_ty,
                            expr: Box::new(Expression::Multiply {
                                loc: *loc,
                                ty: Type::Uint(64),
                                overflowing: true,
                                left: Box::new(
                                    Expression::Variable {
                                        loc: index_loc,
                                        ty: coerced_ty,
                                        var_no: pos,
                                    }
                                    .cast(&Type::Uint(64), ns),
                                ),
                                right: Box::new(Expression::NumberLiteral {
                                    loc: *loc,
                                    ty: Type::Uint(64),
                                    value: elem_size,
                                }),
                            }),
                        }),
                    };
                }
            }

            array_offset(
                loc,
                array,
                Expression::Variable {
                    loc: index_loc,
                    ty: coerced_ty,
                    var_no: pos,
                }
                .cast(&ns.storage_type(), ns),
                elem_ty,
                ns,
            )
        }
    } else {
        match array_ty.deref_memory() {
            Type::DynamicBytes | Type::Array(..) | Type::Slice(_) => Expression::Subscript {
                loc: *loc,
                ty: elem_ty.clone(),
                array_ty: array_ty.clone(),
                expr: Box::new(array),
                index: Box::new(Expression::Variable {
                    loc: index_loc,
                    ty: coerced_ty,
                    var_no: pos,
                }),
            },
            _ => {
                // should not happen as type-checking already done
                unreachable!();
            }
        }
    }
}

fn string_location(
    loc: &StringLocation<ast::Expression>,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
) -> StringLocation<Expression> {
    match loc {
        StringLocation::RunTime(s) => StringLocation::RunTime(Box::new(expression(
            s,
            cfg,
            contract_no,
            func,
            ns,
            vartab,
            opt,
        ))),
        StringLocation::CompileTime(vec) => StringLocation::CompileTime(vec.clone()),
    }
}

// Generate a load from storage instruction
pub fn load_storage(
    loc: &pt::Loc,
    ty: &Type,
    storage: Expression,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    storage_type: Option<pt::StorageType>,
) -> Expression {
    let res = vartab.temp_anonymous(ty);

    cfg.add(
        vartab,
        Instr::LoadStorage {
            res,
            ty: ty.clone(),
            storage,
            storage_type,
        },
    );

    Expression::Variable {
        loc: *loc,
        ty: ty.clone(),
        var_no: res,
    }
}

fn array_literal_to_memory_array(
    loc: &pt::Loc,
    expr: &Expression,
    ty: &Type,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
) -> Expression {
    let memory_array = vartab.temp_anonymous(ty);
    let elem_ty = ty.array_elem();
    let dims = expr.ty().array_length().unwrap().clone();
    let array_size = Expression::NumberLiteral {
        loc: *loc,
        ty: Type::Uint(32),
        value: dims,
    };

    cfg.add(
        vartab,
        Instr::Set {
            loc: *loc,
            res: memory_array,
            expr: Expression::AllocDynamicBytes {
                loc: *loc,
                ty: ty.clone(),
                size: Box::new(array_size.clone()),
                initializer: None,
            },
        },
    );

    let elements = if let Expression::ArrayLiteral { values: items, .. } = expr {
        items
    } else {
        unreachable!()
    };

    for (item_no, item) in elements.iter().enumerate() {
        cfg.add(
            vartab,
            Instr::Store {
                dest: Expression::Subscript {
                    loc: *loc,
                    ty: Type::Ref(Box::new(elem_ty.clone())),
                    array_ty: ty.clone(),
                    expr: Box::new(Expression::Variable {
                        loc: *loc,
                        ty: ty.clone(),
                        var_no: memory_array,
                    }),
                    index: Box::new(Expression::NumberLiteral {
                        loc: *loc,
                        ty: Type::Uint(32),
                        value: BigInt::from(item_no),
                    }),
                },
                data: item.clone(),
            },
        );
    }

    let temp_res = vartab.temp_name("array_length", &Type::Uint(32));
    cfg.add(
        vartab,
        Instr::Set {
            loc: Loc::Codegen,
            res: temp_res,
            expr: array_size,
        },
    );

    cfg.array_lengths_temps.insert(memory_array, temp_res);

    Expression::Variable {
        loc: *loc,
        ty: ty.clone(),
        var_no: memory_array,
    }
}

/// Generate the binary code for a contract
#[cfg(feature = "llvm")]
fn code(loc: &Loc, contract_no: usize, ns: &Namespace, opt: &Options) -> Expression {
    let contract = &ns.contracts[contract_no];

    let code = contract.emit(ns, opt, contract_no);

    let size = Expression::NumberLiteral {
        loc: *loc,
        ty: Type::Uint(32),
        value: code.len().into(),
    };

    Expression::AllocDynamicBytes {
        loc: *loc,
        ty: Type::DynamicBytes,
        size: size.into(),
        initializer: Some(code),
    }
}

#[cfg(not(feature = "llvm"))]
fn code(loc: &Loc, _contract_no: usize, _ns: &Namespace, _opt: &Options) -> Expression {
    let code = b"code placeholder".to_vec();

    let size = Expression::NumberLiteral {
        loc: *loc,
        ty: Type::Uint(32),
        value: code.len().into(),
    };

    Expression::AllocDynamicBytes {
        loc: *loc,
        ty: Type::DynamicBytes,
        size: size.into(),
        initializer: Some(code),
    }
}

fn add_prefix_and_delimiter_to_print(mut expr: Expression) -> Expression {
    let prefix = b"print: ";
    let delimiter = b",\n";

    if let Expression::FormatString { loc, args } = &mut expr {
        let mut new_vec = Vec::new();
        new_vec.push((
            FormatArg::StringLiteral,
            Expression::BytesLiteral {
                loc: Loc::Codegen,
                ty: Type::Bytes(prefix.len() as u8),
                value: prefix.to_vec(),
            },
        ));
        new_vec.append(args);
        new_vec.push((
            FormatArg::StringLiteral,
            Expression::BytesLiteral {
                loc: Loc::Codegen,
                ty: Type::Bytes(delimiter.len() as u8),
                value: delimiter.to_vec(),
            },
        ));

        Expression::FormatString {
            loc: *loc,
            args: new_vec,
        }
    } else {
        Expression::FormatString {
            loc: Loc::Codegen,
            args: vec![
                (
                    FormatArg::StringLiteral,
                    Expression::BytesLiteral {
                        loc: Loc::Codegen,
                        ty: Type::Bytes(prefix.len() as u8),
                        value: prefix.to_vec(),
                    },
                ),
                (FormatArg::Default, expr),
                (
                    FormatArg::StringLiteral,
                    Expression::BytesLiteral {
                        loc: Loc::Codegen,
                        ty: Type::Bytes(delimiter.len() as u8),
                        value: delimiter.to_vec(),
                    },
                ),
            ],
        }
    }
}

fn storage_type(expr: &ast::Expression, ns: &Namespace) -> Option<pt::StorageType> {
    match expr {
        ast::Expression::StorageVariable {
            loc: _,
            ty: _,
            var_no,
            contract_no,
        } => {
            let var = ns.contracts[*contract_no].variables.get(*var_no).unwrap();

            var.storage_type.clone()
        }
        _ => None,
    }
}
