// SPDX-License-Identifier: Apache-2.0

use crate::codegen::Expression;
use crate::sema::ast;
use num_bigint::BigInt;
use num_traits::FromPrimitive;
use num_traits::One;
use num_traits::Zero;

use super::expression::{expression, load_storage};
use super::revert::PanicCode;
use super::revert::SolidityError;
use super::Options;
use super::{
    cfg::{ControlFlowGraph, Instr},
    vartable::Vartable,
};
use crate::codegen::revert::{assert_failure, log_runtime_error};
use crate::sema::ast::{Function, Namespace, RetrieveType, Type};
use solang_parser::pt;

/// Given a storage slot which is the start of the array, calculate the
/// offset of the array element. This function exists to avoid doing
/// 256 bit multiply if possible.
pub fn array_offset(
    loc: &pt::Loc,
    start: Expression,
    index: Expression,
    elem_ty: Type,
    ns: &Namespace,
) -> Expression {
    let elem_size = elem_ty.storage_slots(ns);
    let slot_ty = ns.storage_type();

    // the index needs to be cast to i256 and multiplied by the number
    // of slots for each element
    if elem_size == BigInt::one() {
        Expression::Add {
            loc: *loc,
            ty: slot_ty,
            overflowing: true,
            left: Box::new(start),
            right: Box::new(index),
        }
    } else if (elem_size.clone() & (elem_size.clone() - BigInt::one())) == BigInt::zero() {
        // elem_size is power of 2
        Expression::Add {
            loc: *loc,
            ty: slot_ty.clone(),
            overflowing: true,
            left: Box::new(start),
            right: Box::new(Expression::ShiftLeft {
                loc: *loc,
                ty: slot_ty.clone(),
                left: Box::new(index),
                right: Box::new(Expression::NumberLiteral {
                    loc: *loc,
                    ty: slot_ty,
                    value: BigInt::from_u64(elem_size.bits() - 1).unwrap(),
                }),
            }),
        }
    } else {
        Expression::Add {
            loc: *loc,
            ty: slot_ty.clone(),
            overflowing: true,
            left: Box::new(start),
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
    }
}

/// Push() method on dynamic array in storage
pub fn storage_slots_array_push(
    loc: &pt::Loc,
    args: &[ast::Expression],
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
) -> Expression {
    // set array+length to val_expr
    let slot_ty = ns.storage_type();
    let length_pos = vartab.temp_anonymous(&slot_ty);

    let var_expr = expression(&args[0], cfg, contract_no, func, ns, vartab, opt);

    // TODO(Soroban): Storage type here is None, since arrays are not yet supported in Soroban
    let expr = load_storage(loc, &slot_ty, var_expr.clone(), cfg, vartab, None);

    cfg.add(
        vartab,
        Instr::Set {
            loc: pt::Loc::Codegen,
            res: length_pos,
            expr,
        },
    );

    let elem_ty = args[0].ty().storage_array_elem();

    let entry_pos = vartab.temp_anonymous(&slot_ty);

    cfg.add(
        vartab,
        Instr::Set {
            loc: pt::Loc::Codegen,
            res: entry_pos,
            expr: array_offset(
                loc,
                Expression::Keccak256 {
                    loc: *loc,
                    ty: slot_ty.clone(),
                    exprs: vec![var_expr.clone()],
                },
                Expression::Variable {
                    loc: *loc,
                    ty: slot_ty.clone(),
                    var_no: length_pos,
                },
                elem_ty.clone(),
                ns,
            ),
        },
    );

    if args.len() == 2 {
        let value = expression(&args[1], cfg, contract_no, func, ns, vartab, opt);

        cfg.add(
            vartab,
            Instr::SetStorage {
                ty: elem_ty.clone(),
                value,
                storage: Expression::Variable {
                    loc: *loc,
                    ty: slot_ty.clone(),
                    var_no: entry_pos,
                },
                storage_type: None,
            },
        );
    }

    // increase length
    let new_length = Expression::Add {
        loc: *loc,
        ty: slot_ty.clone(),
        overflowing: true,
        left: Box::new(Expression::Variable {
            loc: *loc,
            ty: slot_ty.clone(),
            var_no: length_pos,
        }),
        right: Box::new(Expression::NumberLiteral {
            loc: *loc,
            ty: slot_ty.clone(),
            value: BigInt::one(),
        }),
    };

    cfg.add(
        vartab,
        Instr::SetStorage {
            ty: slot_ty,
            value: new_length,
            storage: var_expr,
            storage_type: None,
        },
    );

    if args.len() == 1 {
        Expression::Variable {
            loc: *loc,
            ty: elem_ty,
            var_no: entry_pos,
        }
    } else {
        Expression::Poison
    }
}

/// Pop() method on dynamic array in storage
pub fn storage_slots_array_pop(
    loc: &pt::Loc,
    args: &[ast::Expression],
    return_ty: &Type,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
) -> Expression {
    // set array+length to val_expr
    let slot_ty = ns.storage_type();
    let length_ty = ns.storage_type();
    let length_pos = vartab.temp_anonymous(&slot_ty);

    let ty = args[0].ty();
    let var_expr = expression(&args[0], cfg, contract_no, func, ns, vartab, opt);
    // TODO(Soroban): Storage type here is None, since arrays are not yet supported in Soroban
    let expr = load_storage(loc, &length_ty, var_expr.clone(), cfg, vartab, None);

    cfg.add(
        vartab,
        Instr::Set {
            loc: pt::Loc::Codegen,
            res: length_pos,
            expr,
        },
    );

    let empty_array = cfg.new_basic_block("empty_array".to_string());
    let has_elements = cfg.new_basic_block("has_elements".to_string());

    cfg.add(
        vartab,
        Instr::BranchCond {
            cond: Expression::Equal {
                loc: *loc,
                left: Box::new(Expression::Variable {
                    loc: *loc,
                    ty: length_ty.clone(),
                    var_no: length_pos,
                }),
                right: Box::new(Expression::NumberLiteral {
                    loc: *loc,
                    ty: length_ty.clone(),
                    value: BigInt::zero(),
                }),
            },
            true_block: empty_array,
            false_block: has_elements,
        },
    );

    cfg.set_basic_block(empty_array);
    log_runtime_error(
        opt.log_runtime_errors,
        "pop from empty storage array",
        *loc,
        cfg,
        vartab,
        ns,
    );
    let error = SolidityError::Panic(PanicCode::EmptyArrayPop);
    assert_failure(loc, error, ns, cfg, vartab);

    cfg.set_basic_block(has_elements);
    let new_length = vartab.temp_anonymous(&slot_ty);

    cfg.add(
        vartab,
        Instr::Set {
            loc: pt::Loc::Codegen,
            res: new_length,
            expr: Expression::Subtract {
                loc: *loc,
                ty: length_ty.clone(),
                overflowing: true,
                left: Box::new(Expression::Variable {
                    loc: *loc,
                    ty: length_ty.clone(),
                    var_no: length_pos,
                }),
                right: Box::new(Expression::NumberLiteral {
                    loc: *loc,
                    ty: length_ty,
                    value: BigInt::one(),
                }),
            },
        },
    );

    // The array element will be loaded before clearing. So, the return
    // type of pop() is the derefenced array dereference
    let elem_ty = ty.storage_array_elem().deref_any().clone();
    let entry_pos = vartab.temp_anonymous(&slot_ty);

    cfg.add(
        vartab,
        Instr::Set {
            loc: pt::Loc::Codegen,
            res: entry_pos,
            expr: array_offset(
                loc,
                Expression::Keccak256 {
                    loc: *loc,
                    ty: slot_ty.clone(),
                    exprs: vec![var_expr.clone()],
                },
                Expression::Variable {
                    loc: *loc,
                    ty: slot_ty.clone(),
                    var_no: new_length,
                },
                elem_ty.clone(),
                ns,
            ),
        },
    );

    let val = if *return_ty != Type::Void {
        let res_pos = vartab.temp_anonymous(&elem_ty);

        let expr = load_storage(
            loc,
            &elem_ty,
            Expression::Variable {
                loc: *loc,
                ty: elem_ty.clone(),
                var_no: entry_pos,
            },
            cfg,
            vartab,
            None,
        );

        cfg.add(
            vartab,
            Instr::Set {
                loc: *loc,
                res: res_pos,
                expr,
            },
        );
        Expression::Variable {
            loc: *loc,
            ty: elem_ty.clone(),
            var_no: res_pos,
        }
    } else {
        Expression::Undefined {
            ty: elem_ty.clone(),
        }
    };

    cfg.add(
        vartab,
        Instr::ClearStorage {
            ty: elem_ty,
            storage: Expression::Variable {
                loc: *loc,
                ty: slot_ty.clone(),
                var_no: entry_pos,
            },
        },
    );

    // set decrease length
    cfg.add(
        vartab,
        Instr::SetStorage {
            ty: slot_ty.clone(),
            value: Expression::Variable {
                loc: *loc,
                ty: slot_ty,
                var_no: new_length,
            },
            storage: var_expr,
            storage_type: None,
        },
    );

    val
}

/// Push() method on array or bytes in storage
pub fn array_push(
    loc: &pt::Loc,
    args: &[ast::Expression],
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
) -> Expression {
    let storage = expression(&args[0], cfg, contract_no, func, ns, vartab, opt);

    let mut ty = args[0].ty().storage_array_elem();

    let value = if args.len() > 1 {
        Some(expression(
            &args[1],
            cfg,
            contract_no,
            func,
            ns,
            vartab,
            opt,
        ))
    } else {
        ty.deref_any().default(ns)
    };

    if !ty.is_reference_type(ns) {
        ty = ty.deref_into();
    }

    let res = vartab.temp_anonymous(&ty);

    cfg.add(
        vartab,
        Instr::PushStorage {
            res,
            ty: ty.deref_any().clone(),
            storage,
            value,
        },
    );

    Expression::Variable {
        loc: *loc,
        ty,
        var_no: res,
    }
}

/// Pop() method on array or bytes in storage
pub fn array_pop(
    loc: &pt::Loc,
    args: &[ast::Expression],
    return_ty: &Type,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
) -> Expression {
    let storage = expression(&args[0], cfg, contract_no, func, ns, vartab, opt);

    let ty = args[0].ty().storage_array_elem().deref_into();

    let res = if *return_ty != Type::Void {
        Some(vartab.temp_anonymous(&ty))
    } else {
        None
    };

    cfg.add(
        vartab,
        Instr::PopStorage {
            res,
            ty: ty.clone(),
            storage,
        },
    );

    if let Some(res) = res {
        Expression::Variable {
            loc: *loc,
            ty,
            var_no: res,
        }
    } else {
        Expression::Undefined { ty }
    }
}
