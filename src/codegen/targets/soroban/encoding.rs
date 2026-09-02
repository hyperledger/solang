// SPDX-License-Identifier: Apache-2.0

use crate::codegen::cfg::InternalCallTy;
use crate::codegen::cfg::{ControlFlowGraph, Instr};
use crate::codegen::encoding::create_encoder;
use crate::codegen::error::CodegenError;
use crate::codegen::vartable::Vartable;
use crate::codegen::HostFunctions;
use crate::codegen::{Builtin, Expression};
use crate::sema::ast::{Namespace, RetrieveType, StructType, Type, Type::Uint};
use num_bigint::BigInt;
use num_traits::Zero;
use solang_parser::helpers::CodeLocation;
use solang_parser::pt;
use solang_parser::pt::Loc;

#[allow(dead_code)]
pub(super) mod tags {
    // Inline / small-value tags (CAP-0046-01 §ScVal bit layout, bits 0-7)
    pub const FALSE: u64 = 0;
    pub const TRUE: u64 = 1;
    pub const VOID: u64 = 2;
    pub const ERROR: u64 = 3;
    pub const U32: u64 = 4;
    pub const I32: u64 = 5;
    pub const U64_SML: u64 = 6;
    pub const I64_SML: u64 = 7;
    pub const U128_SML: u64 = 10;
    pub const I128_SML: u64 = 11;
    pub const U256_SML: u64 = 12;
    pub const I256_SML: u64 = 13;

    // Object-handle tags (host allocates; handle stored in bits 32-63)
    pub const U64_OBJ: u64 = 64;
    pub const I64_OBJ: u64 = 65;
    pub const U128_OBJ: u64 = 68;
    pub const I128_OBJ: u64 = 69;
    pub const U256_OBJ: u64 = 70;
    pub const I256_OBJ: u64 = 71;
    pub const BYTES_OBJ: u64 = 72;
    pub const STRING_OBJ: u64 = 73;
    pub const SYMBOL_OBJ: u64 = 74;
    pub const VEC_OBJ: u64 = 75;
    pub const MAP_OBJ: u64 = 76;
    pub const ADDR_OBJ: u64 = 77;
}

pub fn soroban_encode(
    loc: &Loc,
    args: Vec<Expression>,
    ns: &Namespace,
    vartab: &mut Vartable,
    cfg: &mut ControlFlowGraph,
    packed: bool,
) -> (Expression, Expression, Vec<Expression>) {
    let mut encoder = create_encoder(ns, packed);

    let size = 8 * args.len(); // 8 bytes per argument

    let size_expr = Expression::NumberLiteral {
        loc: *loc,
        ty: Uint(32),
        value: size.into(),
    };
    let encoded_bytes = vartab.temp_name("abi_encoded", &Type::DynamicBytes);

    let expr = Expression::AllocDynamicBytes {
        loc: *loc,
        ty: Type::DynamicBytes,
        size: size_expr.clone().into(),
        initializer: None,
    };

    cfg.add(
        vartab,
        Instr::Set {
            loc: *loc,
            res: encoded_bytes,
            expr,
        },
    );

    let mut offset = Expression::NumberLiteral {
        loc: *loc,
        ty: Uint(64),
        value: BigInt::zero(),
    };

    let buffer = Expression::Variable {
        loc: *loc,
        ty: Type::DynamicBytes,
        var_no: encoded_bytes,
    };

    let mut encoded_items = Vec::new();

    for (arg_no, item) in args.iter().enumerate() {
        let var = if matches!(
            item,
            Expression::AllocDynamicBytes { .. } | Expression::BytesLiteral { .. }
        ) {
            encode_as_symbol(item.clone(), cfg, vartab, ns)
        } else {
            soroban_encode_arg(item.clone(), cfg, vartab, ns)
        };

        encoded_items.push(var.clone());

        let advance = encoder.encode(&var, &buffer, &offset, arg_no, ns, vartab, cfg);
        offset = Expression::Add {
            loc: *loc,
            ty: Uint(64),
            overflowing: false,
            left: offset.into(),
            right: advance.into(),
        };
    }

    (buffer, size_expr, encoded_items)
}

pub fn soroban_decode(
    _loc: &Loc,
    buffer: &Expression,
    types: &[Type],
    ns: &Namespace,
    vartab: &mut Vartable,
    cfg: &mut ControlFlowGraph,
    _buffer_size_expr: Option<Expression>,
) -> Vec<Expression> {
    let mut returns = Vec::with_capacity(types.len());
    for (i, ty) in types.iter().enumerate() {
        let loaded_val = Expression::Builtin {
            loc: Loc::Codegen,
            tys: vec![Type::Uint(64)],
            kind: Builtin::ReadFromBuffer,
            args: vec![
                buffer.clone(),
                Expression::NumberLiteral {
                    loc: Loc::Codegen,
                    ty: Type::Uint(32),
                    value: BigInt::from(8 * i),
                },
            ],
        };
        let decoded_val = soroban_decode_arg(loaded_val, cfg, vartab, ns, Some(ty.clone()));
        returns.push(decoded_val);
    }
    returns
}

pub fn soroban_decode_arg(
    arg: Expression,
    wrapper_cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    ns: &Namespace,
    decode_as: Option<Type>,
) -> Expression {
    let ty = match decode_as {
        Some(ty) => ty,
        None => {
            if let Type::Ref(inner_ty) = arg.ty() {
                *inner_ty
            } else if let Type::StorageRef(_, inner) = arg.ty() {
                *inner
            } else if let Type::SorobanHandle(inner) = arg.ty() {
                *inner
            } else {
                arg.ty()
            }
        }
    };

    match ty {
        Type::Bool => Expression::NotEqual {
            loc: Loc::Codegen,
            left: arg.into(),
            right: Box::new(Expression::NumberLiteral {
                loc: Loc::Codegen,
                ty: Type::Uint(64),
                value: 0u64.into(),
            }),
        },
        Type::Uint(64) => decode_u64(wrapper_cfg, vartab, arg),

        Type::Address(_) => arg.clone(),
        Type::String => decode_string(arg, wrapper_cfg, vartab),
        Type::DynamicBytes => decode_bytes(arg, wrapper_cfg, vartab),
        Type::Bytes(n) => {
            let as_dyn = decode_bytes(arg, wrapper_cfg, vartab);
            Expression::BytesCast {
                loc: Loc::Codegen,
                ty: Type::Bytes(n),
                from: Type::DynamicBytes,
                expr: Box::new(as_dyn),
            }
        }

        Type::Enum(enum_no) => {
            let decoded = soroban_decode_arg(arg, wrapper_cfg, vartab, ns, Some(Type::Uint(32)));
            decoded.cast(&Type::Enum(enum_no), ns)
        }

        Type::Int(128) | Type::Uint(128) => decode_i128(wrapper_cfg, vartab, arg, &ty),

        Type::Int(256) | Type::Uint(256) => decode_i256(wrapper_cfg, vartab, arg, &ty),

        Type::Uint(32) => {
            // get payload out of major bits then truncate to 32‑bit
            Expression::Trunc {
                loc: Loc::Codegen,
                ty: Type::Uint(32),
                expr: Box::new(Expression::ShiftRight {
                    loc: Loc::Codegen,
                    ty: Type::Uint(64),
                    left: arg.into(),
                    right: Box::new(Expression::NumberLiteral {
                        loc: Loc::Codegen,
                        ty: Type::Uint(64),
                        value: 32u64.into(),
                    }),
                    signed: false,
                }),
            }
        }

        Type::Int(32) => Expression::Trunc {
            loc: Loc::Codegen,
            ty: Type::Int(32),
            expr: Box::new(Expression::ShiftRight {
                loc: Loc::Codegen,
                ty: Type::Int(64),
                left: arg.into(),
                right: Box::new(Expression::NumberLiteral {
                    loc: Loc::Codegen,
                    ty: Type::Uint(64),
                    value: 32u64.into(),
                }),
                signed: true,
            }),
        },
        Type::Int(64) => decode_i64(wrapper_cfg, vartab, arg),
        Type::Struct(StructType::UserDefined(n)) => {
            decode_struct_map(arg, wrapper_cfg, vartab, n, ns, ty)
        }
        Type::Array(base, dims) => {
            if let Type::StorageRef(_, _) = arg.ty() {
                arg.clone()
            } else {
                let array_ty = Type::Array(base, dims);
                decode_vector(arg, &array_ty, ns, wrapper_cfg, vartab, false)
            }
        }

        _ => panic!(
            "{}",
            CodegenError::unsupported_soroban_type(
                arg.loc(),
                "by the Soroban decoder",
                ty.to_string(ns),
            )
        ),
    }
}

pub fn soroban_storage_decode_arg(
    arg: Expression,
    wrapper_cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    ns: &Namespace,
    decode_as: Option<Type>,
) -> Expression {
    let ty = match &decode_as {
        Some(ty) => ty.clone(),
        None => match arg.ty() {
            Type::Ref(inner) => *inner,
            Type::StorageRef(_, inner) => *inner,
            Type::SorobanHandle(inner) => *inner,
            other => other,
        },
    };

    match ty {
        Type::Struct(StructType::UserDefined(n)) => {
            decode_struct_storage(arg, wrapper_cfg, vartab, n, ns, ty)
        }
        Type::Array(..) if !matches!(arg.ty(), Type::StorageRef(_, _)) => {
            decode_vector(arg, &ty, ns, wrapper_cfg, vartab, true)
        }
        _ => soroban_decode_arg(arg, wrapper_cfg, vartab, ns, decode_as),
    }
}

