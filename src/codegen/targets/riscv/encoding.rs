// SPDX-License-Identifier: Apache-2.0

//! Ethereum ABI encoding for the RISC-V target.
//!
//! r55 contracts are called with ordinary Ethereum calldata, so arguments and
//! return values use the Ethereum ABI rather than SCALE or Borsh.
//!
//! Only the static types are implemented: every value occupies exactly one
//! 32-byte word. The dynamic types (`string`, `bytes` and dynamic arrays)
//! additionally need head/tail offset encoding, which is not supported yet.
//!
//! Big-endian conversion comes for free: `Instr::WriteBuffer` and
//! `Builtin::ReadFromBuffer` byte-swap any value typed `Type::Bytes(n)`, so
//! values are widened to 256 bits and reinterpreted as `Type::Bytes(32)`.

use crate::codegen::cfg::{ControlFlowGraph, Instr};
use crate::codegen::interface::TargetCodegen;
use crate::codegen::targets::abi::{buffer_validator::BufferValidator, AbiEncoding};
use crate::codegen::vartable::Vartable;
use crate::codegen::{Builtin, Expression};
use crate::sema::ast::{Namespace, RetrieveType, StructType, Type, Type::Uint};
use num_bigint::BigInt;
use solang_parser::pt::Loc::Codegen;
use std::collections::HashMap;

/// The size of a single ABI word.
const WORD: u64 = 32;

pub(crate) struct EthAbiEncoding {
    storage_cache: HashMap<usize, Expression>,
}

impl EthAbiEncoding {
    pub fn new() -> Self {
        Self {
            storage_cache: HashMap::new(),
        }
    }

    /// A `Uint(32)` literal holding the width of one ABI word.
    fn word_size() -> Expression {
        Expression::NumberLiteral {
            loc: Codegen,
            ty: Uint(32),
            value: BigInt::from(WORD),
        }
    }

    /// Widen `expr` to a 256-bit integer, preserving its value.
    fn widen(expr: &Expression, ns: &Namespace) -> Expression {
        let ty = expr.ty().unwrap_user_type(ns);

        match &ty {
            // An address is an array of bytes rather than an integer, but
            // casting it to an integer already performs the big-endian
            // conversion and leaves it right-aligned, which is what the ABI
            // wants.
            Type::Address(_) | Type::Contract(_) => Expression::Cast {
                loc: Codegen,
                ty: Uint(256),
                expr: expr.clone().into(),
            },
            Type::Int(_) => Expression::SignExt {
                loc: Codegen,
                ty: Type::Int(256),
                expr: expr.clone().into(),
            },
            // `bytesN` is left-aligned, so it is shifted up into the high
            // bytes of the word.
            Type::Bytes(n) if *n < 32 => {
                let widened = Expression::ZeroExt {
                    loc: Codegen,
                    ty: Uint(256),
                    expr: expr.clone().into(),
                };
                Expression::ShiftLeft {
                    loc: Codegen,
                    ty: Uint(256),
                    left: widened.into(),
                    right: Expression::NumberLiteral {
                        loc: Codegen,
                        ty: Uint(256),
                        value: BigInt::from((WORD - *n as u64) * 8),
                    }
                    .into(),
                }
            }
            Type::Bytes(_) => expr.clone(),
            Type::Uint(256) => expr.clone(),
            _ => Expression::ZeroExt {
                loc: Codegen,
                ty: Uint(256),
                expr: expr.clone().into(),
            },
        }
    }

    /// Reinterpret a 256-bit integer as `Type::Bytes(32)` so that writing it
    /// emits big-endian bytes.
    fn as_word(expr: Expression) -> Expression {
        Expression::Cast {
            loc: Codegen,
            ty: Type::Bytes(32),
            expr: expr.into(),
        }
    }

    fn unsupported(ty: &Type) -> ! {
        unimplemented!(
            "the RISC-V target does not support ABI encoding {ty:?} yet: \
             only statically sized types are implemented"
        )
    }
}

