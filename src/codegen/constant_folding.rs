// SPDX-License-Identifier: Apache-2.0

use super::cfg::{ControlFlowGraph, Instr};
use super::reaching_definitions;
use crate::codegen::{Builtin, Expression};
use crate::sema::{
    ast::{Diagnostic, Namespace, RetrieveType, StringLocation, Type},
    eval::overflow_diagnostic,
};
use num_bigint::{BigInt, Sign};
use num_traits::{ToPrimitive, Zero};
use ripemd::Ripemd160;
use sha2::{Digest, Sha256};
use solang_parser::pt;
use solang_parser::pt::Loc;
use std::ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Rem, Shl, Shr, Sub};
use tiny_keccak::{Hasher, Keccak};

/// Constant folding pass on the given cfg. During constant folding, we may find issues
/// like divide by zero, so this function may add diagnostics to the namespace. If dry_run
/// is true, then diagnostics are generated but the CFG is not modified. This
///
pub fn constant_folding(cfg: &mut ControlFlowGraph, dry_run: bool, ns: &mut Namespace) {
    // for each block, instruction
    for block_no in 0..cfg.blocks.len() {
        let mut vars = cfg.blocks[block_no].defs.clone();

        for instr_no in 0..cfg.blocks[block_no].instr.len() {
            match &cfg.blocks[block_no].instr[instr_no] {
                Instr::Set { loc, res, expr, .. } => {
                    let (expr, expr_constant) = expression(expr, Some(&vars), cfg, ns);

                    if expr_constant {
                        ns.var_constants.insert(*loc, expr.clone());
                    }

                    if !dry_run {
                        cfg.blocks[block_no].instr[instr_no] = Instr::Set {
                            loc: *loc,
                            res: *res,
                            expr,
                        };
                    }
                }
                Instr::Call {
                    res,
                    call,
                    args,
                    return_tys,
                } => {
                    let args = args
                        .iter()
                        .map(|e| expression(e, Some(&vars), cfg, ns).0)
                        .collect();

                    if !dry_run {
                        cfg.blocks[block_no].instr[instr_no] = Instr::Call {
                            res: res.clone(),
                            call: call.clone(),
                            args,
                            return_tys: return_tys.clone(),
                        };
                    }
                }
                Instr::Return { value } => {
                    let value = value
                        .iter()
                        .map(|e| expression(e, Some(&vars), cfg, ns).0)
                        .collect();

                    if !dry_run {
                        cfg.blocks[block_no].instr[instr_no] = Instr::Return { value };
                    }
                }
                Instr::BranchCond {
                    cond,
                    true_block,
                    false_block,
                } => {
                    let (cond, _) = expression(cond, Some(&vars), cfg, ns);

                    if !dry_run {
                        if let Expression::BoolLiteral { value: cond, .. } = cond {
                            cfg.blocks[block_no].instr[instr_no] = Instr::Branch {
                                block: if cond { *true_block } else { *false_block },
                            };
                        } else {
                            cfg.blocks[block_no].instr[instr_no] = Instr::BranchCond {
                                cond,
                                true_block: *true_block,
                                false_block: *false_block,
                            };
                        }
                    }
                }
                Instr::Store { dest, data } => {
                    let (dest, _) = expression(dest, Some(&vars), cfg, ns);
                    let (data, _) = expression(data, Some(&vars), cfg, ns);

                    if !dry_run {
                        cfg.blocks[block_no].instr[instr_no] = Instr::Store { dest, data };
                    }
                }
                Instr::AssertFailure {
                    encoded_args: Some(expr),
                } => {
                    let (buf, _) = expression(expr, Some(&vars), cfg, ns);

                    if !dry_run {
                        cfg.blocks[block_no].instr[instr_no] = Instr::AssertFailure {
                            encoded_args: Some(buf),
                        };
                    }
                }
                Instr::Print { expr } => {
                    let (expr, _) = expression(expr, Some(&vars), cfg, ns);

                    if !dry_run {
                        cfg.blocks[block_no].instr[instr_no] = Instr::Print { expr };
                    }
                }
                Instr::ClearStorage { ty, storage } => {
                    let (storage, _) = expression(storage, Some(&vars), cfg, ns);

                    if !dry_run {
                        cfg.blocks[block_no].instr[instr_no] = Instr::ClearStorage {
                            ty: ty.clone(),
                            storage,
                        };
                    }
                }
                Instr::SetStorage {
                    ty,
                    storage,
                    value,
                    storage_type,
                } => {
                    let (storage, _) = expression(storage, Some(&vars), cfg, ns);
                    let (value, _) = expression(value, Some(&vars), cfg, ns);

                    if !dry_run {
                        cfg.blocks[block_no].instr[instr_no] = Instr::SetStorage {
                            ty: ty.clone(),
                            storage,
                            value,
                            storage_type: storage_type.clone(),
                        };
                    }
                }
                Instr::LoadStorage {
                    ty,
                    storage,
                    res,
                    storage_type,
                } => {
                    let (storage, _) = expression(storage, Some(&vars), cfg, ns);

                    if !dry_run {
                        cfg.blocks[block_no].instr[instr_no] = Instr::LoadStorage {
                            ty: ty.clone(),
                            storage,
                            res: *res,
                            storage_type: storage_type.clone(),
                        };
                    }
                }
                Instr::SetStorageBytes {
                    storage,
                    value,
                    offset,
                } => {
                    let (storage, _) = expression(storage, Some(&vars), cfg, ns);
                    let (value, _) = expression(value, Some(&vars), cfg, ns);
                    let (offset, _) = expression(offset, Some(&vars), cfg, ns);

                    if !dry_run {
                        cfg.blocks[block_no].instr[instr_no] = Instr::SetStorageBytes {
                            storage,
                            value,
                            offset,
                        };
                    }
                }
                Instr::PushStorage {
                    res,
                    ty,
                    storage,
                    value,
                } => {
                    let (storage, _) = expression(storage, Some(&vars), cfg, ns);
                    let value = value
                        .as_ref()
                        .map(|expr| expression(expr, Some(&vars), cfg, ns).0);

                    if !dry_run {
                        cfg.blocks[block_no].instr[instr_no] = Instr::PushStorage {
                            res: *res,
                            ty: ty.clone(),
                            storage,
                            value,
                        };
                    }
                }
                Instr::PopStorage { res, ty, storage } => {
                    let (storage, _) = expression(storage, Some(&vars), cfg, ns);

                    if !dry_run {
                        cfg.blocks[block_no].instr[instr_no] = Instr::PopStorage {
                            res: *res,
                            ty: ty.clone(),
                            storage,
                        };
                    }
                }
                Instr::PushMemory {
                    res,
                    ty,
                    array,
                    value,
                } => {
                    let (value, _) = expression(value, Some(&vars), cfg, ns);

                    if !dry_run {
                        cfg.blocks[block_no].instr[instr_no] = Instr::PushMemory {
                            res: *res,
                            ty: ty.clone(),
                            array: *array,
                            value: Box::new(value),
                        };
                    }
                }
                Instr::Constructor {
                    success,
                    res,
                    contract_no,
                    encoded_args,
                    value,
                    gas,
                    salt,
                    address,
                    seeds,
                    loc,
                    accounts,
                    constructor_no,
                } => {
                    let encoded_args = expression(encoded_args, Some(&vars), cfg, ns).0;
                    let value = value
                        .as_ref()
                        .map(|expr| expression(expr, Some(&vars), cfg, ns).0);
                    let gas = expression(gas, Some(&vars), cfg, ns).0;
                    let salt = salt
                        .as_ref()
                        .map(|expr| expression(expr, Some(&vars), cfg, ns).0);
                    let address = address
                        .as_ref()
                        .map(|expr| expression(expr, Some(&vars), cfg, ns).0);
                    let seeds = seeds
                        .as_ref()
                        .map(|expr| expression(expr, Some(&vars), cfg, ns).0);
                    let accounts = accounts.map(|expr| expression(expr, Some(&vars), cfg, ns).0);

                    if !dry_run {
                        cfg.blocks[block_no].instr[instr_no] = Instr::Constructor {
                            success: *success,
                            res: *res,
                            contract_no: *contract_no,
                            constructor_no: *constructor_no,
                            encoded_args,
                            value,
                            gas,
                            salt,
                            address,
                            seeds,
                            loc: *loc,
                            accounts,
                        };
                    }
                }
                Instr::ExternalCall {
                    loc,
                    success,
                    address,
                    payload,
                    value,
                    gas,
                    accounts,
                    callty,
                    seeds,
                    contract_function_no,
                    flags,
                } => {
                    let value = expression(value, Some(&vars), cfg, ns).0;
                    let gas = expression(gas, Some(&vars), cfg, ns).0;
                    let payload = expression(payload, Some(&vars), cfg, ns).0;
                    let address = address
                        .as_ref()
                        .map(|expr| expression(expr, Some(&vars), cfg, ns).0);
                    let accounts = accounts.map(|expr| expression(expr, Some(&vars), cfg, ns).0);
                    let seeds = seeds
                        .as_ref()
                        .map(|expr| expression(expr, Some(&vars), cfg, ns).0);
                    let flags = flags
                        .as_ref()
                        .map(|expr| expression(expr, Some(&vars), cfg, ns).0);

                    if !dry_run {
                        cfg.blocks[block_no].instr[instr_no] = Instr::ExternalCall {
                            loc: *loc,
                            success: *success,
                            address,
                            accounts,
                            seeds,
                            payload,
                            value,
                            gas,
                            callty: callty.clone(),
                            contract_function_no: *contract_function_no,
                            flags,
                        };
                    }
                }
                Instr::SelfDestruct { recipient } => {
                    let (recipient, _) = expression(recipient, Some(&vars), cfg, ns);

                    if !dry_run {
                        cfg.blocks[block_no].instr[instr_no] = Instr::SelfDestruct { recipient };
                    }
                }
                Instr::EmitEvent {
                    event_no,
                    data,
                    topics,
                } => {
                    let topics = topics
                        .iter()
                        .map(|e| expression(e, Some(&vars), cfg, ns).0)
                        .collect();

                    if !dry_run {
                        cfg.blocks[block_no].instr[instr_no] = Instr::EmitEvent {
                            event_no: *event_no,
                            data: expression(data, Some(&vars), cfg, ns).0,
                            topics,
                        };
                    }
                }
                Instr::MemCopy {
                    source,
                    destination,
                    bytes,
                } => {
                    let bytes = expression(bytes, Some(&vars), cfg, ns);
                    let source = expression(source, Some(&vars), cfg, ns);
                    let destination = expression(destination, Some(&vars), cfg, ns);

                    if !dry_run {
                        cfg.blocks[block_no].instr[instr_no] = Instr::MemCopy {
                            source: source.0,
                            destination: destination.0,
                            bytes: bytes.0,
                        };
                    }
                }
                Instr::Switch {
                    cond,
                    cases,
                    default,
                } => {
                    let cond = expression(cond, Some(&vars), cfg, ns);
                    let cases = cases
                        .iter()
                        .map(|(exp, goto)| (expression(exp, Some(&vars), cfg, ns).0, *goto))
                        .collect::<Vec<(Expression, usize)>>();

                    if !dry_run {
                        if let Expression::NumberLiteral { value: num, .. } = &cond.0 {
                            let mut simplified_branch = None;
                            for (match_item, block) in &cases {
                                if let Expression::NumberLiteral {
                                    value: match_num, ..
                                } = match_item
                                {
                                    if match_num == num {
                                        simplified_branch = Some(*block);
                                    }
                                }
                            }
                            cfg.blocks[block_no].instr[instr_no] = Instr::Branch {
                                block: simplified_branch.unwrap_or(*default),
                            };
                            break;
                        }

                        cfg.blocks[block_no].instr[instr_no] = Instr::Switch {
                            cond: cond.0,
                            cases,
                            default: *default,
                        };
                    }
                }
                Instr::ReturnData { data, data_len } => {
                    let data = expression(data, Some(&vars), cfg, ns);
                    let data_len = expression(data_len, Some(&vars), cfg, ns);

                    if !dry_run {
                        cfg.blocks[block_no].instr[instr_no] = Instr::ReturnData {
                            data: data.0,
                            data_len: data_len.0,
                        };
                    }
                }
                Instr::WriteBuffer { buf, offset, value } => {
                    let offset = expression(offset, Some(&vars), cfg, ns).0;
                    let value = expression(value, Some(&vars), cfg, ns).0;

                    if !dry_run {
                        cfg.blocks[block_no].instr[instr_no] = Instr::WriteBuffer {
                            buf: buf.clone(),
                            offset,
                            value,
                        };
                    }
                }
                _ => (),
            }

            reaching_definitions::apply_transfers(
                &cfg.blocks[block_no].transfers[instr_no],
                &mut vars,
            );
        }
    }
}