pub fn soroban_storage_encode_arg(
    item: Expression,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    ns: &Namespace,
) -> Expression {
    let ty = match item.ty() {
        Type::Ref(inner) => *inner,
        other => other,
    };
    match ty {
        Type::Struct(StructType::UserDefined(n)) => encode_struct_storage(item, cfg, vartab, ns, n),
        Type::Array(..) => encode_vector(item, cfg, vartab, ns, true),
        _ => soroban_encode_arg(item, cfg, vartab, ns),
    }
}

pub fn soroban_encode_arg(
    item: Expression,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    ns: &Namespace,
) -> Expression {
    if let Type::Bytes(n) = item.ty() {
        let as_dyn = Expression::BytesCast {
            loc: item.loc(),
            ty: Type::DynamicBytes,
            from: Type::Bytes(n),
            expr: Box::new(item),
        };
        return soroban_encode_arg(as_dyn, cfg, vartab, ns);
    }

    let obj = vartab.temp_name("obj_".to_string().as_str(), &Type::Uint(64));

    let ret = match item.ty() {
        Type::Bool => {
            let encoded = match item {
                Expression::BoolLiteral { value, .. } => Expression::NumberLiteral {
                    loc: item.loc(),
                    ty: Type::Uint(64),
                    value: if value { 1u64.into() } else { 0u64.into() },
                },
                _ => item.cast(&Type::Uint(64), ns),
            };

            Instr::Set {
                loc: item.loc(),
                res: obj,
                expr: encoded,
            }
        }
        Type::String | Type::DynamicBytes => {
            let loc = item.loc();

            let item_var = if matches!(item, Expression::Variable { .. }) {
                item.clone()
            } else {
                let tmp = vartab.temp_name("vec_tmp", &item.ty());
                cfg.add(
                    vartab,
                    Instr::Set {
                        loc,
                        res: tmp,
                        expr: item.clone(),
                    },
                );
                Expression::Variable {
                    loc,
                    ty: item.ty(),
                    var_no: tmp,
                }
            };

            let ptr_u32val = encode_object(
                loc,
                Expression::VectorData {
                    pointer: Box::new(item_var.clone()),
                },
                32,
                tags::U32,
            );
            let len_u32val = encode_object(
                loc,
                Expression::Builtin {
                    loc,
                    tys: vec![Type::Uint(32)],
                    kind: Builtin::ArrayLength,
                    args: vec![item_var],
                },
                32,
                tags::U32,
            );

            let host_fn = if matches!(item.ty(), Type::String) {
                HostFunctions::StringNewFromLinearMemory // b.i
            } else {
                HostFunctions::BytesNewFromLinearMemory // b.3
            };

            host_call(vec![obj], host_fn, vec![ptr_u32val, len_u32val])
        }
        Type::Uint(32) | Type::Int(32) => {
            // widen to 64 bits so we can shift
            let widened = match item.ty() {
                Type::Uint(32) => Expression::ZeroExt {
                    loc: item.loc(),
                    ty: Type::Uint(64),
                    expr: Box::new(item.clone()),
                },
                Type::Int(32) => Expression::SignExt {
                    loc: item.loc(),
                    ty: Type::Int(64),
                    expr: Box::new(item.clone()),
                },
                _ => unreachable!(),
            };

            // the value goes into the major bits of the 64 bit value
            let shifted = Expression::ShiftLeft {
                loc: item.loc(),
                ty: Type::Uint(64),
                left: Box::new(widened.cast(&Type::Uint(64), ns)),
                right: Box::new(Expression::NumberLiteral {
                    loc: item.loc(),
                    ty: Type::Uint(64),
                    value: 32u64.into(), // 24 (minor) + 8 (tag)
                }),
            };

            let tag = if matches!(item.ty(), Type::Uint(32)) {
                4
            } else {
                5
            };
            Instr::Set {
                loc: item.loc(),
                res: obj,
                expr: Expression::Add {
                    loc: item.loc(),
                    ty: Type::Uint(64),
                    left: Box::new(shifted),
                    right: Box::new(Expression::NumberLiteral {
                        loc: item.loc(),
                        ty: Type::Uint(64),
                        value: tag.into(),
                    }),
                    overflowing: false,
                },
            }
        }
        Type::Enum(_) => {
            let widened = Expression::ZeroExt {
                loc: item.loc(),
                ty: Type::Uint(64),
                expr: Box::new(item.cast(&Type::Uint(32), ns)),
            };

            let shifted = Expression::ShiftLeft {
                loc: item.loc(),
                ty: Type::Uint(64),
                left: Box::new(widened),
                right: Box::new(Expression::NumberLiteral {
                    loc: item.loc(),
                    ty: Type::Uint(64),
                    value: 32u64.into(),
                }),
            };

            Instr::Set {
                loc: item.loc(),
                res: obj,
                expr: Expression::Add {
                    loc: item.loc(),
                    ty: Type::Uint(64),
                    left: Box::new(shifted),
                    right: Box::new(Expression::NumberLiteral {
                        loc: item.loc(),
                        ty: Type::Uint(64),
                        value: tags::U32.into(),
                    }),
                    overflowing: false,
                },
            }
        }
        Type::Uint(64) => {
            let encoded = encode_u64(cfg, vartab, item.clone());
            Instr::Set {
                loc: item.loc(),
                res: obj,
                expr: encoded,
            }
        }
        Type::Int(64) => {
            let encoded = encode_i64(cfg, vartab, item.clone(), ns);
            Instr::Set {
                loc: item.loc(),
                res: obj,
                expr: encoded,
            }
        }
        Type::Address(_) => {
            if let Expression::Cast {
                loc: _,
                ty: _,
                expr,
            } = item.clone()
            {
                if let Expression::BytesLiteral { loc, ty: _, value } = *expr.clone() {
                    let address_literal = expr;

                    let pointer = Expression::VectorData {
                        pointer: address_literal.clone(),
                    };

                    let pointer_extend = Expression::ZeroExt {
                        loc,
                        ty: Type::Uint(64),
                        expr: Box::new(pointer),
                    };

                    let encoded = Expression::ShiftLeft {
                        loc,
                        ty: Uint(64),
                        left: Box::new(pointer_extend),
                        right: Box::new(Expression::NumberLiteral {
                            loc,
                            ty: Type::Uint(64),
                            value: BigInt::from(32),
                        }),
                    };

                    let encoded = Expression::Add {
                        loc,
                        ty: Type::Uint(64),
                        overflowing: true,
                        left: Box::new(encoded),
                        right: Box::new(Expression::NumberLiteral {
                            loc,
                            ty: Type::Uint(64),
                            value: BigInt::from(tags::U32),
                        }),
                    };

                    let len = Expression::NumberLiteral {
                        loc,
                        ty: Type::Uint(64),
                        value: BigInt::from(value.len() as u64),
                    };

                    let len = Expression::ShiftLeft {
                        loc,
                        ty: Type::Uint(64),
                        left: Box::new(len),
                        right: Box::new(Expression::NumberLiteral {
                            loc,
                            ty: Type::Uint(64),
                            value: BigInt::from(32),
                        }),
                    };

                    let len = Expression::Add {
                        loc,
                        ty: Type::Uint(64),
                        left: Box::new(len),
                        right: Box::new(Expression::NumberLiteral {
                            loc,
                            ty: Type::Uint(64),
                            value: BigInt::from(tags::U32),
                        }),
                        overflowing: false,
                    };

                    let str_key_temp = vartab.temp_name("str_key", &Type::Uint(64));
                    let str_key_var = Expression::Variable {
                        loc,
                        ty: Type::Uint(64),
                        var_no: str_key_temp,
                    };

                    cfg.add(
                        vartab,
                        host_call(
                            vec![str_key_temp],
                            HostFunctions::StringNewFromLinearMemory,
                            vec![encoded.clone(), len.clone()],
                        ),
                    );

                    host_call(vec![obj], HostFunctions::StrKeyToAddr, vec![str_key_var])
                } else {
                    Instr::Set {
                        loc: Loc::Codegen,
                        res: obj,
                        expr: item.clone(),
                    }
                }
            } else {
                Instr::Set {
                    loc: Loc::Codegen,
                    res: obj,
                    expr: item.clone(),
                }
            }
        }
        Type::Int(128) | Type::Uint(128) => {
            let low = Expression::Trunc {
                loc: Loc::Codegen,
                ty: Type::Int(64),
                expr: Box::new(item.clone()),
            };

            let high = Expression::ShiftRight {
                loc: Loc::Codegen,
                ty: Type::Int(128),
                left: Box::new(item.clone()),
                right: Box::new(Expression::NumberLiteral {
                    loc: Loc::Codegen,
                    ty: Type::Int(128),
                    value: BigInt::from(64),
                }),
                signed: false,
            };

            let high = Expression::Trunc {
                loc: Loc::Codegen,
                ty: Type::Int(64),
                expr: Box::new(high),
            };

            let encoded = encode_i128(cfg, vartab, low, high, item.ty());
            Instr::Set {
                loc: item.loc(),
                res: obj,
                expr: encoded,
            }
        }
        Type::Int(256) | Type::Uint(256) => {
            let encoded = encode_i256(cfg, vartab, item.clone());
            Instr::Set {
                loc: item.loc(),
                res: obj,
                expr: encoded,
            }
        }
        Type::Struct(StructType::UserDefined(n)) => {
            let map = encode_struct_map(item.clone(), cfg, vartab, ns, n);
            Instr::Set {
                loc: Loc::Codegen,
                res: obj,
                expr: map,
            }
        }
        Type::SorobanHandle(_) => Instr::Set {
            loc: Loc::Codegen,
            res: obj,
            expr: item.clone(),
        },
        Type::Array(_, _) => Instr::Set {
            loc: Loc::Codegen,
            res: obj,
            expr: encode_vector(item.clone(), cfg, vartab, ns, false),
        },

        _ => panic!(
            "{}",
            CodegenError::unsupported_soroban_type(
                item.loc(),
                "by the Soroban encoder",
                item.ty().to_string(ns),
            )
        ),
    };

    cfg.add(vartab, ret);

    Expression::Variable {
        loc: pt::Loc::Codegen,
        ty: Type::Uint(64),
        var_no: obj,
    }
}

