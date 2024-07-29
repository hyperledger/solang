// SPDX-License-Identifier: Apache-2.0

use crate::sema::address::to_hexstr_eip55;
use crate::sema::ast::{ArrayLength, Expression, Namespace, RetrieveType, StructType, Type};
use crate::sema::diagnostics::Diagnostics;
use crate::sema::expression::integers::bigint_to_expression;
use crate::sema::expression::resolve_expression::expression;
use crate::sema::expression::strings::unescape;
use crate::sema::expression::{ExprContext, ResolveTo};
use crate::sema::symtable::Symtable;
use crate::sema::unused_variable::used_variable;
use crate::Target;
use base58::{FromBase58, FromBase58Error};
use num_bigint::{BigInt, Sign};
use num_rational::BigRational;
use num_traits::{FromPrimitive, Num, Zero};
use solang_parser::diagnostics::Diagnostic;
use solang_parser::pt;
use solang_parser::pt::{CodeLocation, Loc};
use std::ops::Mul;
use std::str::FromStr;

pub(super) fn string_literal(
    v: &[pt::StringLiteral],
    file_no: usize,
    diagnostics: &mut Diagnostics,
    resolve_to: ResolveTo,
) -> Expression {


    println!("string_literal: {:?}", v);
    println!("resolve_to: {:?}", resolve_to);


    // Concatenate the strings
    let mut result = Vec::new();
    let mut loc = v[0].loc;

    for s in v {
        result.append(&mut unescape(&s.string, s.loc.start(), file_no, diagnostics).1);
        loc.use_end_from(&s.loc);
    }

    let length = result.len();

   let ret =  match resolve_to {
        ResolveTo::Type(Type::String) => Expression::AllocDynamicBytes {
            loc,
            ty: Type::String,
            length: Box::new(Expression::NumberLiteral {
                loc,
                ty: Type::Uint(32),
                value: BigInt::from(length),
            }),
            init: Some(result),
        },
        ResolveTo::Type(Type::Slice(ty)) if ty.as_ref() == &Type::Bytes(1) => {
            Expression::AllocDynamicBytes {
                loc,
                ty: Type::Slice(ty.clone()),
                length: Box::new(Expression::NumberLiteral {
                    loc,
                    ty: Type::Uint(32),
                    value: BigInt::from(length),
                }),
                init: Some(result),
            }
        }
        _ => Expression::BytesLiteral {
            loc,
            ty: Type::Bytes(length as u8),
            value: result,
        },
    };

    println!("ret: {:?}", ret);
    ret
}

pub(super) fn hex_literal(
    v: &[pt::HexLiteral],
    diagnostics: &mut Diagnostics,
    resolve_to: ResolveTo,
) -> Result<Expression, ()> {
    let mut result = Vec::new();
    let mut loc = v[0].loc;

    for s in v {
        if (s.hex.len() % 2) != 0 {
            diagnostics.push(Diagnostic::error(
                s.loc,
                format!("hex string \"{}\" has odd number of characters", s.hex),
            ));
            return Err(());
        } else {
            result.extend_from_slice(&hex::decode(&s.hex).unwrap());
            loc.use_end_from(&s.loc);
        }
    }

    let length = result.len();

    match resolve_to {
        ResolveTo::Type(Type::Slice(ty)) if ty.as_ref() == &Type::Bytes(1) => {
            Ok(Expression::AllocDynamicBytes {
                loc,
                ty: Type::Slice(ty.clone()),
                length: Box::new(Expression::NumberLiteral {
                    loc,
                    ty: Type::Uint(32),
                    value: BigInt::from(length),
                }),
                init: Some(result),
            })
        }
        ResolveTo::Type(Type::DynamicBytes) => Ok(Expression::AllocDynamicBytes {
            loc,
            ty: Type::DynamicBytes,
            length: Box::new(Expression::NumberLiteral {
                loc,
                ty: Type::Uint(32),
                value: BigInt::from(length),
            }),
            init: Some(result),
        }),
        _ => Ok(Expression::BytesLiteral {
            loc,
            ty: Type::Bytes(length as u8),
            value: result,
        }),
    }
}