/// Recursively walk the expression and fold any constant expressions or variables. This function returns the
/// constant folded expression, and a boolean which is true if the value is "pure", the value does not depend
/// on context. This is used for constant folding, so that e.g. an external function call is not constant
/// folded (and moved/copied as a result).
fn expression(
    expr: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    match expr {
        Expression::Add {
            loc,
            ty,
            overflowing,
            left,
            right,
        } => add(loc, ty, *overflowing, left, right, vars, cfg, ns),
        Expression::Subtract {
            loc,
            ty,
            overflowing,
            left,
            right,
        } => subtract(loc, ty, *overflowing, left, right, vars, cfg, ns),
        Expression::AdvancePointer {
            pointer,
            bytes_offset: offset,
        } => advance_pointer(pointer, offset, vars, cfg, ns),
        Expression::Multiply {
            loc,
            ty,
            overflowing,
            left,
            right,
        } => multiply(loc, ty, *overflowing, left, right, vars, cfg, ns),
        Expression::BitwiseAnd {
            loc,
            ty,
            left,
            right,
        } => bitwise_and(loc, ty, left, right, vars, cfg, ns),
        Expression::BitwiseOr {
            loc,
            ty,
            left,
            right,
        } => bitwise_or(loc, ty, left, right, vars, cfg, ns),
        Expression::BitwiseXor {
            loc,
            ty,
            left,
            right,
        } => bitwise_xor(loc, ty, left, right, vars, cfg, ns),
        Expression::ShiftLeft {
            loc,
            ty,
            left: left_expr,
            right: right_expr,
        } => shift_left(loc, ty, left_expr, right_expr, vars, cfg, ns),
        Expression::ShiftRight {
            loc,
            ty,
            left: left_expr,
            right: right_expr,
            signed,
        } => shift_right(loc, ty, left_expr, right_expr, *signed, vars, cfg, ns),
        Expression::Power {
            loc,
            ty,
            overflowing,
            base,
            exp,
        } => power(loc, ty, *overflowing, base, exp, vars, cfg, ns),
        Expression::UnsignedDivide {
            loc,
            ty,
            left,
            right,
        } => divide(loc, ty, left, right, vars, cfg, ns, false),
        Expression::SignedDivide {
            loc,
            ty,
            left,
            right,
        } => divide(loc, ty, left, right, vars, cfg, ns, true),
        Expression::SignedModulo {
            loc,
            ty,
            left,
            right,
        } => modulo(loc, ty, left, right, vars, cfg, ns, true),
        Expression::UnsignedModulo {
            loc,
            ty,
            left,
            right,
        } => modulo(loc, ty, left, right, vars, cfg, ns, false),
        Expression::ZeroExt { loc, ty, expr } => zero_ext(loc, ty, expr, vars, cfg, ns),
        Expression::SignExt { loc, ty, expr } => sign_ext(loc, ty, expr, vars, cfg, ns),
        Expression::Trunc { loc, ty, expr } => trunc(loc, ty, expr, vars, cfg, ns),
        Expression::BitwiseNot { loc, ty, expr } => bitwise_not(loc, ty, expr, vars, cfg, ns),
        Expression::Negate {
            loc,
            ty,
            overflowing,
            expr,
        } => negate(loc, ty, expr, *overflowing, vars, cfg, ns),
        Expression::Variable { loc, ty, var_no } => {
            reference_variable(loc, *var_no, ty, vars, cfg, ns, expr)
        }
        Expression::Builtin {
            loc,
            tys,
            kind:
                kind @ Builtin::Keccak256
                | kind @ Builtin::Ripemd160
                | kind @ Builtin::Blake2_256
                | kind @ Builtin::Blake2_128
                | kind @ Builtin::Sha256,
            args,
        } => builtin_hash(loc, tys, *kind, args, vars, cfg, ns),
        Expression::Keccak256 { loc, ty, exprs } => keccak_256(loc, ty, exprs, vars, cfg, ns),
        // The rest is simply for recursing; no constant expansion should be done
        Expression::StructLiteral { loc, ty, values } => {
            struct_literal(loc, ty, values, vars, cfg, ns)
        }
        Expression::ArrayLiteral {
            loc,
            ty,
            dimensions,
            values,
        } => array_literal(loc, ty, values, dimensions, vars, cfg, ns),
        Expression::ConstArrayLiteral {
            loc,
            ty,
            dimensions,
            values,
        } => const_array_literal(loc, ty, values, dimensions, vars, cfg, ns),
        Expression::Load { loc, ty, expr } => load(loc, ty, expr, vars, cfg, ns),
        Expression::Cast { loc, ty, expr } => cast(loc, ty, expr, vars, cfg, ns),
        Expression::BytesCast {
            loc,
            ty,
            from,
            expr,
        } => bytes_cast(loc, ty, from, expr, vars, cfg, ns),
        Expression::More {
            loc,
            signed,
            left,
            right,
        } => more(loc, left, right, vars, cfg, ns, *signed),
        Expression::Less {
            loc,
            signed,
            left,
            right,
        } => less(loc, left, right, vars, cfg, ns, *signed),
        Expression::MoreEqual {
            loc,
            signed,
            left,
            right,
        } => more_equal(loc, left, right, vars, cfg, ns, *signed),
        Expression::LessEqual {
            loc,
            signed,
            left,
            right,
        } => less_equal(loc, left, right, vars, cfg, ns, *signed),
        Expression::Equal { loc, left, right } => equal(loc, left, right, vars, cfg, ns),
        Expression::NotEqual { loc, left, right } => not_equal(loc, left, right, vars, cfg, ns),
        Expression::Not { loc, expr } => not(loc, expr, vars, cfg, ns),
        Expression::Subscript {
            loc,
            ty,
            array_ty,
            expr,
            index,
        } => subscript(loc, ty, array_ty, expr, index, vars, cfg, ns),
        Expression::StructMember {
            loc,
            ty,
            expr,
            member,
        } => struct_member(loc, ty, expr, *member, vars, cfg, ns),
        Expression::StorageArrayLength {
            loc,
            ty,
            array,
            elem_ty,
        } => storage_array_length(loc, ty, array, elem_ty, vars, cfg, ns),
        Expression::StringCompare { loc, left, right } => {
            string_compare(loc, left, right, vars, cfg, ns)
        }
        Expression::Builtin {
            loc,
            kind: Builtin::Concat,
            args,
            ..
        } => bytes_concat(loc, args, vars, cfg, ns),
        Expression::Builtin {
            loc,
            tys,
            kind,
            args,
        } => builtin(loc, tys, *kind, args, vars, cfg, ns),
        Expression::AllocDynamicBytes {
            loc,
            ty,
            size,
            initializer,
        } => (
            Expression::AllocDynamicBytes {
                loc: *loc,
                ty: ty.clone(),
                size: Box::new(expression(size, vars, cfg, ns).0),
                initializer: initializer.clone(),
            },
            false,
        ),

        Expression::NumberLiteral { .. }
        | Expression::RationalNumberLiteral { .. }
        | Expression::BoolLiteral { .. }
        | Expression::BytesLiteral { .. }
        | Expression::FunctionArg { .. } => (expr.clone(), true),

        Expression::ReturnData { .. }
        | Expression::Undefined { .. }
        | Expression::FormatString { .. }
        | Expression::GetRef { .. }
        | Expression::InternalFunctionCfg { .. } => (expr.clone(), false),
        // nothing else is permitted in cfg
        _ => panic!("expr should not be in cfg: {expr:?}"),
    }
}