fn encode_i128(
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    lo: Expression,
    high: Expression,
    int128_ty: Type,
) -> Expression {
    let ret_var = vartab.temp_anonymous(&lo.ty());

    let ret = Expression::Variable {
        loc: pt::Loc::Codegen,
        ty: lo.ty().clone(),
        var_no: ret_var,
    };

    vartab.new_dirty_tracker();

    let check_lo = cfg.new_basic_block("check_lo".to_string());
    let fits_in_56_bits = cfg.new_basic_block("fits_in_56_bits".to_string());
    let should_be_in_host = cfg.new_basic_block("should_be_in_host".to_string());
    let return_block = cfg.new_basic_block("finish".to_string());

    let high_is_zero = Expression::Equal {
        loc: pt::Loc::Codegen,
        left: high.clone().into(),
        right: Expression::NumberLiteral {
            loc: pt::Loc::Codegen,
            ty: high.ty(),
            value: BigInt::from(0_u64),
        }
        .into(),
    };

    cfg.add(
        vartab,
        Instr::BranchCond {
            cond: high_is_zero,
            true_block: check_lo,
            false_block: should_be_in_host,
        },
    );

    cfg.set_basic_block(check_lo);

    // check if the low limb fits within the small representation limit
    // signed positive must stay under 55 bits to avoid sign-extension confusion.
    // unsigned can use up to 56 bits
    let shift_amount = match int128_ty {
        Type::Uint(128) => 56_u64,
        Type::Int(128) => 55_u64,
        _ => unreachable!(),
    };

    let lo_shifted = Expression::ShiftRight {
        loc: pt::Loc::Codegen,
        ty: Type::Uint(64),
        left: lo.clone().into(),
        right: Expression::NumberLiteral {
            loc: pt::Loc::Codegen,
            ty: Type::Uint(64),
            value: BigInt::from(shift_amount),
        }
        .into(),
        signed: false,
    };

    let lo_is_small = Expression::Equal {
        loc: pt::Loc::Codegen,
        left: lo_shifted.into(),
        right: Expression::NumberLiteral {
            loc: pt::Loc::Codegen,
            ty: Type::Uint(64),
            value: BigInt::from(0_u64),
        }
        .into(),
    };

    cfg.add(
        vartab,
        Instr::BranchCond {
            cond: lo_is_small,
            true_block: fits_in_56_bits,
            false_block: should_be_in_host,
        },
    );

    cfg.set_basic_block(fits_in_56_bits);

    let to_return = Expression::ShiftLeft {
        loc: Loc::Codegen,
        ty: Type::Uint(64),
        left: Box::new(lo.clone()),
        right: Box::new(Expression::NumberLiteral {
            loc: Loc::Codegen,
            ty: Type::Uint(64),
            value: BigInt::from(8_u64),
        }),
    };
    let tag = match int128_ty {
        Type::Int(128) => tags::I128_SML,
        Type::Uint(128) => tags::U128_SML,
        _ => unreachable!(),
    };

    let to_return = Expression::Add {
        loc: Loc::Codegen,
        ty: Type::Uint(64),
        left: to_return.into(),
        right: Expression::NumberLiteral {
            loc: Loc::Codegen,
            ty: Type::Uint(64),
            value: BigInt::from(tag),
        }
        .into(),
        overflowing: false,
    };

    let set_instr = Instr::Set {
        loc: pt::Loc::Codegen,
        res: ret_var,
        expr: to_return,
    };
    cfg.add(vartab, set_instr);

    cfg.add(
        vartab,
        Instr::Branch {
            block: return_block,
        },
    );

    cfg.set_basic_block(should_be_in_host);

    let instr = match int128_ty {
        Type::Int(128) => host_call(
            vec![ret_var],
            HostFunctions::ObjFromI128Pieces,
            vec![high, lo],
        ),
        Type::Uint(128) => host_call(
            vec![ret_var],
            HostFunctions::ObjFromU128Pieces,
            vec![high, lo],
        ),
        _ => unreachable!(),
    };

    cfg.add(vartab, instr);

    cfg.add(
        vartab,
        Instr::Branch {
            block: return_block,
        },
    );

    cfg.set_basic_block(return_block);
    cfg.set_phis(return_block, vartab.pop_dirty_tracker());

    ret
}

fn encode_u64(cfg: &mut ControlFlowGraph, vartab: &mut Vartable, value: Expression) -> Expression {
    let ret_var = vartab.temp_anonymous(&Type::Uint(64));

    let ret = Expression::Variable {
        loc: pt::Loc::Codegen,
        ty: Type::Uint(64),
        var_no: ret_var,
    };

    vartab.new_dirty_tracker();

    let fits_in_56_bits = cfg.new_basic_block("u64_fits_in_56_bits".to_string());
    let should_be_in_host = cfg.new_basic_block("u64_should_be_in_host".to_string());
    let return_block = cfg.new_basic_block("u64_finish".to_string());

    let high_8_bits = Expression::ShiftRight {
        loc: pt::Loc::Codegen,
        ty: Type::Uint(64),
        left: value.clone().into(),
        right: Expression::NumberLiteral {
            loc: pt::Loc::Codegen,
            ty: Type::Uint(64),
            value: BigInt::from(56_u64),
        }
        .into(),
        signed: false,
    };

    let cond = Expression::Equal {
        loc: pt::Loc::Codegen,
        left: high_8_bits.into(),
        right: Expression::NumberLiteral {
            loc: pt::Loc::Codegen,
            ty: Type::Uint(64),
            value: BigInt::from(0_u64),
        }
        .into(),
    };

    cfg.add(
        vartab,
        Instr::BranchCond {
            cond,
            true_block: fits_in_56_bits,
            false_block: should_be_in_host,
        },
    );

    cfg.set_basic_block(fits_in_56_bits);

    let small_value = Expression::ShiftLeft {
        loc: Loc::Codegen,
        ty: Type::Uint(64),
        left: Box::new(value.clone()),
        right: Box::new(Expression::NumberLiteral {
            loc: Loc::Codegen,
            ty: Type::Uint(64),
            value: BigInt::from(8_u64),
        }),
    };

    let small_value = Expression::Add {
        loc: Loc::Codegen,
        ty: Type::Uint(64),
        left: small_value.into(),
        right: Expression::NumberLiteral {
            loc: Loc::Codegen,
            ty: Type::Uint(64),
            value: BigInt::from(tags::U64_SML),
        }
        .into(),
        overflowing: false,
    };

    cfg.add(
        vartab,
        Instr::Set {
            loc: pt::Loc::Codegen,
            res: ret_var,
            expr: small_value,
        },
    );

    cfg.add(
        vartab,
        Instr::Branch {
            block: return_block,
        },
    );

    cfg.set_basic_block(should_be_in_host);

    cfg.add(
        vartab,
        host_call(vec![ret_var], HostFunctions::ObjFromU64, vec![value]),
    );

    cfg.add(
        vartab,
        Instr::Branch {
            block: return_block,
        },
    );

    cfg.set_basic_block(return_block);
    cfg.set_phis(return_block, vartab.pop_dirty_tracker());

    ret
}

