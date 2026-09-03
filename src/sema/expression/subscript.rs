// SPDX-License-Identifier: Apache-2.0

use crate::sema::ast::{Expression, Mapping, Namespace, RetrieveType, Type};
use crate::sema::diagnostics::Diagnostics;
use crate::sema::expression::resolve_expression::expression;
use crate::sema::expression::{ExprContext, ResolveTo};
use crate::sema::symtable::Symtable;
use solang_parser::diagnostics::Diagnostic;
use solang_parser::pt;
use solang_parser::pt::CodeLocation;

/// Resolve an array subscript expression
pub(super) fn array_subscript(
    loc: &pt::Loc,
    array: &pt::Expression,
    index: &pt::Expression,
    context: &mut ExprContext,
    ns: &mut Namespace,
    symtable: &mut Symtable,
    diagnostics: &mut Diagnostics,
) -> Result<Expression, ()> {
    let mut array = expression(
        array,
        context,
        ns,
        symtable,
        diagnostics,
        ResolveTo::Unknown,
    )?;
    let array_ty = array.ty();

    if array.ty().is_mapping() {
        return mapping_subscript(loc, array, index, context, ns, symtable, diagnostics);
    }

    let index_width_ty = if array_ty.is_contract_storage() && !array_ty.is_storage_bytes() {
        Type::Uint(256)
    } else {
        Type::Uint(32)
    };

    let mut index = expression(
        index,
        context,
        ns,
        symtable,
        diagnostics,
        ResolveTo::Type(&index_width_ty),
    )?;

    let index_ty = index.ty();

    index.check_constant_overflow(diagnostics);

    match index_ty.deref_any() {
        Type::Uint(_) => (),
        _ => {
            diagnostics.push(Diagnostic::error(
                *loc,
                format!(
                    "array subscript must be an unsigned integer, not '{}'",
                    index.ty().to_string(ns)
                ),
            ));
            return Err(());
        }
    };

    if array_ty.is_storage_bytes() {
        return Ok(Expression::Subscript {
            loc: *loc,
            ty: Type::StorageRef(false, Box::new(Type::Bytes(1))),
            array_ty,
            array: Box::new(array),
            index: Box::new(index.cast(&index.loc(), &Type::Uint(32), false, ns, diagnostics)?),
        });
    }

    // make sure we load the index value if needed
    index = index.cast(&index.loc(), index_ty.deref_any(), true, ns, diagnostics)?;

    let deref_ty = array_ty.deref_any();

    // Subscripting a zero-length bytes type is always out of bounds.
    // Reject early to avoid a u8 underflow in codegen (array_length - 1).
    if let Type::Bytes(0) = deref_ty {
        diagnostics.push(Diagnostic::error(
            *loc,
            "array subscript is out of bounds".to_string(),
        ));
        return Err(());
    }

    match deref_ty {
        Type::Bytes(_) | Type::Array(..) | Type::DynamicBytes | Type::Slice(_) => {
            if array_ty.is_contract_storage() {
                let elem_ty = array_ty.storage_array_elem();

                // When subscripting a bytes32 type array, we need to load it. It is not
                // assignable and the value is calculated by shifting in codegen
                if let Type::Bytes(_) = deref_ty {
                    array = array.cast(&array.loc(), deref_ty, true, ns, diagnostics)?;
                }

                Ok(Expression::Subscript {
                    loc: *loc,
                    ty: elem_ty,
                    array_ty,
                    array: Box::new(array),
                    index: Box::new(index),
                })
            } else {
                let elem_ty = array_ty.array_deref();

                array = array.cast(
                    &array.loc(),
                    if array_ty.deref_memory().is_fixed_reference_type(ns) {
                        &array_ty
                    } else {
                        array_ty.deref_any()
                    },
                    true,
                    ns,
                    diagnostics,
                )?;

                Ok(Expression::Subscript {
                    loc: *loc,
                    ty: elem_ty,
                    array_ty,
                    array: Box::new(array),
                    index: Box::new(index),
                })
            }
        }
        Type::String => {
            diagnostics.push(Diagnostic::error(
                array.loc(),
                "array subscript is not permitted on string".to_string(),
            ));
            Err(())
        }
        _ => {
            diagnostics.push(Diagnostic::error(
                array.loc(),
                "expression is not an array".to_string(),
            ));
            Err(())
        }
    }
}

/// Calculate storage subscript
fn mapping_subscript(
    loc: &pt::Loc,
    mapping: Expression,
    index: &pt::Expression,
    context: &mut ExprContext,
    ns: &mut Namespace,
    symtable: &mut Symtable,
    diagnostics: &mut Diagnostics,
) -> Result<Expression, ()> {
    let ty = mapping.ty();
    let elem_ty = ty.storage_array_elem();

    if let Type::Mapping(Mapping { key, .. }) = ty.deref_any() {
        let index_expr = expression(
            index,
            context,
            ns,
            symtable,
            diagnostics,
            ResolveTo::Type(key),
        )?
        .cast(&index.loc(), key, true, ns, diagnostics)?;

        Ok(Expression::Subscript {
            loc: *loc,
            ty: elem_ty,
            array_ty: ty,
            array: Box::new(mapping),
            index: Box::new(index_expr),
        })
    } else {
        unreachable!()
    }
}