fn bigint_to_expression(
    loc: &Loc,
    ty: &Type,
    value: BigInt,
    overflowing: bool,
    ns: &mut Namespace,
) -> (Expression, bool) {
    if !overflowing {
        if let Some(diagnostic) = overflow_diagnostic(&value, ty, loc) {
            ns.diagnostics.push(diagnostic);
        }
    }

    let value = match ty {
        Type::Uint(bits) => {
            if value.sign() == Sign::Minus {
                let mut bs = value.to_signed_bytes_le();
                bs.resize(*bits as usize / 8, 0xff);

                BigInt::from_bytes_le(Sign::Plus, &bs)
            } else if value.bits() > *bits as u64 {
                let (_, mut bs) = value.to_bytes_le();
                bs.truncate(*bits as usize / 8);

                BigInt::from_bytes_le(Sign::Plus, &bs)
            } else {
                value
            }
        }
        Type::Int(bits) => {
            let mut bs = value.to_signed_bytes_le();

            if bs.len() * 8 > *bits as usize {
                bs.truncate(*bits as usize / 8);

                BigInt::from_signed_bytes_le(&bs)
            } else {
                value
            }
        }
        Type::StorageRef(..) => value,
        _ => unreachable!(),
    };

    (
        Expression::NumberLiteral {
            loc: *loc,
            ty: ty.clone(),
            value,
        },
        true,
    )
}