fn encode_i64(
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    value: Expression,
    ns: &Namespace,
) -> Expression {
    let ret_var = vartab.temp_anonymous(&Type::Uint(64));

    let ret = Expression::Variable {
        loc: pt::Loc::Codegen,
        ty: Type::Uint(64),
        var_no: ret_var,
    };

    vartab.new_dirty_tracker();

    let fits_small = cfg.new_basic_block("i64_fits_small".to_string());
    let should_be_in_host = cfg.new_basic_block("i64_should_be_in_host".to_string());
    let return_block = cfg.new_basic_block("i64_finish".to_string());

    let shifted_left = Expression::ShiftLeft {
        loc: Loc::Codegen,
        ty: Type::Int(64),
        left: Box::new(value.clone()),
        right: Box::new(Expression::NumberLiteral {
            loc: Loc::Codegen,
            ty: Type::Uint(64),
            value: BigInt::from(8_u64),
        }),
    };

    let round_trip = Expression::ShiftRight {
        loc: Loc::Codegen,
        ty: Type::Int(64),
        left: Box::new(shifted_left),
        right: Box::new(Expression::NumberLiteral {
            loc: Loc::Codegen,
            ty: Type::Uint(64),
            value: BigInt::from(8_u64),
        }),
        signed: true,
    };

    let fits = Expression::Equal {
        loc: Loc::Codegen,
        left: Box::new(round_trip),
        right: Box::new(value.clone()),
    };

    cfg.add(
        vartab,
        Instr::BranchCond {
            cond: fits,
            true_block: fits_small,
            false_block: should_be_in_host,
        },
    );

    cfg.set_basic_block(fits_small);

    let value_bits = value.clone().cast(&Type::Uint(64), ns);
    let small_value = Expression::ShiftLeft {
        loc: Loc::Codegen,
        ty: Type::Uint(64),
        left: Box::new(value_bits),
        right: Box::new(Expression::NumberLiteral {
            loc: Loc::Codegen,
            ty: Type::Uint(64),
            value: BigInt::from(8_u64),
        }),
    };

    let small_value = Expression::Add {
        loc: Loc::Codegen,
        ty: Type::Uint(64),
        left: Box::new(small_value),
        right: Box::new(Expression::NumberLiteral {
            loc: Loc::Codegen,
            ty: Type::Uint(64),
            value: BigInt::from(tags::I64_SML),
        }),
        overflowing: false,
    };

    cfg.add(
        vartab,
        Instr::Set {
            loc: pt::Loc::Codegen,
            res: ret_var,
            expr: small_value,
        },
    );

    cfg.add(
        vartab,
        Instr::Branch {
            block: return_block,
        },
    );

    cfg.set_basic_block(should_be_in_host);

    cfg.add(
        vartab,
        Instr::Call {
            res: vec![ret_var],
            return_tys: vec![Type::Uint(64)],
            call: InternalCallTy::HostFunction {
                name: HostFunctions::ObjFromI64.name().to_string(),
            },
            args: vec![value],
        },
    );

    cfg.add(
        vartab,
        Instr::Branch {
            block: return_block,
        },
    );

    cfg.set_basic_block(return_block);
    cfg.set_phis(return_block, vartab.pop_dirty_tracker());

    ret
}

const SMALL_BODY_BITS: u64 = 56;

fn i256_extract_limb(item: &Expression, shift: u64, is_signed: bool) -> Expression {
    let source = if shift == 0 {
        item.clone()
    } else {
        Expression::ShiftRight {
            loc: Loc::Codegen,
            ty: item.ty(),
            left: Box::new(item.clone()),
            right: Box::new(Expression::NumberLiteral {
                loc: Loc::Codegen,
                ty: item.ty(),
                value: BigInt::from(shift),
            }),
            signed: is_signed,
        }
    };

    Expression::Trunc {
        loc: Loc::Codegen,
        ty: Type::Int(64),
        expr: Box::new(source),
    }
}

fn i256_recompose_limb(
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    arg: &Expression,
    host_fn: HostFunctions,
    shift: u64,
    ty: &Type,
) -> Expression {
    let limb_var = vartab.temp_anonymous(&Type::Uint(64));
    cfg.add(
        vartab,
        host_call(vec![limb_var], host_fn, vec![arg.clone()]),
    );

    let limb = Expression::ZeroExt {
        loc: Loc::Codegen,
        ty: ty.clone(),
        expr: Box::new(Expression::Variable {
            loc: Loc::Codegen,
            ty: Type::Uint(64),
            var_no: limb_var,
        }),
    };

    if shift == 0 {
        limb
    } else {
        Expression::ShiftLeft {
            loc: Loc::Codegen,
            ty: ty.clone(),
            left: Box::new(limb),
            right: Box::new(Expression::NumberLiteral {
                loc: Loc::Codegen,
                ty: ty.clone(),
                value: BigInt::from(shift),
            }),
        }
    }
}

fn encode_i256(cfg: &mut ControlFlowGraph, vartab: &mut Vartable, item: Expression) -> Expression {
    let ret_var = vartab.temp_name("encoded_i256", &Type::Uint(64));
    let ret = Expression::Variable {
        loc: pt::Loc::Codegen,
        ty: Type::Uint(64),
        var_no: ret_var,
    };

    let ty = item.ty();
    let is_signed = matches!(ty, Type::Int(256));
    let small_tag = if is_signed {
        tags::I256_SML
    } else {
        tags::U256_SML
    };

    vartab.new_dirty_tracker();
    let small_obj = cfg.new_basic_block("i256_small_obj".to_string());
    let host_obj = cfg.new_basic_block("i256_host_obj".to_string());
    let merge_block = cfg.new_basic_block("i256_merge".to_string());

    let high_bits = 256 - SMALL_BODY_BITS;
    let round_trip = Expression::ShiftRight {
        loc: Loc::Codegen,
        ty: ty.clone(),
        left: Box::new(Expression::ShiftLeft {
            loc: Loc::Codegen,
            ty: ty.clone(),
            left: Box::new(item.clone()),
            right: Box::new(Expression::NumberLiteral {
                loc: Loc::Codegen,
                ty: ty.clone(),
                value: BigInt::from(high_bits),
            }),
        }),
        right: Box::new(Expression::NumberLiteral {
            loc: Loc::Codegen,
            ty: ty.clone(),
            value: BigInt::from(high_bits),
        }),
        signed: is_signed,
    };
    let cond = Expression::Equal {
        loc: Loc::Codegen,
        left: Box::new(round_trip),
        right: Box::new(item.clone()),
    };
    cfg.add(
        vartab,
        Instr::BranchCond {
            cond,
            true_block: small_obj,
            false_block: host_obj,
        },
    );

    cfg.set_basic_block(small_obj);
    let body = i256_extract_limb(&item, 0, is_signed);
    let encoded_val = encode_object(Loc::Codegen, body, 8, small_tag);
    cfg.add(
        vartab,
        Instr::Set {
            loc: Loc::Codegen,
            res: ret_var,
            expr: encoded_val,
        },
    );
    cfg.add(vartab, Instr::Branch { block: merge_block });

    cfg.set_basic_block(host_obj);
    let host_fn = if is_signed {
        HostFunctions::ObjFromI256Pieces
    } else {
        HostFunctions::ObjFromU256Pieces
    };
    let pieces = vec![
        i256_extract_limb(&item, 192, is_signed),
        i256_extract_limb(&item, 128, is_signed),
        i256_extract_limb(&item, 64, is_signed),
        i256_extract_limb(&item, 0, is_signed),
    ];
    cfg.add(vartab, host_call(vec![ret_var], host_fn, pieces));
    cfg.add(vartab, Instr::Branch { block: merge_block });

    cfg.set_basic_block(merge_block);
    cfg.set_phis(merge_block, vartab.pop_dirty_tracker());
    ret
}