pub(crate) fn hex_number_literal(
    loc: &pt::Loc,
    n: &str,
    ns: &Namespace,
    diagnostics: &mut Diagnostics,
    resolve_to: ResolveTo,
) -> Result<Expression, ()> {
    // ns.address_length is in bytes; double for hex and two for the leading 0x
    if n.starts_with("0x") && !n.chars().any(|c| c == '_') && n.len() == 42 {
        let address = to_hexstr_eip55(n);

        if ns.target == Target::EVM {
            return if address == *n {
                let s: String = address.chars().skip(2).collect();

                Ok(Expression::NumberLiteral {
                    loc: *loc,
                    ty: Type::Address(false),
                    value: BigInt::from_str_radix(&s, 16).unwrap(),
                })
            } else {
                diagnostics.push(Diagnostic::error(
                    *loc,
                    format!("address literal has incorrect checksum, expected '{address}'"),
                ));
                Err(())
            };
        } else if address == *n {
            // looks like ethereum address
            diagnostics.push(Diagnostic::error(
                *loc,
                format!(
                    "ethereum address literal '{}' not supported on target {}",
                    n, ns.target
                ),
            ));
            return Err(());
        }
    }

    // from_str_radix does not like the 0x prefix
    let s: String = n.chars().skip(2).filter(|v| *v != '_').collect();

    // hex values are allowed for bytesN but the length must match
    if let ResolveTo::Type(Type::Bytes(length)) = resolve_to {
        let expected_length = *length as usize * 2;
        let val = BigInt::from_str_radix(&s, 16).unwrap();

        return if !val.is_zero() && s.len() != expected_length {
            diagnostics.push(Diagnostic::cast_error(
                *loc,
                format!(
                    "hex literal {n} must be {expected_length} digits for type 'bytes{length}'"
                ),
            ));
            Err(())
        } else {
            Ok(Expression::NumberLiteral {
                loc: *loc,
                ty: Type::Bytes(*length),
                value: val,
            })
        };
    }

    bigint_to_expression(
        loc,
        &BigInt::from_str_radix(&s, 16).unwrap(),
        ns,
        diagnostics,
        resolve_to,
        Some(s.len()),
    )
}

pub(super) fn address_literal(
    loc: &pt::Loc,
    address: &str,
    ns: &Namespace,
    diagnostics: &mut Diagnostics,
) -> Result<Expression, ()> {
    if ns.target.is_polkadot() {
        match address.from_base58() {
            Ok(v) => {
                if v.len() != ns.address_length + 3 {
                    diagnostics.push(Diagnostic::error(
                        *loc,
                        format!(
                            "address literal {} incorrect length of {}",
                            address,
                            v.len()
                        ),
                    ));
                    return Err(());
                }

                let hash_data: Vec<u8> = b"SS58PRE"
                    .iter()
                    .chain(v[..=ns.address_length].iter())
                    .cloned()
                    .collect();

                let hash = blake2_rfc::blake2b::blake2b(64, &[], &hash_data);
                let hash = hash.as_bytes();

                if v[ns.address_length + 1] != hash[0] || v[ns.address_length + 2] != hash[1] {
                    diagnostics.push(Diagnostic::error(
                        *loc,
                        format!("address literal {address} hash incorrect checksum"),
                    ));
                    return Err(());
                }

                Ok(Expression::NumberLiteral {
                    loc: *loc,
                    ty: Type::Address(false),
                    value: BigInt::from_bytes_be(Sign::Plus, &v[1..ns.address_length + 1]),
                })
            }
            Err(FromBase58Error::InvalidBase58Length) => {
                diagnostics.push(Diagnostic::error(
                    *loc,
                    format!("address literal {address} invalid base58 length"),
                ));
                Err(())
            }
            Err(FromBase58Error::InvalidBase58Character(ch, pos)) => {
                let mut loc = *loc;
                if let pt::Loc::File(_, start, end) = &mut loc {
                    *start += pos;
                    *end = *start;
                }
                diagnostics.push(Diagnostic::error(
                    loc,
                    format!("address literal {address} invalid character '{ch}'"),
                ));
                Err(())
            }
        }
    } else if ns.target == Target::Solana {
        match address.from_base58() {
            Ok(v) => {
                if v.len() != ns.address_length {
                    diagnostics.push(Diagnostic::error(
                        *loc,
                        format!(
                            "address literal {} incorrect length of {}",
                            address,
                            v.len()
                        ),
                    ));
                    Err(())
                } else {
                    Ok(Expression::NumberLiteral {
                        loc: *loc,
                        ty: Type::Address(false),
                        value: BigInt::from_bytes_be(Sign::Plus, &v),
                    })
                }
            }
            Err(FromBase58Error::InvalidBase58Length) => {
                diagnostics.push(Diagnostic::error(
                    *loc,
                    format!("address literal {address} invalid base58 length"),
                ));
                Err(())
            }
            Err(FromBase58Error::InvalidBase58Character(ch, pos)) => {
                let mut loc = *loc;
                if let pt::Loc::File(_, start, end) = &mut loc {
                    *start += pos;
                    *end = *start;
                }
                diagnostics.push(Diagnostic::error(
                    loc,
                    format!("address literal {address} invalid character '{ch}'"),
                ));
                Err(())
            }
        }
    } else {
        diagnostics.push(Diagnostic::error(
            *loc,
            format!("address literal {} not supported on {}", address, ns.target),
        ));
        Err(())
    }
}