fn get_definition<'a>(
    def: &reaching_definitions::Def,
    cfg: &'a ControlFlowGraph,
) -> Option<&'a Expression> {
    if let Instr::Set { expr, .. } = &cfg.blocks[def.block_no].instr[def.instr_no] {
        Some(expr)
    } else {
        None
    }
}

/// Are these two expressions the same constant-folded value?
fn constants_equal(left: &Expression, right: &Expression) -> bool {
    match left {
        Expression::NumberLiteral { value: left, .. } => match right {
            Expression::NumberLiteral { value: right, .. } => left == right,
            _ => false,
        },
        Expression::BytesLiteral { value: left, .. }
        | Expression::AllocDynamicBytes {
            initializer: Some(left),
            ..
        } => match right {
            Expression::BytesLiteral { value: right, .. }
            | Expression::AllocDynamicBytes {
                initializer: Some(right),
                ..
            } => left == right,
            _ => false,
        },
        _ => false,
    }
}

fn add(
    loc: &pt::Loc,
    ty: &Type,
    overflowing: bool,
    left: &Expression,
    right: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let left = expression(left, vars, cfg, ns);
    let right = expression(right, vars, cfg, ns);

    if let (
        Expression::NumberLiteral { value: left, .. },
        Expression::NumberLiteral { value: right, .. },
    ) = (&left.0, &right.0)
    {
        bigint_to_expression(loc, ty, left.add(right), overflowing, ns)
    } else {
        (
            Expression::Add {
                loc: *loc,
                ty: ty.clone(),
                overflowing,
                left: Box::new(left.0),
                right: Box::new(right.0),
            },
            left.1 && right.1,
        )
    }
}

fn subtract(
    loc: &pt::Loc,
    ty: &Type,
    overflowing: bool,
    left: &Expression,
    right: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let left = expression(left, vars, cfg, ns);
    let right = expression(right, vars, cfg, ns);

    if let (
        Expression::NumberLiteral { value: left, .. },
        Expression::NumberLiteral { value: right, .. },
    ) = (&left.0, &right.0)
    {
        bigint_to_expression(loc, ty, left.sub(right), overflowing, ns)
    } else {
        (
            Expression::Subtract {
                loc: *loc,
                ty: ty.clone(),
                overflowing,
                left: Box::new(left.0),
                right: Box::new(right.0),
            },
            left.1 && right.1,
        )
    }
}

fn advance_pointer(
    pointer: &Expression,
    offset: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    // Only the offset can be simplified
    let offset = expression(offset, vars, cfg, ns);

    match &offset.0 {
        // There is no reason to advance the pointer by a zero offset
        Expression::NumberLiteral { value: num, .. } if num.is_zero() => (pointer.clone(), false),

        _ => (
            Expression::AdvancePointer {
                pointer: Box::new(pointer.clone()),
                bytes_offset: Box::new(offset.0),
            },
            offset.1,
        ),
    }
}