fn decode_i128(
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    arg: Expression,
    ty: &Type,
) -> Expression {
    let ty: Type = ty.clone();

    let ret_var = vartab.temp_anonymous(&ty);

    let ret = Expression::Variable {
        loc: pt::Loc::Codegen,
        ty: ty.clone(),
        var_no: ret_var,
    };

    vartab.new_dirty_tracker();

    let tag = extract_tag(arg.clone());

    let val_in_host = cfg.new_basic_block("val_is_host".to_string());
    let val_in_obj = cfg.new_basic_block("val_is_obj".to_string());
    let return_block = cfg.new_basic_block("finish".to_string());

    let predicate = match ty {
        Type::Int(128) => tags::I128_SML,
        Type::Uint(128) => tags::U128_SML,
        _ => unreachable!(),
    };
    let is_in_obj = Expression::Equal {
        loc: pt::Loc::Codegen,
        left: tag.clone().into(),
        right: Expression::NumberLiteral {
            loc: pt::Loc::Codegen,
            ty: Type::Uint(64),
            value: BigInt::from(predicate),
        }
        .into(),
    };

    cfg.add(
        vartab,
        Instr::BranchCond {
            cond: is_in_obj,
            true_block: val_in_obj,
            false_block: val_in_host,
        },
    );

    cfg.set_basic_block(val_in_obj);

    let is_signed = matches!(ty, Type::Int(128));

    let value = Expression::ShiftRight {
        loc: pt::Loc::Codegen,
        ty: if is_signed {
            Type::Int(64)
        } else {
            Type::Uint(64)
        },
        left: arg.clone().into(),
        right: Expression::NumberLiteral {
            loc: pt::Loc::Codegen,
            ty: Type::Uint(64),
            value: BigInt::from(8_u64),
        }
        .into(),
        signed: is_signed,
    };

    let extend = match ty {
        Type::Int(128) => Expression::SignExt {
            loc: Loc::Codegen,
            ty: ty.clone(),
            expr: Box::new(value.clone()),
        },
        Type::Uint(128) => Expression::ZeroExt {
            loc: Loc::Codegen,
            ty: ty.clone(),
            expr: Box::new(value.clone()),
        },
        _ => unreachable!(),
    };

    let set_instr = Instr::Set {
        loc: pt::Loc::Codegen,
        res: ret_var,
        expr: extend,
    };

    cfg.add(vartab, set_instr);

    cfg.add(
        vartab,
        Instr::Branch {
            block: return_block,
        },
    );

    cfg.set_basic_block(val_in_host);

    let low_var_no = vartab.temp_anonymous(&Type::Uint(64));
    let low_var = Expression::Variable {
        loc: pt::Loc::Codegen,
        ty: Type::Uint(64),
        var_no: low_var_no,
    };

    let get_lo_instr = match ty {
        Type::Int(128) => host_call(
            vec![low_var_no],
            HostFunctions::ObjToI128Lo64,
            vec![arg.clone()],
        ),
        Type::Uint(128) => host_call(
            vec![low_var_no],
            HostFunctions::ObjToU128Lo64,
            vec![arg.clone()],
        ),
        _ => unreachable!(),
    };

    cfg.add(vartab, get_lo_instr);

    let low_var = Expression::ZeroExt {
        loc: Loc::Codegen,
        ty: ty.clone(),
        expr: Box::new(low_var),
    };

    let high_var_no = vartab.temp_anonymous(&Type::Uint(64));
    let high_var = Expression::Variable {
        loc: pt::Loc::Codegen,
        ty: Type::Uint(64),
        var_no: high_var_no,
    };

    let get_hi_instr = match ty {
        Type::Int(128) => host_call(
            vec![high_var_no],
            HostFunctions::ObjToI128Hi64,
            vec![arg.clone()],
        ),
        Type::Uint(128) => host_call(
            vec![high_var_no],
            HostFunctions::ObjToU128Hi64,
            vec![arg.clone()],
        ),
        _ => unreachable!(),
    };

    cfg.add(vartab, get_hi_instr);

    let total = Expression::ZeroExt {
        loc: Loc::Codegen,
        ty: ty.clone(),
        expr: Box::new(high_var),
    };

    let total = Expression::ShiftLeft {
        loc: Loc::Codegen,
        ty: ty.clone(),
        left: Box::new(total),
        right: Box::new(Expression::NumberLiteral {
            loc: Loc::Codegen,
            ty: ty.clone(),
            value: BigInt::from(64),
        }),
    };

    let total = Expression::Add {
        loc: Loc::Codegen,
        ty: ty.clone(),
        overflowing: false,
        left: total.into(),
        right: low_var.into(),
    };

    let set_instr = Instr::Set {
        loc: pt::Loc::Codegen,
        res: ret_var,
        expr: total,
    };

    cfg.add(vartab, set_instr);

    cfg.add(
        vartab,
        Instr::Branch {
            block: return_block,
        },
    );

    cfg.set_basic_block(return_block);
    cfg.set_phis(return_block, vartab.pop_dirty_tracker());

    ret
}

fn decode_i256(
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    arg: Expression,
    ty: &Type,
) -> Expression {
    let ret_var = vartab.temp_name("decoded_i256", ty);
    let ret = Expression::Variable {
        loc: pt::Loc::Codegen,
        ty: ty.clone(),
        var_no: ret_var,
    };

    let is_signed = matches!(ty, Type::Int(256));
    let small_tag = if is_signed {
        tags::I256_SML
    } else {
        tags::U256_SML
    };

    vartab.new_dirty_tracker();
    let small_obj = cfg.new_basic_block("i256_small_obj".to_string());
    let host_obj = cfg.new_basic_block("i256_host_obj".to_string());
    let merge_block = cfg.new_basic_block("i256_merge".to_string());

    let tag = extract_tag(arg.clone());
    let cond = Expression::Equal {
        loc: Loc::Codegen,
        left: Box::new(tag),
        right: Box::new(Expression::NumberLiteral {
            loc: Loc::Codegen,
            ty: Type::Uint(64),
            value: BigInt::from(small_tag),
        }),
    };
    cfg.add(
        vartab,
        Instr::BranchCond {
            cond,
            true_block: small_obj,
            false_block: host_obj,
        },
    );

    cfg.set_basic_block(small_obj);
    let body = Expression::ShiftRight {
        loc: pt::Loc::Codegen,
        ty: Type::Int(64),
        left: arg.clone().into(),
        right: Expression::NumberLiteral {
            loc: pt::Loc::Codegen,
            ty: Type::Int(64),
            value: BigInt::from(8_u64),
        }
        .into(),
        signed: is_signed,
    };
    let extended = if is_signed {
        Expression::SignExt {
            loc: Loc::Codegen,
            ty: ty.clone(),
            expr: Box::new(body),
        }
    } else {
        Expression::ZeroExt {
            loc: Loc::Codegen,
            ty: ty.clone(),
            expr: Box::new(body),
        }
    };
    cfg.add(
        vartab,
        Instr::Set {
            loc: pt::Loc::Codegen,
            res: ret_var,
            expr: extended,
        },
    );
    cfg.add(vartab, Instr::Branch { block: merge_block });

    cfg.set_basic_block(host_obj);
    let (lolo_fn, lohi_fn, hilo_fn, hihi_fn) = if is_signed {
        (
            HostFunctions::ObjToI256LoLo,
            HostFunctions::ObjToI256LoHi,
            HostFunctions::ObjToI256HiLo,
            HostFunctions::ObjToI256HiHi,
        )
    } else {
        (
            HostFunctions::ObjToU256LoLo,
            HostFunctions::ObjToU256LoHi,
            HostFunctions::ObjToU256HiLo,
            HostFunctions::ObjToU256HiHi,
        )
    };
    let limb_000 = i256_recompose_limb(cfg, vartab, &arg, lolo_fn, 0, ty);
    let limb_064 = i256_recompose_limb(cfg, vartab, &arg, lohi_fn, 64, ty);
    let limb_128 = i256_recompose_limb(cfg, vartab, &arg, hilo_fn, 128, ty);
    let limb_192 = i256_recompose_limb(cfg, vartab, &arg, hihi_fn, 192, ty);

    let recomposed = [limb_064, limb_128, limb_192]
        .into_iter()
        .fold(limb_000, |acc, limb| Expression::BitwiseOr {
            loc: Loc::Codegen,
            ty: ty.clone(),
            left: Box::new(acc),
            right: Box::new(limb),
        });

    cfg.add(
        vartab,
        Instr::Set {
            loc: Loc::Codegen,
            res: ret_var,
            expr: recomposed,
        },
    );
    cfg.add(vartab, Instr::Branch { block: merge_block });
    cfg.set_basic_block(merge_block);
    cfg.set_phis(merge_block, vartab.pop_dirty_tracker());
    ret
}

fn decode_u64(cfg: &mut ControlFlowGraph, vartab: &mut Vartable, arg: Expression) -> Expression {
    let ty = match arg.ty() {
        Type::Ref(inner_ty) => *inner_ty.clone(),
        Type::SorobanHandle(inner_ty) => *inner_ty.clone(),
        _ => arg.ty(),
    };

    let ret_var = vartab.temp_anonymous(&ty);

    let ret = Expression::Variable {
        loc: pt::Loc::Codegen,
        ty: ty.clone(),
        var_no: ret_var,
    };

    vartab.new_dirty_tracker();

    let tag = extract_tag(arg.clone());

    let val_is_u64_small = cfg.new_basic_block("u64_val_is_u64_small".to_string());
    let val_is_u32_small = cfg.new_basic_block("u64_val_is_u32_small".to_string());
    let val_in_host = cfg.new_basic_block("u64_val_is_host".to_string());
    let val_not_u64_small = cfg.new_basic_block("u64_val_not_u64_small".to_string());
    let return_block = cfg.new_basic_block("u64_finish".to_string());

    let is_u64_small = Expression::Equal {
        loc: pt::Loc::Codegen,
        left: tag.clone().into(),
        right: Expression::NumberLiteral {
            loc: pt::Loc::Codegen,
            ty: Type::Uint(64),
            value: BigInt::from(tags::U64_SML),
        }
        .into(),
    };

    cfg.add(
        vartab,
        Instr::BranchCond {
            cond: is_u64_small,
            true_block: val_is_u64_small,
            false_block: val_not_u64_small,
        },
    );

    cfg.set_basic_block(val_is_u64_small);

    let u64_small_value = Expression::ShiftRight {
        loc: pt::Loc::Codegen,
        ty: Type::Uint(64),
        left: arg.clone().into(),
        right: Expression::NumberLiteral {
            loc: pt::Loc::Codegen,
            ty: Type::Uint(64),
            value: BigInt::from(8_u64),
        }
        .into(),
        signed: false,
    };

    cfg.add(
        vartab,
        Instr::Set {
            loc: pt::Loc::Codegen,
            res: ret_var,
            expr: u64_small_value,
        },
    );

    cfg.add(
        vartab,
        Instr::Branch {
            block: return_block,
        },
    );

    cfg.set_basic_block(val_not_u64_small);

    // Some host paths (for example VecLen) produce U32Val. Allow widening it
    // when decoding to uint64.
    let is_u32_small = Expression::Equal {
        loc: pt::Loc::Codegen,
        left: tag.into(),
        right: Expression::NumberLiteral {
            loc: pt::Loc::Codegen,
            ty: Type::Uint(64),
            value: BigInt::from(tags::U32),
        }
        .into(),
    };

    cfg.add(
        vartab,
        Instr::BranchCond {
            cond: is_u32_small,
            true_block: val_is_u32_small,
            false_block: val_in_host,
        },
    );

    cfg.set_basic_block(val_is_u32_small);

    let u32_small_value = Expression::ShiftRight {
        loc: pt::Loc::Codegen,
        ty: Type::Uint(64),
        left: arg.clone().into(),
        right: Expression::NumberLiteral {
            loc: pt::Loc::Codegen,
            ty: Type::Uint(64),
            value: BigInt::from(32_u64),
        }
        .into(),
        signed: false,
    };

    cfg.add(
        vartab,
        Instr::Set {
            loc: pt::Loc::Codegen,
            res: ret_var,
            expr: u32_small_value,
        },
    );

    cfg.add(
        vartab,
        Instr::Branch {
            block: return_block,
        },
    );

    cfg.set_basic_block(val_in_host);

    cfg.add(
        vartab,
        host_call(vec![ret_var], HostFunctions::ObjToU64, vec![arg]),
    );

    cfg.add(
        vartab,
        Instr::Branch {
            block: return_block,
        },
    );

    cfg.set_basic_block(return_block);
    cfg.set_phis(return_block, vartab.pop_dirty_tracker());

    ret
}