/// Resolve the given number literal, multiplied by value of unit
pub(crate) fn number_literal(
    loc: &pt::Loc,
    integer: &str,
    exp: &str,
    ns: &Namespace,
    unit: &BigInt,
    diagnostics: &mut Diagnostics,
    resolve_to: ResolveTo,
) -> Result<Expression, ()> {
    if integer.starts_with('0') && integer.len() > 1 {
        diagnostics.push(Diagnostic::error(
            *loc,
            "leading zeros not permitted, can be confused with octal".into(),
        ));
        return Err(());
    }

    let integer = BigInt::from_str(integer).unwrap();

    let n = if exp.is_empty() {
        integer
    } else {
        let base10 = BigInt::from_str("10").unwrap();

        if let Some(abs_exp) = exp.strip_prefix('-') {
            if let Ok(exp) = u8::from_str(abs_exp) {
                let res = BigRational::new(integer, base10.pow(exp.into()));

                if res.is_integer() {
                    res.to_integer()
                } else {
                    return Ok(Expression::RationalNumberLiteral {
                        loc: *loc,
                        ty: Type::Rational,
                        value: res,
                    });
                }
            } else {
                diagnostics.push(Diagnostic::error(
                    *loc,
                    format!("exponent '{exp}' too large"),
                ));
                return Err(());
            }
        } else if let Ok(exp) = u8::from_str(exp) {
            integer.mul(base10.pow(exp.into()))
        } else {
            diagnostics.push(Diagnostic::error(
                *loc,
                format!("exponent '{exp}' too large"),
            ));
            return Err(());
        }
    };

    bigint_to_expression(loc, &n.mul(unit), ns, diagnostics, resolve_to, None)
}

/// Resolve the given rational number literal, multiplied by value of unit
pub(super) fn rational_number_literal(
    loc: &pt::Loc,
    integer: &str,
    fraction: &str,
    exp: &str,
    unit: &BigInt,
    ns: &Namespace,
    diagnostics: &mut Diagnostics,
    resolve_to: ResolveTo,
) -> Result<Expression, ()> {
    if integer.starts_with("-0") && integer.len() > 2
        || integer.starts_with('0') && integer.len() > 1
    {
        diagnostics.push(Diagnostic::error(
            *loc,
            "leading zeros not permitted, can be confused with octal".into(),
        ));
        return Err(());
    }

    let mut integer = integer.to_owned();
    let len = fraction.len() as u32;
    let exp_negative = exp.starts_with('-');

    let denominator = BigInt::from_str("10").unwrap().pow(len);
    let zero_index = fraction
        .chars()
        .position(|c| c != '0')
        .unwrap_or(usize::MAX);
    let n = if exp.is_empty() {
        if integer.is_empty() || integer == "0" {
            if zero_index < usize::MAX {
                BigRational::new(
                    BigInt::from_str(&fraction[zero_index..]).unwrap(),
                    denominator,
                )
            } else {
                BigRational::from(BigInt::zero())
            }
        } else {
            integer.push_str(fraction);
            BigRational::new(BigInt::from_str(&integer).unwrap(), denominator)
        }
    } else {
        let exp = if let Ok(exp) = u8::from_str(if exp_negative { &exp[1..] } else { exp }) {
            exp
        } else {
            diagnostics.push(Diagnostic::error(
                *loc,
                format!("exponent '{exp}' too large"),
            ));
            return Err(());
        };
        let exp_result = BigInt::from_str("10").unwrap().pow(exp.into());

        if integer.is_empty() || integer == "0" {
            if zero_index < usize::MAX {
                if exp_negative {
                    BigRational::new(
                        BigInt::from_str(&fraction[zero_index..]).unwrap(),
                        denominator.mul(exp_result),
                    )
                } else {
                    BigRational::new(
                        BigInt::from_str(&fraction[zero_index..])
                            .unwrap()
                            .mul(exp_result),
                        denominator,
                    )
                }
            } else {
                BigRational::from(BigInt::zero())
            }
        } else {
            integer.push_str(fraction);
            if exp_negative {
                BigRational::new(
                    BigInt::from_str(&integer).unwrap(),
                    denominator.mul(exp_result),
                )
            } else {
                BigRational::new(
                    BigInt::from_str(&integer).unwrap().mul(exp_result),
                    denominator,
                )
            }
        }
    };

    let res = n.mul(unit);

    if res.is_integer() {
        bigint_to_expression(loc, &res.to_integer(), ns, diagnostics, resolve_to, None)
    } else {
        Ok(Expression::RationalNumberLiteral {
            loc: *loc,
            ty: Type::Rational,
            value: res,
        })
    }
}