fn multiply(
    loc: &pt::Loc,
    ty: &Type,
    overflowing: bool,
    left: &Expression,
    right: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let left = expression(left, vars, cfg, ns);
    let right = expression(right, vars, cfg, ns);

    if let (
        Expression::NumberLiteral { value: left, .. },
        Expression::NumberLiteral { value: right, .. },
    ) = (&left.0, &right.0)
    {
        bigint_to_expression(loc, ty, left.mul(right), overflowing, ns)
    } else {
        (
            Expression::Multiply {
                loc: *loc,
                ty: ty.clone(),
                overflowing,
                left: Box::new(left.0),
                right: Box::new(right.0),
            },
            left.1 && right.1,
        )
    }
}

fn bitwise_and(
    loc: &pt::Loc,
    ty: &Type,
    left: &Expression,
    right: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let left = expression(left, vars, cfg, ns);
    let right = expression(right, vars, cfg, ns);

    if let (
        Expression::NumberLiteral { value: left, .. },
        Expression::NumberLiteral { value: right, .. },
    ) = (&left.0, &right.0)
    {
        bigint_to_expression(loc, ty, left.bitand(right), true, ns)
    } else {
        (
            Expression::BitwiseAnd {
                loc: *loc,
                ty: ty.clone(),
                left: Box::new(left.0),
                right: Box::new(right.0),
            },
            left.1 && right.1,
        )
    }
}

fn bitwise_or(
    loc: &pt::Loc,
    ty: &Type,
    left: &Expression,
    right: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let left = expression(left, vars, cfg, ns);
    let right = expression(right, vars, cfg, ns);

    if let (
        Expression::NumberLiteral { value: left, .. },
        Expression::NumberLiteral { value: right, .. },
    ) = (&left.0, &right.0)
    {
        bigint_to_expression(loc, ty, left.bitor(right), true, ns)
    } else {
        (
            Expression::BitwiseOr {
                loc: *loc,
                ty: ty.clone(),
                left: Box::new(left.0),
                right: Box::new(right.0),
            },
            left.1 && right.1,
        )
    }
}

fn bitwise_xor(
    loc: &pt::Loc,
    ty: &Type,
    left: &Expression,
    right: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let left = expression(left, vars, cfg, ns);
    let right = expression(right, vars, cfg, ns);

    if let (
        Expression::NumberLiteral { value: left, .. },
        Expression::NumberLiteral { value: right, .. },
    ) = (&left.0, &right.0)
    {
        bigint_to_expression(loc, ty, left.bitxor(right), true, ns)
    } else {
        (
            Expression::BitwiseXor {
                loc: *loc,
                ty: ty.clone(),
                left: Box::new(left.0),
                right: Box::new(right.0),
            },
            left.1 && right.1,
        )
    }
}

fn shift_left(
    loc: &pt::Loc,
    ty: &Type,
    left_expr: &Expression,
    right_expr: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let left = expression(left_expr, vars, cfg, ns);
    let right = expression(right_expr, vars, cfg, ns);

    if let (
        Expression::NumberLiteral { value: left, .. },
        Expression::NumberLiteral { value: right, .. },
    ) = (&left.0, &right.0)
    {
        if right.sign() == Sign::Minus || right >= &BigInt::from(left_expr.ty().bits(ns)) {
            ns.diagnostics.push(Diagnostic::error(
                *loc,
                format!("left shift by {right} is not possible"),
            ));
        } else {
            let right: u64 = right.to_u64().unwrap();

            return bigint_to_expression(loc, ty, left.shl(&right), true, ns);
        }
    }
    (
        Expression::ShiftLeft {
            loc: *loc,
            ty: ty.clone(),
            left: Box::new(left.0),
            right: Box::new(right.0),
        },
        left.1 && right.1,
    )
}

fn shift_right(
    loc: &pt::Loc,
    ty: &Type,
    left_expr: &Expression,
    right_expr: &Expression,
    signed: bool,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let left = expression(left_expr, vars, cfg, ns);
    let right = expression(right_expr, vars, cfg, ns);

    if let (
        Expression::NumberLiteral { value: left, .. },
        Expression::NumberLiteral { value: right, .. },
    ) = (&left.0, &right.0)
    {
        if right.sign() == Sign::Minus || right >= &BigInt::from(left_expr.ty().bits(ns)) {
            ns.diagnostics.push(Diagnostic::error(
                *loc,
                format!("right shift by {right} is not possible"),
            ));
        } else {
            let right: u64 = right.to_u64().unwrap();

            return bigint_to_expression(loc, ty, left.shr(&right), true, ns);
        }
    }

    (
        Expression::ShiftRight {
            loc: *loc,
            ty: ty.clone(),
            left: Box::new(left.0),
            right: Box::new(right.0),
            signed,
        },
        left.1 && right.1,
    )
}

fn power(
    loc: &pt::Loc,
    ty: &Type,
    overflowing: bool,
    base: &Expression,
    exp: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let base = expression(base, vars, cfg, ns);
    let exp = expression(exp, vars, cfg, ns);

    if let (
        Expression::NumberLiteral { value: left, .. },
        Expression::NumberLiteral { value: right, .. },
    ) = (&base.0, &exp.0)
    {
        if right.sign() == Sign::Minus || right >= &BigInt::from(u16::MAX) {
            ns.diagnostics.push(Diagnostic::error(
                *loc,
                format!("power {right} not possible"),
            ));
        } else {
            let right: u32 = right.to_u32().unwrap();

            return bigint_to_expression(loc, ty, left.pow(right), overflowing, ns);
        }
    }

    (
        Expression::Power {
            loc: *loc,
            ty: ty.clone(),
            overflowing,
            base: Box::new(base.0),
            exp: Box::new(exp.0),
        },
        base.1 && exp.1,
    )
}