impl AbiEncoding for EthAbiEncoding {
    fn size_width(
        &self,
        _size: &Expression,
        _vartab: &mut Vartable,
        _cfg: &mut ControlFlowGraph,
    ) -> Expression {
        unimplemented!("dynamically sized types are not supported on RISC-V yet")
    }

    fn encode(
        &mut self,
        expr: &Expression,
        buffer: &Expression,
        offset: &Expression,
        _arg_no: usize,
        ns: &Namespace,
        vartab: &mut Vartable,
        cfg: &mut ControlFlowGraph,
    ) -> Expression {
        let ty = expr.ty().unwrap_user_type(ns);

        match &ty {
            Type::Uint(_)
            | Type::Int(_)
            | Type::Bool
            | Type::Enum(_)
            | Type::Value
            | Type::Address(_)
            | Type::Contract(_)
            | Type::Bytes(_) => {
                let value = Self::as_word(Self::widen(expr, ns));

                cfg.add(
                    vartab,
                    Instr::WriteBuffer {
                        buf: buffer.clone(),
                        offset: offset.clone(),
                        value,
                    },
                );

                Self::word_size()
            }
            Type::Ref(inner) => {
                let loaded = Expression::Load {
                    loc: Codegen,
                    ty: *inner.clone(),
                    expr: expr.clone().into(),
                };
                self.encode(&loaded, buffer, offset, _arg_no, ns, vartab, cfg)
            }
            Type::StorageRef(..) => {
                let loaded = self.storage_cache_remove(_arg_no).unwrap();
                self.encode(&loaded, buffer, offset, _arg_no, ns, vartab, cfg)
            }
            _ => Self::unsupported(&ty),
        }
    }

    fn encode_size(
        &mut self,
        _expr: &Expression,
        _buffer: &Expression,
        _offset: &Expression,
        _ns: &Namespace,
        _vartab: &mut Vartable,
        _cfg: &mut ControlFlowGraph,
    ) -> Expression {
        unimplemented!("dynamically sized types are not supported on RISC-V yet")
    }

    fn read_from_buffer(
        &self,
        buffer: &Expression,
        offset: &Expression,
        ty: &Type,
        validator: &mut BufferValidator,
        ns: &Namespace,
        vartab: &mut Vartable,
        cfg: &mut ControlFlowGraph,
    ) -> (Expression, Expression) {
        let ty = ty.clone().unwrap_user_type(ns);
        let size = Self::word_size();

        match &ty {
            Type::Uint(_)
            | Type::Int(_)
            | Type::Bool
            | Type::Enum(_)
            | Type::Value
            | Type::Address(_)
            | Type::Contract(_)
            | Type::Bytes(_) => {
                validator.validate_offset_plus_size(offset, &size, ns, vartab, cfg);

                // Reading as `Bytes(32)` converts the word from big-endian.
                let word = Expression::Builtin {
                    loc: Codegen,
                    tys: vec![Type::Bytes(32)],
                    kind: Builtin::ReadFromBuffer,
                    args: vec![buffer.clone(), offset.clone()],
                };
                let word = Expression::Cast {
                    loc: Codegen,
                    ty: Uint(256),
                    expr: word.into(),
                };

                let value = match &ty {
                    Type::Uint(256) | Type::Int(256) => word,
                    // `bytesN` sits in the high bytes of the word.
                    Type::Bytes(n) if *n < 32 => {
                        let shifted = Expression::ShiftRight {
                            loc: Codegen,
                            ty: Uint(256),
                            left: word.into(),
                            right: Expression::NumberLiteral {
                                loc: Codegen,
                                ty: Uint(256),
                                value: BigInt::from((WORD - *n as u64) * 8),
                            }
                            .into(),
                            signed: false,
                        };
                        Expression::Trunc {
                            loc: Codegen,
                            ty: ty.clone(),
                            expr: shifted.into(),
                        }
                    }
                    Type::Address(_) | Type::Contract(_) => Expression::Cast {
                        loc: Codegen,
                        ty: ty.clone(),
                        expr: word.into(),
                    },
                    _ => Expression::Trunc {
                        loc: Codegen,
                        ty: ty.clone(),
                        expr: word.into(),
                    },
                };

                let read_var = vartab.temp_anonymous(&ty);
                cfg.add(
                    vartab,
                    Instr::Set {
                        loc: Codegen,
                        res: read_var,
                        expr: value,
                    },
                );

                let read_expr = Expression::Variable {
                    loc: Codegen,
                    ty: ty.clone(),
                    var_no: read_var,
                };

                (read_expr, size)
            }
            _ => Self::unsupported(&ty),
        }
    }