/// Resolve a function call with positional arguments
pub(super) fn struct_literal(
    loc: &pt::Loc,
    id: pt::IdentifierPath,
    struct_ty: &StructType,
    args: &[pt::Expression],
    context: &mut ExprContext,
    ns: &mut Namespace,
    symtable: &mut Symtable,
    diagnostics: &mut Diagnostics,
) -> Result<Expression, ()> {
    let struct_def = struct_ty.definition(ns).clone();

    let ty = Type::Struct(*struct_ty);

    if ty.contains_builtins(ns, &StructType::AccountInfo).is_some() {
        diagnostics.push(Diagnostic::error(
            *loc,
            format!(
                "builtin struct '{}' cannot be created using struct literal",
                struct_def.id,
            ),
        ));
        Err(())
    } else if args.len() != struct_def.fields.len() {
        diagnostics.push(Diagnostic::error(
            *loc,
            format!(
                "struct '{}' has {} fields, not {}",
                struct_def.id,
                struct_def.fields.len(),
                args.len()
            ),
        ));
        Err(())
    } else {
        let mut fields = Vec::new();

        for (i, a) in args.iter().enumerate() {
            let expr = expression(
                a,
                context,
                ns,
                symtable,
                diagnostics,
                ResolveTo::Type(&struct_def.fields[i].ty),
            )?;
            used_variable(ns, &expr, symtable);
            fields.push((
                None,
                expr.cast(loc, &struct_def.fields[i].ty, true, ns, diagnostics)?,
            ));
        }

        Ok(Expression::StructLiteral {
            loc: *loc,
            id,
            ty,
            values: fields,
        })
    }
}

pub(crate) fn unit_literal(
    loc: &pt::Loc,
    unit: &Option<pt::Identifier>,
    ns: &Namespace,
    diagnostics: &mut Diagnostics,
) -> BigInt {
    if let Some(unit) = unit {
        match unit.name.as_str() {
            "wei" | "gwei" | "ether" if ns.target != crate::Target::EVM => {
                diagnostics.push(Diagnostic::warning(
                    *loc,
                    format!("ethereum currency unit used while targeting {}", ns.target),
                ));
            }
            "sol" | "lamports" if ns.target != crate::Target::Solana => {
                diagnostics.push(Diagnostic::warning(
                    *loc,
                    format!("solana currency unit used while targeting {}", ns.target),
                ));
            }
            _ => (),
        }

        match unit.name.as_str() {
            "seconds" => BigInt::from(1),
            "minutes" => BigInt::from(60),
            "hours" => BigInt::from(60 * 60),
            "days" => BigInt::from(60 * 60 * 24),
            "weeks" => BigInt::from(60 * 60 * 24 * 7),
            "wei" => BigInt::from(1),
            "gwei" => BigInt::from(10).pow(9u32),
            "ether" => BigInt::from(10).pow(18u32),
            "sol" => BigInt::from(10).pow(9u32),
            "lamports" => BigInt::from(1),
            _ => {
                diagnostics.push(Diagnostic::error(
                    *loc,
                    format!("unknown unit '{}'", unit.name),
                ));
                BigInt::from(1)
            }
        }
    } else {
        BigInt::from(1)
    }
}