fn decode_i64(cfg: &mut ControlFlowGraph, vartab: &mut Vartable, arg: Expression) -> Expression {
    let ret_var = vartab.temp_anonymous(&Type::Int(64));
    let ret = Expression::Variable {
        loc: pt::Loc::Codegen,
        ty: Type::Int(64),
        var_no: ret_var,
    };

    vartab.new_dirty_tracker();

    let tag = extract_tag(arg.clone());

    let val_is_small = cfg.new_basic_block("i64_val_is_small".to_string());
    let val_in_host = cfg.new_basic_block("i64_val_is_host".to_string());
    let return_block = cfg.new_basic_block("i64_decode_finish".to_string());

    let is_small = Expression::Equal {
        loc: pt::Loc::Codegen,
        left: tag.into(),
        right: Expression::NumberLiteral {
            loc: pt::Loc::Codegen,
            ty: Type::Uint(64),
            value: BigInt::from(tags::I64_SML),
        }
        .into(),
    };

    cfg.add(
        vartab,
        Instr::BranchCond {
            cond: is_small,
            true_block: val_is_small,
            false_block: val_in_host,
        },
    );

    cfg.set_basic_block(val_is_small);

    let small_value = Expression::ShiftRight {
        loc: pt::Loc::Codegen,
        ty: Type::Int(64),
        left: arg.clone().into(),
        right: Expression::NumberLiteral {
            loc: pt::Loc::Codegen,
            ty: Type::Uint(64),
            value: BigInt::from(8_u64),
        }
        .into(),
        signed: true,
    };

    cfg.add(
        vartab,
        Instr::Set {
            loc: pt::Loc::Codegen,
            res: ret_var,
            expr: small_value,
        },
    );

    cfg.add(
        vartab,
        Instr::Branch {
            block: return_block,
        },
    );

    cfg.set_basic_block(val_in_host);

    cfg.add(
        vartab,
        Instr::Call {
            res: vec![ret_var],
            return_tys: vec![Type::Int(64)],
            call: InternalCallTy::HostFunction {
                name: HostFunctions::ObjToI64.name().to_string(),
            },
            args: vec![arg],
        },
    );

    cfg.add(
        vartab,
        Instr::Branch {
            block: return_block,
        },
    );

    cfg.set_basic_block(return_block);
    cfg.set_phis(return_block, vartab.pop_dirty_tracker());

    ret
}

fn extract_tag(arg: Expression) -> Expression {
    let bit_mask = Expression::NumberLiteral {
        loc: pt::Loc::Codegen,
        ty: Type::Uint(64),
        value: BigInt::from(0xFF),
    };

    Expression::BitwiseAnd {
        loc: pt::Loc::Codegen,
        ty: Type::Uint(64),
        left: arg.clone().into(),
        right: bit_mask.into(),
    }
}

fn struct_field_key(
    name: &str,
    loc: pt::Loc,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    ns: &Namespace,
) -> Expression {
    encode_as_symbol(
        Expression::BytesLiteral {
            loc,
            ty: Type::String,
            value: name.as_bytes().to_vec(),
        },
        cfg,
        vartab,
        ns,
    )
}

fn encode_struct_map(
    item: Expression,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    ns: &Namespace,
    struct_no: usize,
) -> Expression {
    let loc = item.loc();
    let map_var = vartab.temp_name("struct_map", &Type::Uint(64));
    cfg.add(
        vartab,
        Instr::Call {
            res: vec![map_var],
            return_tys: vec![Type::Uint(64)],
            call: InternalCallTy::HostFunction {
                name: HostFunctions::MapNew.name().to_string(),
            },
            args: vec![],
        },
    );
    let map_expr = Expression::Variable {
        loc,
        ty: Type::Uint(64),
        var_no: map_var,
    };

    let fields = &ns.structs[struct_no].fields;
    for (index, field) in fields.iter().enumerate() {
        let name = field
            .id
            .as_ref()
            .map(|id| id.name.clone())
            .unwrap_or_else(|| index.to_string());
        let key = struct_field_key(&name, loc, cfg, vartab, ns);

        let member = Expression::StructMember {
            loc,
            ty: field.ty.clone(),
            expr: Box::new(item.clone()),
            member: index,
        };
        let value = if field.ty.is_fixed_reference_type(ns) {
            member
        } else {
            Expression::Load {
                loc: Loc::Codegen,
                ty: field.ty.clone(),
                expr: Box::new(member),
            }
        };
        let value = soroban_encode_arg(value, cfg, vartab, ns);
        cfg.add(
            vartab,
            Instr::Call {
                res: vec![map_var],
                return_tys: vec![Type::Uint(64)],
                call: InternalCallTy::HostFunction {
                    name: HostFunctions::MapPut.name().to_string(),
                },
                args: vec![map_expr.clone(), key, value],
            },
        );
    }

    map_expr
}

fn encode_struct_storage(
    item: Expression,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    ns: &Namespace,
    struct_no: usize,
) -> Expression {
    let loc = item.loc();
    let field_tys: Vec<Type> = ns.structs[struct_no]
        .fields
        .iter()
        .map(|f| f.ty.clone())
        .collect();

    let mut vec_no = vartab.temp_name("struct_vec", &Type::Uint(64));
    cfg.add(
        vartab,
        Instr::Call {
            res: vec![vec_no],
            return_tys: vec![Type::Uint(64)],
            call: InternalCallTy::HostFunction {
                name: HostFunctions::VectorNew.name().to_string(),
            },
            args: vec![],
        },
    );

    for (index, field_ty) in field_tys.iter().enumerate() {
        let member = Expression::StructMember {
            loc,
            ty: field_ty.clone(),
            expr: Box::new(item.clone()),
            member: index,
        };
        let loaded = if field_ty.is_fixed_reference_type(ns) {
            member
        } else {
            Expression::Load {
                loc: Loc::Codegen,
                ty: field_ty.clone(),
                expr: Box::new(member),
            }
        };
        let encoded = soroban_storage_encode_arg(loaded, cfg, vartab, ns);

        let prev_vec = Expression::Variable {
            loc,
            ty: Type::Uint(64),
            var_no: vec_no,
        };
        let next_vec = vartab.temp_name("struct_vec", &Type::Uint(64));
        cfg.add(
            vartab,
            Instr::Call {
                res: vec![next_vec],
                return_tys: vec![Type::Uint(64)],
                call: InternalCallTy::HostFunction {
                    name: HostFunctions::VecPushBack.name().to_string(),
                },
                args: vec![prev_vec, encoded],
            },
        );
        vec_no = next_vec;
    }

    Expression::Variable {
        loc,
        ty: Type::Uint(64),
        var_no: vec_no,
    }
}

