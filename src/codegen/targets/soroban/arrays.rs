// SPDX-License-Identifier: Apache-2.0

use super::encoding::{soroban_decode_arg, soroban_storage_encode_arg};
use super::storage_path::{lower_storage_path, path_load, path_store};
use super::{load_raw_handle, soroban_host_call, soroban_vec_handle_ty};
use crate::codegen::cfg::ControlFlowGraph;
use crate::codegen::expression::expression;
use crate::codegen::interface::TargetCodegen;
use crate::codegen::vartable::Vartable;
use crate::codegen::Options;
use crate::codegen::{Expression, HostFunctions};
use crate::sema::ast;
use crate::sema::ast::{Function, Namespace, RetrieveType, Type};
use solang_parser::pt;

fn soroban_vec_push_back(
    loc: &pt::Loc,
    vec_obj: Expression,
    vec_ty: &Type,
    value: Expression,
    cfg: &mut ControlFlowGraph,
    ns: &Namespace,
    vartab: &mut Vartable,
) -> Expression {
    let value_encoded = soroban_storage_encode_arg(value, cfg, vartab, ns);
    let handle_ty = soroban_vec_handle_ty(vec_ty);
    soroban_host_call(
        loc,
        "soroban_vec_push",
        HostFunctions::VecPushBack,
        &handle_ty,
        vec![vec_obj, value_encoded],
        cfg,
        vartab,
    )
}

pub(crate) fn soroban_storage_push(
    loc: &pt::Loc,
    args: &[ast::Expression],
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
    target: &dyn TargetCodegen,
) -> Expression {
    let base = &args[0];
    let vec_ty = base.ty();

    let value = if args.len() > 1 {
        expression(&args[1], cfg, contract_no, func, ns, vartab, opt, target)
    } else {
        let elem_ty = vec_ty.storage_array_elem().deref_into();
        elem_ty.default(ns).unwrap()
    };
    let (dest, path, storage_type) =
        lower_storage_path(base, cfg, contract_no, func, ns, vartab, opt, target);
    let old_vec_obj = path_load(&path, &storage_type, cfg, vartab, ns);
    let new_vec_var = soroban_vec_push_back(loc, old_vec_obj, &vec_ty, value, cfg, ns, vartab);
    path_store(&path, new_vec_var, &storage_type, cfg, vartab, ns);
    dest
}

fn soroban_vec_pop_back(
    loc: &pt::Loc,
    vec_obj: Expression,
    vec_ty: &Type,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
) -> Expression {
    let handle_ty = soroban_vec_handle_ty(vec_ty);
    soroban_host_call(
        loc,
        "soroban_vec_pop",
        HostFunctions::VecPopBack,
        &handle_ty,
        vec![vec_obj],
        cfg,
        vartab,
    )
}

pub(crate) fn soroban_storage_pop(
    loc: &pt::Loc,
    args: &[ast::Expression],
    return_ty: &Type,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
    target: &dyn TargetCodegen,
) -> Expression {
    let base = &args[0];
    let vec_ty = base.ty();

    let (_, path, storage_type) =
        lower_storage_path(base, cfg, contract_no, func, ns, vartab, opt, target);
    let old_vec_obj = path_load(&path, &storage_type, cfg, vartab, ns);
    let new_vec_var = soroban_vec_pop_back(loc, old_vec_obj, &vec_ty, cfg, vartab);
    path_store(&path, new_vec_var, &storage_type, cfg, vartab, ns);

    Expression::Undefined {
        ty: return_ty.clone(),
    }
}

pub(crate) fn soroban_storage_array_length(
    loc: &pt::Loc,
    ty: &Type,
    array: Expression,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    ns: &Namespace,
) -> Expression {
    let vec_obj = load_raw_handle(loc, array, None, cfg, vartab);
    soroban_vec_len(loc, ty, vec_obj, cfg, vartab, ns)
}

pub(crate) fn soroban_vec_len(
    loc: &pt::Loc,
    ty: &Type,
    vec_obj: Expression,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    ns: &Namespace,
) -> Expression {
    let len_var = soroban_host_call(
        loc,
        "soroban_vec_len",
        HostFunctions::VecLen,
        &Type::Uint(64),
        vec![vec_obj],
        cfg,
        vartab,
    );
    let len_u32 = soroban_decode_arg(len_var, cfg, vartab, ns, Some(Type::Uint(32)));
    len_u32.cast(ty, ns)
}
