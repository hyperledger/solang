// SPDX-License-Identifier: Apache-2.0

use super::encoding::{soroban_decode_arg, soroban_encode_arg};
use super::storage_path::{is_descent_storage_expr, lower_storage_path, path_load, path_store};
use super::{load_raw_handle, soroban_host_call};
use crate::codegen::cfg::ControlFlowGraph;
use crate::codegen::expression::expression;
use crate::codegen::interface::TargetCodegen;
use crate::codegen::vartable::Vartable;
use crate::codegen::Options;
use crate::codegen::{Expression, HostFunctions};
use crate::sema::ast;
use crate::sema::ast::{Function, Namespace, Type};
use solang_parser::helpers::CodeLocation;
use solang_parser::pt;

fn encode_byte(
    value: Expression,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    ns: &Namespace,
) -> Expression {
    let byte_u32 = value.cast(&Type::Uint(8), ns).cast(&Type::Uint(32), ns);
    soroban_encode_arg(byte_u32, cfg, vartab, ns)
}

pub(crate) fn soroban_bytes_push(
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
    let value = expression(&args[1], cfg, contract_no, func, ns, vartab, opt, target);
    let byte_val = encode_byte(value, cfg, vartab, ns);

    let (dest, path, storage_type) =
        lower_storage_path(&args[0], cfg, contract_no, func, ns, vartab, opt, target);
    let handle = path_load(&path, &storage_type, cfg, vartab, ns);
    let new_handle = soroban_host_call(
        loc,
        "bytes_push",
        HostFunctions::BytesPush,
        &Type::Uint(64),
        vec![handle, byte_val],
        cfg,
        vartab,
    );
    path_store(&path, new_handle, &storage_type, cfg, vartab, ns);
    dest
}

pub(crate) fn soroban_bytes_pop(
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
    let (_, path, storage_type) =
        lower_storage_path(&args[0], cfg, contract_no, func, ns, vartab, opt, target);
    let handle = path_load(&path, &storage_type, cfg, vartab, ns);
    let new_handle = soroban_host_call(
        loc,
        "bytes_pop",
        HostFunctions::BytesPop,
        &Type::Uint(64),
        vec![handle],
        cfg,
        vartab,
    );
    path_store(&path, new_handle, &storage_type, cfg, vartab, ns);
    Expression::Undefined {
        ty: return_ty.clone(),
    }
}

pub(crate) fn soroban_bytes_subscript_read(
    container: &ast::Expression,
    index: &ast::Expression,
    elem_ty: &Type,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
    target: &dyn TargetCodegen,
) -> Option<Expression> {
    if !is_descent_storage_expr(container) {
        return None;
    }
    let (_, path, storage_type) =
        lower_storage_path(container, cfg, contract_no, func, ns, vartab, opt, target);
    let handle = path_load(&path, &storage_type, cfg, vartab, ns);

    let idx = expression(index, cfg, contract_no, func, ns, vartab, opt, target);
    let idx_val = soroban_encode_arg(idx.cast(&Type::Uint(32), ns), cfg, vartab, ns);
    let raw = soroban_host_call(
        &container.loc(),
        "bytes_get",
        HostFunctions::BytesGet,
        &Type::Uint(64),
        vec![handle, idx_val],
        cfg,
        vartab,
    );

    let byte_u32 = soroban_decode_arg(raw, cfg, vartab, ns, Some(Type::Uint(32)));
    Some(Expression::Trunc {
        loc: container.loc(),
        ty: elem_ty.deref_any().clone(),
        expr: Box::new(byte_u32),
    })
}

pub(crate) fn soroban_storage_bytes_subscript_write(
    container: &ast::Expression,
    index: &ast::Expression,
    value: Expression,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
    target: &dyn TargetCodegen,
) {
    let (_, path, storage_type) =
        lower_storage_path(container, cfg, contract_no, func, ns, vartab, opt, target);
    let handle = path_load(&path, &storage_type, cfg, vartab, ns);

    let idx = expression(index, cfg, contract_no, func, ns, vartab, opt, target);
    let idx_val = soroban_encode_arg(idx.cast(&Type::Uint(32), ns), cfg, vartab, ns);
    let byte_val = encode_byte(value, cfg, vartab, ns);
    let new_handle = soroban_host_call(
        &container.loc(),
        "bytes_put",
        HostFunctions::BytesPut,
        &Type::Uint(64),
        vec![handle, idx_val, byte_val],
        cfg,
        vartab,
    );
    path_store(&path, new_handle, &storage_type, cfg, vartab, ns);
}

pub(crate) fn soroban_bytes_length(
    loc: &pt::Loc,
    bytes_var: Expression,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    ns: &Namespace,
) -> Expression {
    let handle = load_raw_handle(loc, bytes_var, None, cfg, vartab);
    soroban_obj_length(loc, handle, HostFunctions::BytesLen, cfg, vartab, ns)
}

pub(crate) fn soroban_strings_length(
    loc: &pt::Loc,
    string_var: Expression,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    ns: &Namespace,
) -> Expression {
    let handle = load_raw_handle(loc, string_var, None, cfg, vartab);
    soroban_obj_length(loc, handle, HostFunctions::StringLen, cfg, vartab, ns)
}

pub(crate) fn soroban_obj_length(
    loc: &pt::Loc,
    handle: Expression,
    len_fn: HostFunctions,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    ns: &Namespace,
) -> Expression {
    let len = soroban_host_call(
        loc,
        "obj_length",
        len_fn,
        &Type::Uint(64),
        vec![handle],
        cfg,
        vartab,
    );
    soroban_decode_arg(len, cfg, vartab, ns, Some(Type::Uint(32)))
}

pub(crate) fn soroban_bytes_new(
    loc: &pt::Loc,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
) -> Expression {
    let ty = Type::SorobanHandle(Box::new(Type::DynamicBytes));
    soroban_host_call(
        loc,
        "bytes_obj_new",
        HostFunctions::BytesNew,
        &ty,
        vec![],
        cfg,
        vartab,
    )
}