/// Resolve a struct literal with named fields
pub(super) fn named_struct_literal(
    loc: &pt::Loc,
    id: pt::IdentifierPath,
    str_ty: &StructType,
    args: &[pt::NamedArgument],
    context: &mut ExprContext,
    ns: &mut Namespace,
    symtable: &mut Symtable,
    diagnostics: &mut Diagnostics,
) -> Result<Expression, ()> {
    let struct_def = str_ty.definition(ns).clone();
    let ty = Type::Struct(*str_ty);

    if ty.contains_builtins(ns, &StructType::AccountInfo).is_some() {
        diagnostics.push(Diagnostic::error(
            *loc,
            format!(
                "builtin struct '{}' cannot be created using struct literal",
                struct_def.id,
            ),
        ));
        Err(())
    } else if args.len() != struct_def.fields.len() {
        diagnostics.push(Diagnostic::error(
            *loc,
            format!(
                "struct '{}' has {} fields, not {}",
                struct_def.id,
                struct_def.fields.len(),
                args.len()
            ),
        ));
        Err(())
    } else {
        let mut fields = Vec::new();
        fields.resize(
            args.len(),
            (
                None,
                Expression::BoolLiteral {
                    loc: Loc::Implicit,
                    value: false,
                },
            ),
        );
        for a in args {
            match struct_def.fields.iter().enumerate().find(|(_, f)| {
                f.id.as_ref().map(|id| id.name.as_str()) == Some(a.name.name.as_str())
            }) {
                Some((i, f)) => {
                    let expr = expression(
                        &a.expr,
                        context,
                        ns,
                        symtable,
                        diagnostics,
                        ResolveTo::Type(&f.ty),
                    )?;
                    used_variable(ns, &expr, symtable);
                    fields[i] = (
                        Some(a.name.clone()),
                        expr.cast(loc, &f.ty, true, ns, diagnostics)?,
                    );
                }
                None => {
                    diagnostics.push(Diagnostic::error(
                        a.name.loc,
                        format!("struct '{}' has no field '{}'", struct_def.id, a.name.name,),
                    ));
                    return Err(());
                }
            }
        }
        Ok(Expression::StructLiteral {
            loc: *loc,
            id,
            ty,
            values: fields,
        })
    }
}

