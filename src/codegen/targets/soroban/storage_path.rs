// SPDX-License-Identifier: Apache-2.0

use super::encoding::soroban_encode_arg;
use super::map::{is_true, map_del, map_get, map_get_or_default, map_has, map_put};
use super::{soroban_default_handle, soroban_field_index_val};
use crate::codegen::cfg::{ControlFlowGraph, Instr, InternalCallTy};
use crate::codegen::expression::expression;
use crate::codegen::interface::TargetCodegen;
use crate::codegen::vartable::Vartable;
use crate::codegen::Options;
use crate::codegen::{Expression, HostFunctions};
use crate::sema::ast;
use crate::sema::ast::{Function, Namespace, RetrieveType, Type};
use solang_parser::pt;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Idx {
    Field(usize),
    Array(Box<Expression>),
    Map {
        key: Box<Expression>,
        key_ty: Box<Type>,
        val_ty: Box<Type>,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct Loc {
    pub root_key: Expression,
    pub idxs: Vec<Idx>,
}

pub(crate) fn is_array_descent(array_ty: &Type) -> bool {
    match array_ty {
        Type::StorageRef(_, inner) => matches!(inner.as_ref(), Type::Array(..) | Type::Slice(_)),
        _ => false,
    }
}

pub(crate) fn is_map_descent(array_ty: &Type) -> bool {
    matches!(array_ty, Type::StorageRef(_, inner) if matches!(inner.as_ref(), Type::Mapping(_)))
}

fn map_key_val_ty(array_ty: &Type) -> (Type, Type) {
    match array_ty {
        Type::StorageRef(_, inner) => match inner.as_ref() {
            Type::Mapping(m) => ((*m.key).clone(), (*m.value).clone()),
            _ => unreachable!("map_key_val_ty: not a mapping"),
        },
        _ => unreachable!("map_key_val_ty: not a storage ref"),
    }
}

pub(crate) fn is_descent_storage_expr(e: &ast::Expression) -> bool {
    match e {
        ast::Expression::StructMember { ty, .. } => ty.is_contract_storage(),
        ast::Expression::Subscript { array_ty, .. } => {
            is_array_descent(array_ty) || is_map_descent(array_ty)
        }
        _ => false,
    }
}

pub(crate) fn root_storage_type(expr: &ast::Expression, ns: &Namespace) -> Option<pt::StorageType> {
    match expr {
        ast::Expression::StorageVariable {
            var_no,
            contract_no,
            ..
        } => ns.contracts[*contract_no]
            .variables
            .get(*var_no)
            .and_then(|v| v.storage_type.clone()),
        ast::Expression::StructMember { expr: inner, .. }
        | ast::Expression::Subscript { array: inner, .. } => root_storage_type(inner, ns),
        _ => None,
    }
}

pub(crate) fn lower_storage_lvalue(
    left: &ast::Expression,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
    target: &dyn TargetCodegen,
) -> Expression {
    match left {
        ast::Expression::StructMember {
            loc,
            ty,
            expr: var,
            field,
        } if ty.is_contract_storage() => {
            let inner = lower_storage_lvalue(var, cfg, contract_no, func, ns, vartab, opt, target);
            Expression::StructMember {
                loc: *loc,
                ty: ty.clone(),
                expr: Box::new(inner),
                member: *field,
            }
        }
        ast::Expression::Subscript {
            loc,
            ty,
            array_ty,
            array,
            index,
        } if is_array_descent(array_ty) => {
            let inner =
                lower_storage_lvalue(array, cfg, contract_no, func, ns, vartab, opt, target);
            let idx = expression(index, cfg, contract_no, func, ns, vartab, opt, target)
                .cast(&Type::Uint(64), ns);
            Expression::Subscript {
                loc: *loc,
                ty: ty.clone(),
                array_ty: array_ty.clone(),
                expr: Box::new(inner),
                index: Box::new(idx),
            }
        }
        ast::Expression::Subscript {
            loc,
            ty,
            array_ty,
            array,
            index,
        } if is_map_descent(array_ty) => {
            let inner =
                lower_storage_lvalue(array, cfg, contract_no, func, ns, vartab, opt, target);
            let key = expression(index, cfg, contract_no, func, ns, vartab, opt, target);
            Expression::Subscript {
                loc: *loc,
                ty: ty.clone(),
                array_ty: array_ty.clone(),
                expr: Box::new(inner),
                index: Box::new(key),
            }
        }
        _ => expression(left, cfg, contract_no, func, ns, vartab, opt, target),
    }
}

pub(crate) fn lower_storage_path(
    container: &ast::Expression,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
    target: &dyn TargetCodegen,
) -> (Expression, Loc, Option<pt::StorageType>) {
    let storage_type = root_storage_type(container, ns);
    let dest = lower_storage_lvalue(container, cfg, contract_no, func, ns, vartab, opt, target);
    let mut path = peel(&dest);
    hoist_indices(&mut path, cfg, vartab);
    (dest, path, storage_type)
}

pub(crate) fn peel(expr: &Expression) -> Loc {
    let mut idxs_rev: Vec<Idx> = Vec::new();
    let mut cur = expr;

    loop {
        match cur {
            Expression::StructMember {
                expr: inner,
                member,
                ty,
                ..
            } if ty.is_contract_storage() => {
                idxs_rev.push(Idx::Field(*member));
                cur = inner;
            }
            Expression::Subscript {
                expr: inner,
                index,
                array_ty,
                ..
            } if is_array_descent(array_ty) => {
                idxs_rev.push(Idx::Array(index.clone()));
                cur = inner;
            }
            Expression::Subscript {
                expr: inner,
                index,
                array_ty,
                ..
            } if is_map_descent(array_ty) => {
                let (key_ty, val_ty) = map_key_val_ty(array_ty);
                idxs_rev.push(Idx::Map {
                    key: index.clone(),
                    key_ty: Box::new(key_ty),
                    val_ty: Box::new(val_ty),
                });
                cur = inner;
            }
            _ => break,
        }
    }
    idxs_rev.reverse();
    Loc {
        root_key: cur.clone(),
        idxs: idxs_rev,
    }
}

pub(crate) fn hoist_indices(loc: &mut Loc, cfg: &mut ControlFlowGraph, vartab: &mut Vartable) {
    for idx in &mut loc.idxs {
        let Idx::Array(expr) = idx else { continue };
        if matches!(
            **expr,
            Expression::Variable { .. } | Expression::NumberLiteral { .. }
        ) {
            continue;
        }
        let ty = expr.ty();
        let var_no = vartab.temp_name("storage_idx", &ty);
        cfg.add(
            vartab,
            Instr::Set {
                loc: pt::Loc::Codegen,
                res: var_no,
                expr: (**expr).clone(),
            },
        );
        **expr = Expression::Variable {
            loc: pt::Loc::Codegen,
            ty,
            var_no,
        };
    }
}

pub(crate) fn path_load(
    loc: &Loc,
    storage_type: &Option<pt::StorageType>,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    ns: &Namespace,
) -> Expression {
    let ploc = pt::Loc::Codegen;
    let mut handle = load_root(&ploc, loc.root_key.clone(), storage_type, cfg, vartab);
    for idx in &loc.idxs {
        let idx_val = encode_index(&ploc, idx, cfg, vartab, ns);
        handle = vec_get(&ploc, handle, idx_val, cfg, vartab);
    }
    handle
}

fn load_root(
    loc: &pt::Loc,
    root_key: Expression,
    storage_type: &Option<pt::StorageType>,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
) -> Expression {
    let handle_no = vartab.temp_name("storage_handle", &Type::Uint(64));
    cfg.add(
        vartab,
        Instr::LoadStorage {
            res: handle_no,
            ty: Type::Uint(64),
            storage: root_key,
            storage_type: storage_type.clone(),
        },
    );
    Expression::Variable {
        loc: *loc,
        ty: Type::Uint(64),
        var_no: handle_no,
    }
}

fn encode_index(
    loc: &pt::Loc,
    idx: &Idx,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    ns: &Namespace,
) -> Expression {
    match idx {
        Idx::Field(field_no) => soroban_field_index_val(loc, *field_no, cfg, vartab, ns),
        Idx::Array(index) => {
            soroban_encode_arg((**index).clone().cast(&Type::Uint(32), ns), cfg, vartab, ns)
        }
        Idx::Map { key, key_ty, .. } => {
            soroban_encode_arg((**key).clone().cast(key_ty, ns), cfg, vartab, ns)
        }
    }
}

fn vec_get(
    loc: &pt::Loc,
    handle: Expression,
    idx_val: Expression,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
) -> Expression {
    let elem_no = vartab.temp_name("path_vec_get", &Type::Uint(64));
    cfg.add(
        vartab,
        Instr::Call {
            res: vec![elem_no],
            return_tys: vec![Type::Uint(64)],
            call: InternalCallTy::HostFunction {
                name: HostFunctions::VecGet.name().to_string(),
            },
            args: vec![handle, idx_val],
        },
    );
    Expression::Variable {
        loc: *loc,
        ty: Type::Uint(64),
        var_no: elem_no,
    }
}

fn vec_put(
    loc: &pt::Loc,
    handle: Expression,
    idx_val: Expression,
    value: Expression,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
) -> Expression {
    let new_no = vartab.temp_name("path_vec_put", &Type::Uint(64));
    cfg.add(
        vartab,
        Instr::Call {
            res: vec![new_no],
            return_tys: vec![Type::Uint(64)],
            call: InternalCallTy::HostFunction {
                name: HostFunctions::VecPut.name().to_string(),
            },
            args: vec![handle, idx_val, value],
        },
    );
    Expression::Variable {
        loc: *loc,
        ty: Type::Uint(64),
        var_no: new_no,
    }
}

pub(crate) fn path_load_map(
    loc: &Loc,
    value_ty: &Type,
    storage_type: &Option<pt::StorageType>,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    ns: &Namespace,
) -> Expression {
    let ploc = pt::Loc::Codegen;
    let res_no = vartab.temp_name("map_read", &Type::Uint(64));
    let result = Expression::Variable {
        loc: ploc,
        ty: Type::Uint(64),
        var_no: res_no,
    };

    vartab.new_dirty_tracker();
    let absent = cfg.new_basic_block("map_read_absent".to_string());
    let merge = cfg.new_basic_block("map_read_merge".to_string());

    let mut cur = load_root(&ploc, loc.root_key.clone(), storage_type, cfg, vartab);
    for idx in &loc.idxs {
        let idx_val = encode_index(&ploc, idx, cfg, vartab, ns);
        match idx {
            Idx::Map { .. } => {
                let has = map_has(&ploc, cur.clone(), idx_val.clone(), cfg, vartab);
                let hit = cfg.new_basic_block("map_read_hit".to_string());
                cfg.add(
                    vartab,
                    Instr::BranchCond {
                        cond: is_true(has),
                        true_block: hit,
                        false_block: absent,
                    },
                );
                cfg.set_basic_block(hit);
                cur = map_get(&ploc, cur, idx_val, cfg, vartab);
            }
            Idx::Field(_) | Idx::Array(_) => {
                cur = vec_get(&ploc, cur, idx_val, cfg, vartab);
            }
        }
    }
    cfg.add(
        vartab,
        Instr::Set {
            loc: ploc,
            res: res_no,
            expr: cur,
        },
    );
    cfg.add(vartab, Instr::Branch { block: merge });

    cfg.set_basic_block(absent);
    let def = soroban_default_handle(&ploc, value_ty, cfg, vartab, ns);
    cfg.add(
        vartab,
        Instr::Set {
            loc: ploc,
            res: res_no,
            expr: def,
        },
    );
    cfg.add(vartab, Instr::Branch { block: merge });

    cfg.set_basic_block(merge);
    cfg.set_phis(merge, vartab.pop_dirty_tracker());
    result
}

pub(crate) fn path_store(
    loc: &Loc,
    value: Expression,
    storage_type: &Option<pt::StorageType>,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    ns: &Namespace,
) {
    let ploc = pt::Loc::Codegen;
    let n = loc.idxs.len();

    let mut new_root = value;
    if n > 0 {
        let encoded: Vec<Expression> = loc
            .idxs
            .iter()
            .map(|idx| encode_index(&ploc, idx, cfg, vartab, ns))
            .collect();

        let mut handles = Vec::with_capacity(n);
        handles.push(load_root(
            &ploc,
            loc.root_key.clone(),
            storage_type,
            cfg,
            vartab,
        ));
        for k in 1..n {
            let parent = handles[k - 1].clone();
            let addr = encoded[k - 1].clone();
            let h = match &loc.idxs[k - 1] {
                Idx::Map { val_ty, .. } => {
                    map_get_or_default(&ploc, parent, addr, val_ty, cfg, vartab, ns)
                }
                Idx::Field(_) | Idx::Array(_) => vec_get(&ploc, parent, addr, cfg, vartab),
            };
            handles.push(h);
        }

        for k in (0..n).rev() {
            let parent = handles[k].clone();
            let addr = encoded[k].clone();
            new_root = match &loc.idxs[k] {
                Idx::Map { .. } => map_put(&ploc, parent, addr, new_root, cfg, vartab),
                Idx::Field(_) | Idx::Array(_) => {
                    vec_put(&ploc, parent, addr, new_root, cfg, vartab)
                }
            };
        }
    }

    cfg.add(
        vartab,
        Instr::SetStorage {
            ty: Type::Uint(64),
            value: new_root,
            storage: loc.root_key.clone(),
            storage_type: storage_type.clone(),
        },
    );
}

pub(crate) fn path_delete_map(
    loc: &Loc,
    storage_type: &Option<pt::StorageType>,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    ns: &Namespace,
) {
    let ploc = pt::Loc::Codegen;
    let n = loc.idxs.len();
    debug_assert!(n >= 1, "path_delete_map requires at least one index");

    let encoded: Vec<Expression> = loc
        .idxs
        .iter()
        .map(|idx| encode_index(&ploc, idx, cfg, vartab, ns))
        .collect();

    let mut handles = Vec::with_capacity(n);
    handles.push(load_root(
        &ploc,
        loc.root_key.clone(),
        storage_type,
        cfg,
        vartab,
    ));
    for k in 1..n {
        let parent = handles[k - 1].clone();
        let addr = encoded[k - 1].clone();
        let h = match &loc.idxs[k - 1] {
            Idx::Map { val_ty, .. } => {
                map_get_or_default(&ploc, parent, addr, val_ty, cfg, vartab, ns)
            }
            Idx::Field(_) | Idx::Array(_) => vec_get(&ploc, parent, addr, cfg, vartab),
        };
        handles.push(h);
    }

    let has = map_has(
        &ploc,
        handles[n - 1].clone(),
        encoded[n - 1].clone(),
        cfg,
        vartab,
    );

    vartab.new_dirty_tracker();
    let present = cfg.new_basic_block("map_del_present".to_string());
    let done = cfg.new_basic_block("map_del_done".to_string());

    cfg.add(
        vartab,
        Instr::BranchCond {
            cond: is_true(has),
            true_block: present,
            false_block: done,
        },
    );

    cfg.set_basic_block(present);
    let mut new_root = map_del(
        &ploc,
        handles[n - 1].clone(),
        encoded[n - 1].clone(),
        cfg,
        vartab,
    );

    for k in (0..n - 1).rev() {
        let parent = handles[k].clone();
        let addr = encoded[k].clone();
        new_root = match &loc.idxs[k] {
            Idx::Map { .. } => map_put(&ploc, parent, addr, new_root, cfg, vartab),
            Idx::Field(_) | Idx::Array(_) => vec_put(&ploc, parent, addr, new_root, cfg, vartab),
        };
    }

    cfg.add(
        vartab,
        Instr::SetStorage {
            ty: Type::Uint(64),
            value: new_root,
            storage: loc.root_key.clone(),
            storage_type: storage_type.clone(),
        },
    );
    cfg.add(vartab, Instr::Branch { block: done });

    cfg.set_basic_block(done);
    cfg.set_phis(done, vartab.pop_dirty_tracker());
}