fn divide(
    loc: &pt::Loc,
    ty: &Type,
    left: &Expression,
    right: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
    signed: bool,
) -> (Expression, bool) {
    let left = expression(left, vars, cfg, ns);
    let right = expression(right, vars, cfg, ns);

    if let Expression::NumberLiteral { value: right, .. } = &right.0 {
        if right.is_zero() {
            ns.diagnostics
                .push(Diagnostic::error(*loc, String::from("divide by zero")));
        } else if let Expression::NumberLiteral { value: left, .. } = &left.0 {
            return bigint_to_expression(loc, ty, left.div(right), false, ns);
        }
    }
    (
        if signed {
            Expression::SignedDivide {
                loc: *loc,
                ty: ty.clone(),
                left: Box::new(left.0),
                right: Box::new(right.0),
            }
        } else {
            Expression::UnsignedDivide {
                loc: *loc,
                ty: ty.clone(),
                left: Box::new(left.0),
                right: Box::new(right.0),
            }
        },
        left.1 && right.1,
    )
}

fn modulo(
    loc: &pt::Loc,
    ty: &Type,
    left: &Expression,
    right: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
    signed: bool,
) -> (Expression, bool) {
    let left = expression(left, vars, cfg, ns);
    let right = expression(right, vars, cfg, ns);

    if let Expression::NumberLiteral { value: right, .. } = &right.0 {
        if right.is_zero() {
            ns.diagnostics
                .push(Diagnostic::error(*loc, String::from("divide by zero")));
        } else if let Expression::NumberLiteral { value: left, .. } = &left.0 {
            return bigint_to_expression(loc, ty, left.rem(right), false, ns);
        }
    }

    (
        if signed {
            Expression::SignedModulo {
                loc: *loc,
                ty: ty.clone(),
                left: Box::new(left.0),
                right: Box::new(right.0),
            }
        } else {
            Expression::UnsignedModulo {
                loc: *loc,
                ty: ty.clone(),
                left: Box::new(left.0),
                right: Box::new(right.0),
            }
        },
        left.1 && right.1,
    )
}

fn zero_ext(
    loc: &pt::Loc,
    ty: &Type,
    expr: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let expr = expression(expr, vars, cfg, ns);
    if let Expression::NumberLiteral { value, .. } = expr.0 {
        (
            Expression::NumberLiteral {
                loc: *loc,
                ty: ty.clone(),
                value,
            },
            true,
        )
    } else {
        (
            Expression::ZeroExt {
                loc: *loc,
                ty: ty.clone(),
                expr: Box::new(expr.0),
            },
            expr.1,
        )
    }
}

fn sign_ext(
    loc: &pt::Loc,
    ty: &Type,
    expr: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let expr = expression(expr, vars, cfg, ns);
    if let Expression::NumberLiteral { value, .. } = expr.0 {
        (
            Expression::NumberLiteral {
                loc: *loc,
                ty: ty.clone(),
                value,
            },
            true,
        )
    } else {
        (
            Expression::SignExt {
                loc: *loc,
                ty: ty.clone(),
                expr: Box::new(expr.0),
            },
            expr.1,
        )
    }
}

fn trunc(
    loc: &pt::Loc,
    ty: &Type,
    expr: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let expr = expression(expr, vars, cfg, ns);
    if let Expression::NumberLiteral { value, .. } = expr.0 {
        bigint_to_expression(loc, ty, value, true, ns)
    } else {
        (
            Expression::Trunc {
                loc: *loc,
                ty: ty.clone(),
                expr: Box::new(expr.0),
            },
            expr.1,
        )
    }
}

fn bitwise_not(
    loc: &pt::Loc,
    ty: &Type,
    expr: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let expr = expression(expr, vars, cfg, ns);
    if let Expression::NumberLiteral { value, .. } = expr.0 {
        bigint_to_expression(loc, ty, !value, true, ns)
    } else {
        (
            Expression::BitwiseNot {
                loc: *loc,
                ty: ty.clone(),
                expr: Box::new(expr.0),
            },
            expr.1,
        )
    }
}

fn negate(
    loc: &pt::Loc,
    ty: &Type,
    expr: &Expression,
    overflowing: bool,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let expr = expression(expr, vars, cfg, ns);
    if let Expression::NumberLiteral { value, .. } = expr.0 {
        bigint_to_expression(loc, ty, -value, overflowing, ns)
    } else {
        (
            Expression::Negate {
                loc: *loc,
                ty: ty.clone(),
                overflowing,
                expr: Box::new(expr.0),
            },
            expr.1,
        )
    }
}

fn reference_variable(
    loc: &pt::Loc,
    var: usize,
    ty: &Type,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
    expr: &Expression,
) -> (Expression, bool) {
    if !matches!(ty, Type::Ref(_) | Type::StorageRef(..)) {
        if let Some(vars) = vars {
            if let Some(defs) = vars.get(&var) {
                // There must be at least one definition, and all should evaluate to the same value
                let mut v = None;

                for def in defs.keys() {
                    if let Some(expr) = get_definition(def, cfg) {
                        let expr = expression(expr, None, cfg, ns);

                        if expr.1 {
                            if let Some(last) = &v {
                                if !constants_equal(last, &expr.0) {
                                    v = None;
                                    break;
                                }
                            }

                            v = Some(expr.0);
                        } else {
                            v = None;
                            break;
                        }
                    } else {
                        v = None;
                        break;
                    }
                }

                if let Some(expr) = v {
                    if *loc != Loc::Codegen {
                        ns.var_constants.insert(*loc, expr.clone());
                    }
                    return (expr, true);
                }
            }
        }
    }

    (expr.clone(), false)
}

fn builtin_hash(
    loc: &pt::Loc,
    tys: &[Type],
    kind: Builtin,
    args: &[Expression],
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let arg = expression(&args[0], vars, cfg, ns);

    if let Expression::AllocDynamicBytes {
        initializer: Some(bs),
        ..
    } = arg.0
    {
        let hash: Vec<u8> = match kind {
            Builtin::Keccak256 => {
                let mut hasher = Keccak::v256();
                hasher.update(&bs);
                let mut hash = [0u8; 32];
                hasher.finalize(&mut hash);
                hash.to_vec()
            }
            Builtin::Ripemd160 => {
                let mut hasher = Ripemd160::new();
                hasher.update(&bs);
                let result = hasher.finalize();
                result[..].to_vec()
            }
            Builtin::Blake2_256 => {
                let hash = blake2_rfc::blake2b::blake2b(32, &[], &bs);
                hash.as_bytes().to_vec()
            }
            Builtin::Blake2_128 => {
                let hash = blake2_rfc::blake2b::blake2b(16, &[], &bs);
                hash.as_bytes().to_vec()
            }
            Builtin::Sha256 => {
                let mut hasher = Sha256::new();
                hasher.update(&bs);
                let result = hasher.finalize();
                result[..].to_vec()
            }
            _ => unreachable!("not a hash"),
        };

        (
            Expression::BytesLiteral {
                loc: *loc,
                ty: tys[0].clone(),
                value: hash,
            },
            true,
        )
    } else {
        (
            Expression::Builtin {
                loc: *loc,
                tys: tys.to_owned(),
                kind,
                args: vec![arg.0],
            },
            false,
        )
    }
}

