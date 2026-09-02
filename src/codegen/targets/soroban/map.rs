// SPDX-License-Identifier: Apache-2.0

use super::soroban_default_handle;
use crate::codegen::cfg::{ControlFlowGraph, Instr, InternalCallTy};
use crate::codegen::vartable::Vartable;
use crate::codegen::{Expression, HostFunctions};
use crate::sema::ast::{Namespace, Type};
use num_bigint::BigInt;
use solang_parser::pt;

fn map_handle_ty(map_ty: &Type) -> Type {
    let inner = if let Type::StorageRef(_, inner) = map_ty {
        inner.as_ref().clone()
    } else {
        map_ty.clone()
    };
    Type::SorobanHandle(Box::new(inner))
}

pub(crate) fn soroban_map_new(
    loc: &pt::Loc,
    map_ty: &Type,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
) -> Expression {
    let handle_ty = map_handle_ty(map_ty);
    let no = vartab.temp_name("soroban_map_new", &handle_ty);
    cfg.add(
        vartab,
        Instr::Call {
            call: InternalCallTy::HostFunction {
                name: HostFunctions::MapNew.name().to_string(),
            },
            args: vec![],
            return_tys: vec![handle_ty.clone()],
            res: vec![no],
        },
    );
    Expression::Variable {
        loc: *loc,
        ty: handle_ty,
        var_no: no,
    }
}

pub(crate) fn map_has(
    loc: &pt::Loc,
    handle: Expression,
    key_val: Expression,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
) -> Expression {
    host_map(
        loc,
        HostFunctions::MapHas,
        "path_map_has",
        vec![handle, key_val],
        cfg,
        vartab,
    )
}

pub(crate) fn map_get(
    loc: &pt::Loc,
    handle: Expression,
    key_val: Expression,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
) -> Expression {
    host_map(
        loc,
        HostFunctions::MapGet,
        "path_map_get",
        vec![handle, key_val],
        cfg,
        vartab,
    )
}

pub(crate) fn map_put(
    loc: &pt::Loc,
    handle: Expression,
    key_val: Expression,
    value: Expression,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
) -> Expression {
    host_map(
        loc,
        HostFunctions::MapPut,
        "path_map_put",
        vec![handle, key_val, value],
        cfg,
        vartab,
    )
}

pub(crate) fn map_del(
    loc: &pt::Loc,
    handle: Expression,
    key_val: Expression,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
) -> Expression {
    host_map(
        loc,
        HostFunctions::MapDel,
        "path_map_del",
        vec![handle, key_val],
        cfg,
        vartab,
    )
}

fn host_map(
    loc: &pt::Loc,
    host_fn: HostFunctions,
    name: &str,
    args: Vec<Expression>,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
) -> Expression {
    let no = vartab.temp_name(name, &Type::Uint(64));
    cfg.add(
        vartab,
        Instr::Call {
            res: vec![no],
            return_tys: vec![Type::Uint(64)],
            call: InternalCallTy::HostFunction {
                name: host_fn.name().to_string(),
            },
            args,
        },
    );
    Expression::Variable {
        loc: *loc,
        ty: Type::Uint(64),
        var_no: no,
    }
}

pub(crate) fn is_true(handle: Expression) -> Expression {
    Expression::NotEqual {
        loc: pt::Loc::Codegen,
        left: Box::new(handle),
        right: Box::new(Expression::NumberLiteral {
            loc: pt::Loc::Codegen,
            ty: Type::Uint(64),
            value: BigInt::from(0),
        }),
    }
}

pub(crate) fn map_get_or_default(
    loc: &pt::Loc,
    parent: Expression,
    key_val: Expression,
    val_ty: &Type,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    ns: &Namespace,
) -> Expression {
    let res_no = vartab.temp_name("map_get_or_default", &Type::Uint(64));
    let result = Expression::Variable {
        loc: *loc,
        ty: Type::Uint(64),
        var_no: res_no,
    };

    let has = map_has(loc, parent.clone(), key_val.clone(), cfg, vartab);

    vartab.new_dirty_tracker();
    let exists = cfg.new_basic_block("map_key_exists".to_string());
    let create = cfg.new_basic_block("map_key_create".to_string());
    let merge = cfg.new_basic_block("map_get_or_default_merge".to_string());

    cfg.add(
        vartab,
        Instr::BranchCond {
            cond: is_true(has),
            true_block: exists,
            false_block: create,
        },
    );

    cfg.set_basic_block(exists);
    let got = map_get(loc, parent, key_val, cfg, vartab);
    cfg.add(
        vartab,
        Instr::Set {
            loc: *loc,
            res: res_no,
            expr: got,
        },
    );
    cfg.add(vartab, Instr::Branch { block: merge });

    cfg.set_basic_block(create);
    let fresh = soroban_default_handle(loc, val_ty, cfg, vartab, ns);
    cfg.add(
        vartab,
        Instr::Set {
            loc: *loc,
            res: res_no,
            expr: fresh,
        },
    );
    cfg.add(vartab, Instr::Branch { block: merge });

    cfg.set_basic_block(merge);
    cfg.set_phis(merge, vartab.pop_dirty_tracker());
    result
}