/// Given an parsed literal array, ensure that it is valid. All the elements in the array
/// must of the same type. The array might be a multidimensional array; all the leaf nodes
/// must match.
pub(super) fn array_literal(
    loc: &pt::Loc,
    exprs: &[pt::Expression],
    context: &mut ExprContext,
    ns: &mut Namespace,
    symtable: &mut Symtable,
    diagnostics: &mut Diagnostics,
    resolve_to: ResolveTo,
) -> Result<Expression, ()> {
    let mut dimensions = Vec::new();
    let mut flattened = Vec::new();

    let resolve_to = match resolve_to {
        ResolveTo::Type(Type::Array(elem_ty, _)) => ResolveTo::Type(elem_ty),
        // Solana seeds are a slice of slice of bytes, e.g. [ [ "fo", "o" ], [ "b", "a", "r"]]. In this
        // case we want to resolve
        ResolveTo::Type(Type::Slice(slice)) if matches!(slice.as_ref(), Type::Slice(_)) => {
            let mut res = Vec::new();
            let mut has_errors = false;

            for expr in exprs {
                let Type::Slice(elem) = slice.as_ref() else {
                    unreachable!()
                };

                let Ok(expr) = expression(
                    expr,
                    context,
                    ns,
                    symtable,
                    diagnostics,
                    ResolveTo::Type(&Type::Array(elem.clone(), vec![ArrayLength::Dynamic])),
                ) else {
                    has_errors = true;
                    continue;
                };

                let ty = expr.ty();

                let deref_ty = ty.deref_any();

                let Ok(expr) = expr.cast(&expr.loc(), deref_ty, true, ns, diagnostics) else {
                    has_errors = true;
                    continue;
                };

                if let Type::Array(elem, _) = deref_ty {
                    if expr
                        .cast(loc, slice, true, ns, &mut Diagnostics::default())
                        .is_err()
                    {
                        diagnostics.push(Diagnostic::error(
                            expr.loc(),
                            format!(
                                "type {} found where array {} expected",
                                elem.to_string(ns),
                                slice.to_string(ns)
                            ),
                        ));
                        has_errors = true;
                    }
                    used_variable(ns, &expr, symtable);
                } else {
                    diagnostics.push(Diagnostic::error(
                        expr.loc(),
                        format!("type {} found where array expected", ty.to_string(ns)),
                    ));
                    has_errors = true;
                }

                res.push(expr);
            }

            return if has_errors {
                Err(())
            } else {
                let aty = Type::Array(
                    slice.clone(),
                    vec![ArrayLength::Fixed(BigInt::from(exprs.len()))],
                );

                Ok(Expression::ArrayLiteral {
                    loc: *loc,
                    ty: aty,
                    dimensions: vec![exprs.len() as u32],
                    values: res,
                })
            };
        }
        _ => resolve_to,
    };

    check_subarrays(
        exprs,
        &mut Some(&mut dimensions),
        &mut flattened,
        diagnostics,
    )?;

    if flattened.is_empty() {
        diagnostics.push(Diagnostic::error(
            *loc,
            "array requires at least one element".to_string(),
        ));
        return Err(());
    }

    let mut flattened = flattened.iter();

    // We follow the solidity scheme were everthing gets implicitly converted to the
    // type of the first element
    let mut first = expression(
        flattened.next().unwrap(),
        context,
        ns,
        symtable,
        diagnostics,
        resolve_to,
    )?;

    let ty = if let ResolveTo::Type(ty) = resolve_to {
        first = first.cast(&first.loc(), ty, true, ns, diagnostics)?;

        ty.clone()
    } else {
        first.ty()
    };
    used_variable(ns, &first, symtable);
    let mut exprs = vec![first];

    for e in flattened {
        let mut other = expression(e, context, ns, symtable, diagnostics, resolve_to)?;
        used_variable(ns, &other, symtable);

        if resolve_to != ResolveTo::Unknown && other.ty() != ty {
            other = other.cast(&e.loc(), &ty, true, ns, diagnostics)?;
        }

        exprs.push(other);
    }

    let aty = Type::Array(
        Box::new(ty),
        dimensions
            .iter()
            .map(|n| ArrayLength::Fixed(BigInt::from_u32(*n).unwrap()))
            .collect::<Vec<ArrayLength>>(),
    );

    if context.constant {
        Ok(Expression::ConstArrayLiteral {
            loc: *loc,
            ty: aty,
            dimensions,
            values: exprs,
        })
    } else {
        Ok(Expression::ArrayLiteral {
            loc: *loc,
            ty: aty,
            dimensions,
            values: exprs,
        })
    }
}

/// Traverse the literal looking for sub arrays. Ensure that all the sub
/// arrays are the same length, and returned a flattened array of elements
fn check_subarrays<'a>(
    exprs: &'a [pt::Expression],
    dims: &mut Option<&mut Vec<u32>>,
    flatten: &mut Vec<&'a pt::Expression>,
    diagnostics: &mut Diagnostics,
) -> Result<(), ()> {
    if let Some(pt::Expression::ArrayLiteral(_, first)) = exprs.first() {
        // ensure all elements are array literals of the same length
        check_subarrays(first, dims, flatten, diagnostics)?;

        for (i, e) in exprs.iter().enumerate().skip(1) {
            if let pt::Expression::ArrayLiteral(_, other) = e {
                if other.len() != first.len() {
                    diagnostics.push(Diagnostic::error(
                        e.loc(),
                        format!(
                            "array elements should be identical, sub array {} has {} elements rather than {}", i + 1, other.len(), first.len()
                        ),
                    ));
                    return Err(());
                }
                check_subarrays(other, &mut None, flatten, diagnostics)?;
            } else {
                diagnostics.push(Diagnostic::error(
                    e.loc(),
                    format!("array element {} should also be an array", i + 1),
                ));
                return Err(());
            }
        }
    } else {
        for (i, e) in exprs.iter().enumerate().skip(1) {
            if let pt::Expression::ArrayLiteral(loc, _) = e {
                diagnostics.push(Diagnostic::error(
                    *loc,
                    format!(
                        "array elements should be of the type, element {} is unexpected array",
                        i + 1
                    ),
                ));
                return Err(());
            }
        }
        flatten.extend(exprs);
    }

    if let Some(dims) = dims.as_deref_mut() {
        dims.push(exprs.len() as u32);
    }

    Ok(())
}