fn encode_vector(
    item: Expression,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    ns: &Namespace,
    storage: bool,
) -> Expression {
    let loc = item.loc();
    let item_ty = item.ty();
    let elem_ty = item_ty.array_elem();

    let arr_no = vartab.temp_name("vec_src", &item_ty);
    cfg.add(
        vartab,
        Instr::Set {
            loc,
            res: arr_no,
            expr: item.clone(),
        },
    );
    let arr = Expression::Variable {
        loc,
        ty: item_ty.clone(),
        var_no: arr_no,
    };

    let len = if let Some(fixed_len) = item_ty.array_length() {
        Expression::NumberLiteral {
            loc,
            ty: Type::Uint(32),
            value: fixed_len.clone(),
        }
    } else {
        Expression::Builtin {
            loc,
            tys: vec![Type::Uint(32)],
            kind: Builtin::ArrayLength,
            args: vec![arr.clone()],
        }
    };

    let vec_no = vartab.temp_name("vec_obj", &Type::Uint(64));
    cfg.add(
        vartab,
        host_call(vec![vec_no], HostFunctions::VectorNew, vec![]),
    );

    let idx_no = vartab.temp_name("vec_i", &Type::Uint(32));
    cfg.add(
        vartab,
        Instr::Set {
            loc,
            res: idx_no,
            expr: Expression::NumberLiteral {
                loc,
                ty: Type::Uint(32),
                value: BigInt::zero(),
            },
        },
    );

    let cond_block = cfg.new_basic_block("vec_enc_cond".to_string());
    let body_block = cfg.new_basic_block("vec_enc_body".to_string());
    let end_block = cfg.new_basic_block("vec_enc_end".to_string());

    vartab.new_dirty_tracker();
    cfg.add(vartab, Instr::Branch { block: cond_block });

    cfg.set_basic_block(cond_block);
    let idx_var = Expression::Variable {
        loc,
        ty: Type::Uint(32),
        var_no: idx_no,
    };
    cfg.add(
        vartab,
        Instr::BranchCond {
            cond: Expression::Less {
                loc,
                signed: false,
                left: Box::new(idx_var.clone()),
                right: Box::new(len),
            },
            true_block: body_block,
            false_block: end_block,
        },
    );

    cfg.set_basic_block(body_block);
    let elem = if elem_ty.is_fixed_reference_type(ns) {
        Expression::Subscript {
            loc,
            ty: elem_ty.clone(),
            array_ty: item_ty.clone(),
            expr: Box::new(arr.clone()),
            index: Box::new(idx_var.clone()),
        }
    } else {
        Expression::Load {
            loc,
            ty: elem_ty.clone(),
            expr: Box::new(Expression::Subscript {
                loc,
                ty: Type::Ref(Box::new(elem_ty.clone())),
                array_ty: item_ty.clone(),
                expr: Box::new(arr.clone()),
                index: Box::new(idx_var.clone()),
            }),
        }
    };
    let encoded = if storage {
        soroban_storage_encode_arg(elem, cfg, vartab, ns)
    } else {
        soroban_encode_arg(elem, cfg, vartab, ns)
    };
    let prev_vec = Expression::Variable {
        loc,
        ty: Type::Uint(64),
        var_no: vec_no,
    };
    let pushed = vartab.temp_name("vec_pushed", &Type::Uint(64));
    cfg.add(
        vartab,
        host_call(
            vec![pushed],
            HostFunctions::VecPushBack,
            vec![prev_vec, encoded],
        ),
    );
    cfg.add(
        vartab,
        Instr::Set {
            loc,
            res: vec_no,
            expr: Expression::Variable {
                loc,
                ty: Type::Uint(64),
                var_no: pushed,
            },
        },
    );
    cfg.add(
        vartab,
        Instr::Set {
            loc,
            res: idx_no,
            expr: Expression::Add {
                loc,
                ty: Type::Uint(32),
                overflowing: false,
                left: Box::new(idx_var),
                right: Box::new(Expression::NumberLiteral {
                    loc,
                    ty: Type::Uint(32),
                    value: BigInt::from(1),
                }),
            },
        },
    );
    cfg.add(vartab, Instr::Branch { block: cond_block });

    cfg.set_basic_block(end_block);
    let phis = vartab.pop_dirty_tracker();
    cfg.set_phis(cond_block, phis.clone());
    cfg.set_phis(end_block, phis);

    Expression::Variable {
        loc,
        ty: Type::Uint(64),
        var_no: vec_no,
    }
}

fn decode_struct_storage(
    vec_object: Expression,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    struct_no: usize,
    ns: &Namespace,
    struct_ty: Type,
) -> Expression {
    let field_tys: Vec<Type> = ns.structs[struct_no]
        .fields
        .iter()
        .map(|f| f.ty.clone())
        .collect();

    let mut members = Vec::new();

    for (index, ty) in field_tys.iter().enumerate() {
        let idx_val = encode_object(
            Loc::Codegen,
            Expression::NumberLiteral {
                loc: Loc::Codegen,
                ty: Type::Uint(32),
                value: BigInt::from(index),
            },
            32,
            tags::U32,
        );

        let elem_no = vartab.temp_name("struct_field_val", &Type::Uint(64));
        cfg.add(
            vartab,
            Instr::Call {
                res: vec![elem_no],
                return_tys: vec![Type::Uint(64)],
                call: InternalCallTy::HostFunction {
                    name: HostFunctions::VecGet.name().to_string(),
                },
                args: vec![vec_object.clone(), idx_val],
            },
        );
        let elem = Expression::Variable {
            loc: Loc::Codegen,
            ty: Type::Uint(64),
            var_no: elem_no,
        };

        let decoded = soroban_storage_decode_arg(elem, cfg, vartab, ns, Some(ty.clone()));
        members.push(decoded);
    }

    Expression::StructLiteral {
        loc: Loc::Codegen,
        ty: struct_ty,
        values: members,
    }
}

fn decode_struct_map(
    arg: Expression,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    struct_no: usize,
    ns: &Namespace,
    struct_ty: Type,
) -> Expression {
    let loc = arg.loc();
    let fields = &ns.structs[struct_no].fields;
    let mut members = Vec::with_capacity(fields.len());

    for (index, field) in fields.iter().enumerate() {
        let name = field
            .id
            .as_ref()
            .map(|id| id.name.clone())
            .unwrap_or_else(|| index.to_string());
        let key = struct_field_key(&name, loc, cfg, vartab, ns);

        let val_var = vartab.temp_name("map_val", &Type::Uint(64));
        cfg.add(
            vartab,
            Instr::Call {
                res: vec![val_var],
                return_tys: vec![Type::Uint(64)],
                call: InternalCallTy::HostFunction {
                    name: HostFunctions::MapGet.name().to_string(),
                },
                args: vec![arg.clone(), key],
            },
        );
        let val = Expression::Variable {
            loc,
            ty: Type::Uint(64),
            var_no: val_var,
        };

        let decoded = soroban_decode_arg(val, cfg, vartab, ns, Some(field.ty.clone()));
        members.push(decoded);
    }

    Expression::StructLiteral {
        loc,
        ty: struct_ty,
        values: members,
    }
}

pub(crate) fn encode_as_symbol(
    item: Expression,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    ns: &Namespace,
) -> Expression {
    let loc = item.loc();

    let (ptr_u32val, len_u32val) = match &item {
        Expression::BytesLiteral { value, .. } => {
            let ptr = encode_object(
                loc,
                Expression::VectorData {
                    pointer: Box::new(item.clone()),
                },
                32,
                tags::U32,
            );
            let len = encode_object(
                loc,
                Expression::NumberLiteral {
                    loc,
                    ty: Type::Uint(32),
                    value: BigInt::from(value.len()),
                },
                32,
                tags::U32,
            );
            (ptr, len)
        }
        Expression::AllocDynamicBytes { size, .. } => {
            let inp = Expression::VectorData {
                pointer: Box::new(item.clone()),
            };

            let inp_extend = Expression::ZeroExt {
                loc: Loc::Codegen,
                ty: Type::Uint(64),
                expr: Box::new(inp),
            };

            let encoded = Expression::ShiftLeft {
                loc: Loc::Codegen,
                ty: Uint(64),
                left: Box::new(inp_extend),
                right: Box::new(Expression::NumberLiteral {
                    loc: Loc::Codegen,
                    ty: Type::Uint(64),
                    value: BigInt::from(32),
                }),
            };

            let encoded = Expression::Add {
                loc: Loc::Codegen,
                ty: Type::Uint(64),
                overflowing: true,
                left: Box::new(encoded),
                right: Box::new(Expression::NumberLiteral {
                    loc: Loc::Codegen,
                    ty: Type::Uint(64),
                    value: BigInt::from(4),
                }),
            };

            let sesa = Expression::ShiftLeft {
                loc: Loc::Codegen,
                ty: Uint(64),
                left: Box::new(size.clone().cast(&Type::Uint(64), ns)),
                right: Box::new(Expression::NumberLiteral {
                    loc: Loc::Codegen,
                    ty: Type::Uint(64),
                    value: BigInt::from(32),
                }),
            };

            let len = Expression::Add {
                loc: Loc::Codegen,
                ty: Type::Uint(64),
                overflowing: true,
                left: Box::new(sesa),
                right: Box::new(Expression::NumberLiteral {
                    loc: Loc::Codegen,
                    ty: Type::Uint(64),
                    value: BigInt::from(4),
                }),
            };
            (encoded, len)
        }
        _ => {
            unreachable!(
                "encode_as_symbol only accepts BytesLiteral :- {:?}",
                item.clone()
            );
        }
    };

    let sym_var = vartab.temp_name("symbol_obj", &Type::Uint(64));
    cfg.add(
        vartab,
        host_call(
            vec![sym_var],
            HostFunctions::SymbolNewFromLinearMemory,
            vec![ptr_u32val, len_u32val],
        ),
    );

    Expression::Variable {
        loc,
        ty: Type::Uint(64),
        var_no: sym_var,
    }
}

pub(crate) fn encode_object(loc: pt::Loc, value: Expression, shift: u64, tag: u64) -> Expression {
    let shifted = Expression::ShiftLeft {
        loc,
        ty: Type::Uint(64),
        left: Box::new(Expression::ZeroExt {
            loc,
            ty: Type::Uint(64),
            expr: Box::new(value),
        }),
        right: Box::new(Expression::NumberLiteral {
            loc,
            ty: Type::Uint(64),
            value: BigInt::from(shift),
        }),
    };

    Expression::Add {
        loc,
        ty: Type::Uint(64),
        left: Box::new(shifted),
        right: Box::new(Expression::NumberLiteral {
            loc,
            ty: Type::Uint(64),
            value: BigInt::from(tag),
        }),
        overflowing: false,
    }
}