    fn get_expr_size(
        &mut self,
        _arg_no: usize,
        expr: &Expression,
        ns: &Namespace,
        _vartab: &mut Vartable,
        _cfg: &mut ControlFlowGraph,
        _target: &dyn TargetCodegen,
    ) -> Expression {
        let ty = expr.ty().unwrap_user_type(ns);

        match &ty {
            Type::Uint(_)
            | Type::Int(_)
            | Type::Bool
            | Type::Enum(_)
            | Type::Value
            | Type::Address(_)
            | Type::Contract(_)
            | Type::Bytes(_)
            | Type::Ref(..)
            | Type::StorageRef(..) => Self::word_size(),
            _ => Self::unsupported(&ty),
        }
    }

    fn calculate_struct_size(
        &mut self,
        _arg_no: usize,
        _expr: &Expression,
        struct_ty: &StructType,
        _ns: &Namespace,
        _vartab: &mut Vartable,
        _cfg: &mut ControlFlowGraph,
        _target: &dyn TargetCodegen,
    ) -> Expression {
        unimplemented!("the RISC-V target cannot ABI encode struct {struct_ty:?} yet")
    }

    fn encode_external_function(
        &mut self,
        _expr: &Expression,
        _buffer: &Expression,
        _offset: &Expression,
        _ns: &Namespace,
        _vartab: &mut Vartable,
        _cfg: &mut ControlFlowGraph,
    ) -> Expression {
        unimplemented!("the RISC-V target cannot ABI encode external functions yet")
    }

    fn decode_external_function(
        &self,
        _buffer: &Expression,
        _offset: &Expression,
        _ty: &Type,
        _validator: &mut BufferValidator,
        _ns: &Namespace,
        _vartab: &mut Vartable,
        _cfg: &mut ControlFlowGraph,
    ) -> (Expression, Expression) {
        unimplemented!("the RISC-V target cannot ABI decode external functions yet")
    }

    fn retrieve_array_length(
        &self,
        _buffer: &Expression,
        _offset: &Expression,
        _vartab: &mut Vartable,
        _cfg: &mut ControlFlowGraph,
    ) -> (usize, Expression) {
        unimplemented!("dynamically sized arrays are not supported on RISC-V yet")
    }

    fn calculate_string_size(
        &self,
        _expr: &Expression,
        _vartab: &mut Vartable,
        _cfg: &mut ControlFlowGraph,
    ) -> Expression {
        unimplemented!("dynamically sized types are not supported on RISC-V yet")
    }

    fn storage_cache_insert(&mut self, arg_no: usize, expr: Expression) {
        self.storage_cache.insert(arg_no, expr);
    }

    fn storage_cache_remove(&mut self, arg_no: usize) -> Option<Expression> {
        self.storage_cache.remove(&arg_no)
    }

    fn is_packed(&self) -> bool {
        false
    }

    /// Constant-fold the encoding of revert data such as `Panic(uint256)`,
    /// which is a 4 byte selector followed by one 32-byte word.
    fn const_encode(&self, args: &[Expression]) -> Option<Vec<u8>> {
        let mut result = Vec::new();

        for arg in args {
            match arg {
                Expression::NumberLiteral { ty, value, .. } => {
                    let width = match ty {
                        // The selector is written as-is rather than padded to
                        // a word.
                        Type::Bytes(n) => *n as usize,
                        Type::Uint(_) | Type::Int(_) => WORD as usize,
                        _ => return None,
                    };

                    let (_, bytes) = value.to_bytes_be();
                    if bytes.len() > width {
                        return None;
                    }

                    result.extend(std::iter::repeat_n(0, width - bytes.len()));
                    result.extend_from_slice(&bytes);
                }
                _ => return None,
            }
        }

        Some(result)
    }
}