fn keccak_256(
    loc: &pt::Loc,
    ty: &Type,
    args: &[Expression],
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let mut all_constant = true;
    let mut hasher = Keccak::v256();

    let args = args
        .iter()
        .map(|expr| {
            let (expr, _) = expression(expr, vars, cfg, ns);

            if all_constant {
                match &expr {
                    Expression::AllocDynamicBytes {
                        initializer: Some(value),
                        ..
                    }
                    | Expression::BytesLiteral { value, .. } => {
                        hasher.update(value);
                    }
                    Expression::NumberLiteral { ty, value, .. } => {
                        let (sign, mut bs) = value.to_bytes_le();

                        match ty {
                            Type::Enum(_) => bs.resize(1, 0),
                            Type::Uint(bits) => bs.resize(*bits as usize / 8, 0),
                            Type::Int(bits) => {
                                let v = if sign == Sign::Minus { 0xffu8 } else { 0 };

                                bs.resize(*bits as usize / 8, v);
                            }
                            Type::Bytes(n) => {
                                while (*n as usize) < bs.len() {
                                    bs.insert(0, 0);
                                }
                            }
                            Type::Address(_) => {
                                bs.resize(ns.address_length, 0);
                            }
                            _ => unreachable!(),
                        }

                        hasher.update(&bs);
                    }
                    _ => {
                        all_constant = false;
                    }
                }
            }

            expr
        })
        .collect();

    if all_constant {
        let mut hash = [0u8; 32];
        hasher.finalize(&mut hash);
        let mut hash = hash.to_vec();
        hash.reverse();

        (
            Expression::BytesLiteral {
                loc: *loc,
                ty: ty.clone(),
                value: hash,
            },
            true,
        )
    } else {
        (
            Expression::Keccak256 {
                loc: *loc,
                ty: ty.clone(),
                exprs: args,
            },
            false,
        )
    }
}

fn struct_literal(
    loc: &pt::Loc,
    ty: &Type,
    args: &[Expression],
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let args = args
        .iter()
        .map(|expr| expression(expr, vars, cfg, ns).0)
        .collect();

    (
        Expression::StructLiteral {
            loc: *loc,
            ty: ty.clone(),
            values: args,
        },
        false,
    )
}

fn array_literal(
    loc: &pt::Loc,
    ty: &Type,
    args: &[Expression],
    dimensions: &[u32],
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let args = args
        .iter()
        .map(|expr| expression(expr, vars, cfg, ns).0)
        .collect();

    (
        Expression::ArrayLiteral {
            loc: *loc,
            ty: ty.clone(),
            dimensions: dimensions.to_owned(),
            values: args,
        },
        false,
    )
}

fn const_array_literal(
    loc: &pt::Loc,
    ty: &Type,
    args: &[Expression],
    dimensions: &[u32],
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let args = args
        .iter()
        .map(|expr| expression(expr, vars, cfg, ns).0)
        .collect();

    (
        Expression::ConstArrayLiteral {
            loc: *loc,
            ty: ty.clone(),
            dimensions: dimensions.to_owned(),
            values: args,
        },
        false,
    )
}

fn load(
    loc: &pt::Loc,
    ty: &Type,
    expr: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let (expr, _) = expression(expr, vars, cfg, ns);

    (
        Expression::Load {
            loc: *loc,
            ty: ty.clone(),
            expr: Box::new(expr),
        },
        false,
    )
}

fn cast(
    loc: &pt::Loc,
    ty: &Type,
    expr: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let (expr, _) = expression(expr, vars, cfg, ns);

    (
        Expression::Cast {
            loc: *loc,
            ty: ty.clone(),
            expr: Box::new(expr),
        },
        false,
    )
}

fn bytes_cast(
    loc: &pt::Loc,
    to: &Type,
    from: &Type,
    expr: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let (expr, _) = expression(expr, vars, cfg, ns);

    if let Expression::NumberLiteral {
        loc,
        ty: Type::Bytes(len),
        value,
    } = expr
    {
        let (_, mut bs) = value.to_bytes_be();

        while bs.len() < len as usize {
            bs.insert(0, 0);
        }

        (
            Expression::AllocDynamicBytes {
                loc,
                ty: Type::DynamicBytes,
                size: Expression::NumberLiteral {
                    loc,
                    ty: Type::Uint(32),
                    value: len.into(),
                }
                .into(),
                initializer: Some(bs),
            },
            false,
        )
    } else {
        (
            Expression::BytesCast {
                loc: *loc,
                ty: to.clone(),
                from: from.clone(),
                expr: Box::new(expr),
            },
            false,
        )
    }
}

fn more(
    loc: &pt::Loc,
    left: &Expression,
    right: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
    signed: bool,
) -> (Expression, bool) {
    let left = expression(left, vars, cfg, ns);
    let right = expression(right, vars, cfg, ns);

    (
        Expression::More {
            loc: *loc,
            signed,
            left: Box::new(left.0),
            right: Box::new(right.0),
        },
        false,
    )
}

fn less(
    loc: &pt::Loc,
    left: &Expression,
    right: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
    signed: bool,
) -> (Expression, bool) {
    let left = expression(left, vars, cfg, ns);
    let right = expression(right, vars, cfg, ns);

    (
        Expression::Less {
            loc: *loc,
            signed,
            left: Box::new(left.0),
            right: Box::new(right.0),
        },
        false,
    )
}

fn more_equal(
    loc: &pt::Loc,
    left: &Expression,
    right: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
    signed: bool,
) -> (Expression, bool) {
    let left = expression(left, vars, cfg, ns);
    let right = expression(right, vars, cfg, ns);

    (
        Expression::MoreEqual {
            loc: *loc,
            signed,
            left: Box::new(left.0),
            right: Box::new(right.0),
        },
        false,
    )
}