fn host_call(res: Vec<usize>, host_fn: HostFunctions, args: Vec<Expression>) -> Instr {
    Instr::Call {
        res,
        return_tys: vec![Type::Uint(64)],
        call: InternalCallTy::HostFunction {
            name: host_fn.name().to_string(),
        },
        args,
    }
}

pub(crate) fn decode_object(loc: pt::Loc, tagged: Expression, shift: u64) -> Expression {
    Expression::Trunc {
        loc,
        ty: Type::Uint(32),
        expr: Box::new(Expression::ShiftRight {
            loc,
            ty: Type::Uint(64),
            left: Box::new(tagged),
            right: Box::new(Expression::NumberLiteral {
                loc,
                ty: Type::Uint(64),
                value: BigInt::from(shift),
            }),
            signed: false,
        }),
    }
}

pub(crate) fn decode_string(
    handle: Expression,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
) -> Expression {
    let loc = Loc::Codegen;

    let raw_len_var = vartab.temp_name("str_len_raw", &Type::Uint(64));
    cfg.add(
        vartab,
        host_call(
            vec![raw_len_var],
            HostFunctions::StringLen,
            vec![handle.clone()],
        ),
    );
    let raw_len = Expression::Variable {
        loc,
        ty: Type::Uint(64),
        var_no: raw_len_var,
    };

    let len_u32 = decode_object(loc, raw_len, 32);

    let buf_var = vartab.temp_name("str_buf", &Type::String);
    cfg.add(
        vartab,
        Instr::Set {
            loc,
            res: buf_var,
            expr: Expression::AllocDynamicBytes {
                loc,
                ty: Type::String,
                size: Box::new(len_u32.clone()),
                initializer: None,
            },
        },
    );
    let buf = Expression::Variable {
        loc,
        ty: Type::String,
        var_no: buf_var,
    };

    let lm_pos = encode_object(
        loc,
        Expression::VectorData {
            pointer: Box::new(buf.clone()),
        },
        32,
        tags::U32,
    );

    let src_pos = Expression::NumberLiteral {
        loc,
        ty: Type::Uint(64),
        value: BigInt::from(tags::U32),
    };

    let len_u32val = encode_object(loc, len_u32, 32, tags::U32);

    let unused = vartab.temp_name("str_copy_ret", &Type::Uint(64));
    cfg.add(
        vartab,
        host_call(
            vec![unused],
            HostFunctions::StringCopyToLinearMemory,
            vec![handle, src_pos, lm_pos, len_u32val],
        ),
    );

    buf
}

pub(crate) fn decode_bytes(
    handle: Expression,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
) -> Expression {
    let loc = Loc::Codegen;

    let raw_len_var = vartab.temp_name("bytes_len_raw", &Type::Uint(64));
    cfg.add(
        vartab,
        host_call(
            vec![raw_len_var],
            HostFunctions::BytesLen,
            vec![handle.clone()],
        ),
    );
    let raw_len = Expression::Variable {
        loc,
        ty: Type::Uint(64),
        var_no: raw_len_var,
    };

    let len_u32 = decode_object(loc, raw_len, 32);

    let buf_var = vartab.temp_name("bytes_buf", &Type::DynamicBytes);
    cfg.add(
        vartab,
        Instr::Set {
            loc,
            res: buf_var,
            expr: Expression::AllocDynamicBytes {
                loc,
                ty: Type::DynamicBytes,
                size: Box::new(len_u32.clone()),
                initializer: None,
            },
        },
    );
    let buf = Expression::Variable {
        loc,
        ty: Type::DynamicBytes,
        var_no: buf_var,
    };

    let lm_pos = encode_object(
        loc,
        Expression::VectorData {
            pointer: Box::new(buf.clone()),
        },
        32,
        tags::U32,
    );
    let src_pos = Expression::NumberLiteral {
        loc,
        ty: Type::Uint(64),
        value: BigInt::from(tags::U32), // U32Val(0)
    };
    let len_u32val = encode_object(loc, len_u32, 32, tags::U32);

    let unused = vartab.temp_name("bytes_copy_ret", &Type::Uint(64));
    cfg.add(
        vartab,
        host_call(
            vec![unused],
            HostFunctions::BytesCopyToLinearMemory,
            vec![handle, src_pos, lm_pos, len_u32val],
        ),
    );

    buf
}

fn decode_vector(
    vec_object: Expression,
    array_ty: &Type,
    ns: &Namespace,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    storage: bool,
) -> Expression {
    let loc = Loc::Codegen;
    let elem_ty = array_ty.array_elem();

    let handle_no = vartab.temp_name("vec_handle", &Type::Uint(64));
    cfg.add(
        vartab,
        Instr::Set {
            loc,
            res: handle_no,
            expr: vec_object,
        },
    );
    let handle = Expression::Variable {
        loc,
        ty: Type::Uint(64),
        var_no: handle_no,
    };

    let count_var = if let Some(fixed_len) = array_ty.array_length() {
        Expression::NumberLiteral {
            loc,
            ty: Type::Uint(32),
            value: fixed_len.clone(),
        }
    } else {
        let len_val = vartab.temp_name("vec_len", &Type::Uint(64));
        cfg.add(
            vartab,
            host_call(vec![len_val], HostFunctions::VecLen, vec![handle.clone()]),
        );
        let count = soroban_decode_arg(
            Expression::Variable {
                loc,
                ty: Type::Uint(64),
                var_no: len_val,
            },
            cfg,
            vartab,
            ns,
            Some(Type::Uint(32)),
        );
        let count_no = vartab.temp_name("vec_count", &Type::Uint(32));
        cfg.add(
            vartab,
            Instr::Set {
                loc,
                res: count_no,
                expr: count,
            },
        );
        Expression::Variable {
            loc,
            ty: Type::Uint(32),
            var_no: count_no,
        }
    };

    let buffer_no = vartab.temp_name("vec_decoded", array_ty);
    cfg.add(
        vartab,
        Instr::Set {
            loc,
            res: buffer_no,
            expr: Expression::AllocDynamicBytes {
                loc,
                ty: array_ty.clone(),
                size: Box::new(count_var.clone()),
                initializer: None,
            },
        },
    );
    let buffer = Expression::Variable {
        loc,
        ty: array_ty.clone(),
        var_no: buffer_no,
    };

    let idx_no = vartab.temp_name("vec_i", &Type::Uint(32));
    cfg.add(
        vartab,
        Instr::Set {
            loc,
            res: idx_no,
            expr: Expression::NumberLiteral {
                loc,
                ty: Type::Uint(32),
                value: BigInt::zero(),
            },
        },
    );

    let cond_block = cfg.new_basic_block("vec_dec_cond".to_string());
    let body_block = cfg.new_basic_block("vec_dec_body".to_string());
    let end_block = cfg.new_basic_block("vec_dec_end".to_string());

    vartab.new_dirty_tracker();
    cfg.add(vartab, Instr::Branch { block: cond_block });

    cfg.set_basic_block(cond_block);
    let idx_var = Expression::Variable {
        loc,
        ty: Type::Uint(32),
        var_no: idx_no,
    };
    cfg.add(
        vartab,
        Instr::BranchCond {
            cond: Expression::Less {
                loc,
                signed: false,
                left: Box::new(idx_var.clone()),
                right: Box::new(count_var),
            },
            true_block: body_block,
            false_block: end_block,
        },
    );

    cfg.set_basic_block(body_block);
    let idx_encoded = encode_object(loc, idx_var.clone(), 32, tags::U32);
    let elem_val = vartab.temp_name("vec_elem", &Type::Uint(64));
    cfg.add(
        vartab,
        host_call(
            vec![elem_val],
            HostFunctions::VecGet,
            vec![handle.clone(), idx_encoded],
        ),
    );
    let elem_handle = Expression::Variable {
        loc,
        ty: Type::Uint(64),
        var_no: elem_val,
    };
    let decoded = if storage {
        soroban_storage_decode_arg(elem_handle, cfg, vartab, ns, Some(elem_ty.clone()))
    } else {
        soroban_decode_arg(elem_handle, cfg, vartab, ns, Some(elem_ty.clone()))
    };
    let data = if elem_ty.is_fixed_reference_type(ns) {
        Expression::Load {
            loc,
            ty: elem_ty.clone(),
            expr: Box::new(decoded),
        }
    } else {
        decoded
    };
    cfg.add(
        vartab,
        Instr::Store {
            dest: Expression::Subscript {
                loc,
                ty: Type::Ref(Box::new(elem_ty.clone())),
                array_ty: array_ty.clone(),
                expr: Box::new(buffer.clone()),
                index: Box::new(idx_var.clone()),
            },
            data,
        },
    );
    cfg.add(
        vartab,
        Instr::Set {
            loc,
            res: idx_no,
            expr: Expression::Add {
                loc,
                ty: Type::Uint(32),
                overflowing: false,
                left: Box::new(idx_var),
                right: Box::new(Expression::NumberLiteral {
                    loc,
                    ty: Type::Uint(32),
                    value: BigInt::from(1),
                }),
            },
        },
    );
    cfg.add(vartab, Instr::Branch { block: cond_block });

    cfg.set_basic_block(end_block);
    let phis = vartab.pop_dirty_tracker();
    cfg.set_phis(cond_block, phis.clone());
    cfg.set_phis(end_block, phis);

    buffer
}