fn less_equal(
    loc: &pt::Loc,
    left: &Expression,
    right: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
    signed: bool,
) -> (Expression, bool) {
    let left = expression(left, vars, cfg, ns);
    let right = expression(right, vars, cfg, ns);

    (
        Expression::LessEqual {
            loc: *loc,
            signed,
            left: Box::new(left.0),
            right: Box::new(right.0),
        },
        false,
    )
}

fn equal(
    loc: &pt::Loc,
    left: &Expression,
    right: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let left = expression(left, vars, cfg, ns);
    let right = expression(right, vars, cfg, ns);

    if let (Expression::BytesLiteral { value: l, .. }, Expression::BytesLiteral { value: r, .. }) =
        (&left.0, &right.0)
    {
        (
            Expression::BoolLiteral {
                loc: *loc,
                value: l == r,
            },
            true,
        )
    } else {
        (
            Expression::Equal {
                loc: *loc,
                left: Box::new(left.0),
                right: Box::new(right.0),
            },
            false,
        )
    }
}

fn not_equal(
    loc: &pt::Loc,
    left: &Expression,
    right: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let left = expression(left, vars, cfg, ns);
    let right = expression(right, vars, cfg, ns);

    if let (Expression::BytesLiteral { value: l, .. }, Expression::BytesLiteral { value: r, .. }) =
        (&left.0, &right.0)
    {
        (
            Expression::BoolLiteral {
                loc: *loc,
                value: l != r,
            },
            true,
        )
    } else {
        (
            Expression::NotEqual {
                loc: *loc,
                left: Box::new(left.0),
                right: Box::new(right.0),
            },
            false,
        )
    }
}

fn not(
    loc: &pt::Loc,
    expr: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let expr = expression(expr, vars, cfg, ns);

    (
        Expression::Not {
            loc: *loc,
            expr: Box::new(expr.0),
        },
        expr.1,
    )
}

fn subscript(
    loc: &pt::Loc,
    ty: &Type,
    array_ty: &Type,
    array: &Expression,
    index: &Expression,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let array = expression(array, vars, cfg, ns);
    let index = expression(index, vars, cfg, ns);

    (
        Expression::Subscript {
            loc: *loc,
            ty: ty.clone(),
            array_ty: array_ty.clone(),
            expr: Box::new(array.0),
            index: Box::new(index.0),
        },
        false,
    )
}

fn struct_member(
    loc: &pt::Loc,
    ty: &Type,
    expr: &Expression,
    member: usize,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let strct = expression(expr, vars, cfg, ns);

    (
        Expression::StructMember {
            loc: *loc,
            ty: ty.clone(),
            expr: Box::new(strct.0),
            member,
        },
        false,
    )
}

fn storage_array_length(
    loc: &pt::Loc,
    ty: &Type,
    array: &Expression,
    elem_ty: &Type,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let array = expression(array, vars, cfg, ns);

    (
        Expression::StorageArrayLength {
            loc: *loc,
            ty: ty.clone(),
            array: Box::new(array.0),
            elem_ty: elem_ty.clone(),
        },
        false,
    )
}

fn string_compare(
    loc: &pt::Loc,
    left: &StringLocation<Expression>,
    right: &StringLocation<Expression>,
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    if let (StringLocation::CompileTime(left), StringLocation::CompileTime(right)) = (left, right) {
        (
            Expression::BoolLiteral {
                loc: *loc,
                value: left == right,
            },
            true,
        )
    } else {
        let left = if let StringLocation::RunTime(left) = left {
            StringLocation::RunTime(Box::new(expression(left, vars, cfg, ns).0))
        } else {
            left.clone()
        };

        let right = if let StringLocation::RunTime(right) = right {
            StringLocation::RunTime(Box::new(expression(right, vars, cfg, ns).0))
        } else {
            right.clone()
        };

        (
            Expression::StringCompare {
                loc: *loc,
                left,
                right,
            },
            false,
        )
    }
}

fn bytes_concat(
    loc: &pt::Loc,
    args: &[Expression],
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let mut last = None;
    let mut res = Vec::new();

    for arg in args {
        let expr = expression(arg, vars, cfg, ns).0;

        if let Expression::AllocDynamicBytes {
            initializer: Some(bs),
            ..
        } = &expr
        {
            if bs.is_empty() {
                continue;
            }

            if let Some(Expression::AllocDynamicBytes {
                size,
                initializer: Some(init),
                ..
            }) = &mut last
            {
                let Expression::NumberLiteral { value, .. } = size.as_mut() else {
                    unreachable!();
                };

                *value += bs.len();

                init.extend_from_slice(bs);
            } else {
                last = Some(expr);
            }
        } else {
            if let Some(expr) = last {
                res.push(expr);
                last = None;
            }
            res.push(expr);
        }
    }

    if res.is_empty() {
        if let Some(expr) = last {
            (expr, false)
        } else {
            (
                Expression::AllocDynamicBytes {
                    loc: *loc,
                    ty: Type::DynamicBytes,
                    size: Expression::NumberLiteral {
                        loc: *loc,
                        ty: Type::Uint(32),
                        value: 0.into(),
                    }
                    .into(),
                    initializer: None,
                },
                false,
            )
        }
    } else {
        if let Some(expr) = last {
            res.push(expr);
        }

        if res.len() == 1 {
            (res[0].clone(), false)
        } else {
            (
                Expression::Builtin {
                    loc: *loc,
                    tys: vec![Type::DynamicBytes],
                    kind: Builtin::Concat,
                    args: res,
                },
                false,
            )
        }
    }
}

fn builtin(
    loc: &pt::Loc,
    tys: &[Type],
    kind: Builtin,
    args: &[Expression],
    vars: Option<&reaching_definitions::VarDefs>,
    cfg: &ControlFlowGraph,
    ns: &mut Namespace,
) -> (Expression, bool) {
    let args = args
        .iter()
        .map(|expr| expression(expr, vars, cfg, ns).0)
        .collect();

    (
        Expression::Builtin {
            loc: *loc,
            tys: tys.to_owned(),
            kind,
            args,
        },
        false,
    )
}
