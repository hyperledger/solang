// SPDX-License-Identifier: Apache-2.0

/// Any supported encoding scheme should be implemented here.
/// The module is organized as follows:
///
/// - `fn abi_encode()` and `fn abi_decode()` are entry points for wherever there is
///   something to be encoded or decoded.
/// - `AbiEncoding` defines the encoding and decoding API and must be implemented by all schemes.
/// - There are some helper functions to work with more complex types.
///   Any such helper function should work fine regardless of the encoding scheme being used.
mod borsh_encoding;
mod buffer_validator;
pub(super) mod scale_encoding;

use crate::codegen::cfg::{ControlFlowGraph, Instr};
use crate::codegen::encoding::borsh_encoding::BorshEncoding;
use crate::codegen::encoding::scale_encoding::ScaleEncoding;
use crate::codegen::expression::load_storage;
use crate::codegen::vartable::Vartable;
use crate::codegen::{Builtin, Expression};
use crate::sema::ast::{ArrayLength, Namespace, RetrieveType, StructType, Type, Type::Uint};
use crate::Target;
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Zero};
use solang_parser::pt::{Loc, Loc::Codegen};
use std::ops::{AddAssign, MulAssign, Sub};

use self::buffer_validator::BufferValidator;

/// Insert encoding instructions into the `cfg` for any `Expression` in `args`.
/// Returns a pointer to the encoded data and the size as a 32bit integer.
pub(super) fn abi_encode(
    loc: &Loc,
    args: Vec<Expression>,
    ns: &Namespace,
    vartab: &mut Vartable,
    cfg: &mut ControlFlowGraph,
    packed: bool,
) -> (Expression, Expression) {
    let mut encoder = create_encoder(ns, packed);
    let size = calculate_size_args(&mut encoder, &args, ns, vartab, cfg);
    let encoded_bytes = vartab.temp_name("abi_encoded", &Type::DynamicBytes);
    let expr = Expression::AllocDynamicBytes {
        loc: *loc,
        ty: Type::DynamicBytes,
        size: size.clone().into(),
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
        ty: Uint(32),
        value: BigInt::zero(),
    };
    let buffer = Expression::Variable {
        loc: *loc,
        ty: Type::DynamicBytes,
        var_no: encoded_bytes,
    };
    for (arg_no, item) in args.iter().enumerate() {
        let advance = encoder.encode(item, &buffer, &offset, arg_no, ns, vartab, cfg);
        offset = Expression::Add {
            loc: *loc,
            ty: Uint(32),
            overflowing: false,
            left: offset.into(),
            right: advance.into(),
        };
    }
    (buffer, size)
}

/// Insert decoding routines into the `cfg` for the `Expression`s in `args`.
/// Returns a vector containing the encoded data.
pub(super) fn abi_decode(
    loc: &Loc,
    buffer: &Expression,
    types: &[Type],
    ns: &Namespace,
    vartab: &mut Vartable,
    cfg: &mut ControlFlowGraph,
    buffer_size_expr: Option<Expression>,
) -> Vec<Expression> {
    let buffer_size = vartab.temp_anonymous(&Uint(32));
    if let Some(length_expression) = buffer_size_expr {
        cfg.add(
            vartab,
            Instr::Set {
                loc: Codegen,
                res: buffer_size,
                expr: length_expression,
            },
        );
    } else {
        cfg.add(
            vartab,
            Instr::Set {
                loc: Codegen,
                res: buffer_size,
                expr: Expression::Builtin {
                    loc: Codegen,
                    tys: vec![Uint(32)],
                    kind: Builtin::ArrayLength,
                    args: vec![buffer.clone()],
                },
            },
        );
    }

    let mut validator = BufferValidator::new(buffer_size, types);

    let mut read_items: Vec<Expression> = vec![Expression::Poison; types.len()];
    let mut offset = Expression::NumberLiteral {
        loc: *loc,
        ty: Uint(32),
        value: BigInt::zero(),
    };

    validator.initialize_validation(&offset, ns, vartab, cfg);
    let encoder = create_encoder(ns, false);

    for (item_no, item) in types.iter().enumerate() {
        validator.set_argument_number(item_no);
        validator.validate_buffer(&offset, ns, vartab, cfg);
        let (read_item, advance) =
            encoder.read_from_buffer(buffer, &offset, item, &mut validator, ns, vartab, cfg);
        read_items[item_no] = read_item;
        offset = Expression::Add {
            loc: *loc,
            ty: Uint(32),
            overflowing: false,
            left: Box::new(offset),
            right: Box::new(advance),
        };
    }

    validator.validate_all_bytes_read(offset, ns, vartab, cfg);

    read_items
}

/// Calculate the size of a set of arguments to encoding functions
fn calculate_size_args(
    encoder: &mut Box<dyn AbiEncoding>,
    args: &[Expression],
    ns: &Namespace,
    vartab: &mut Vartable,
    cfg: &mut ControlFlowGraph,
) -> Expression {
    let mut size = encoder.get_expr_size(0, &args[0], ns, vartab, cfg);
    for (i, item) in args.iter().enumerate().skip(1) {
        let additional = encoder.get_expr_size(i, item, ns, vartab, cfg);
        size = Expression::Add {
            loc: Codegen,
            ty: Uint(32),
            overflowing: false,
            left: size.into(),
            right: additional.into(),
        };
    }
    size
}

/// This trait should be implemented by all encoding methods (ethabi, SCALE and Borsh), so that
/// we have the same interface for creating encode and decode functions.
///
/// Note: This trait mostly reflects the situation around SCALE and Borsh encoding schemes.
/// These two encoding schemes share only minor differences. We provide default implementations
/// for many methods, which properly work for SCALE and Borsh encoding.
///
/// However, this might be less suitable for schemas vastly different than SCALE or Borsh.
/// In the worst case scenario, you need to provide your own implementation of `fn encode(..)`,
/// which effectively means implementing the encoding logic for any given sema `Type` on your own.
pub(crate) trait AbiEncoding {
    /// The width (in bits) used in size hints for dynamic size types.
    fn size_width(
        &self,
        size: &Expression,
        vartab: &mut Vartable,
        cfg: &mut ControlFlowGraph,
    ) -> Expression;

    /// Provide generic encoding for any given `expr` into `buffer`, depending on its `Type`.
    /// Relies on the methods encoding individual expressions (`encode_*`) to return the encoded size.
    fn encode(
        &mut self,
        expr: &Expression,
        buffer: &Expression,
        offset: &Expression,
        arg_no: usize,
        ns: &Namespace,
        vartab: &mut Vartable,
        cfg: &mut ControlFlowGraph,
    ) -> Expression {
        let expr_ty = &expr.ty().unwrap_user_type(ns);
        match expr_ty {
            Type::Contract(_) | Type::Address(_) => {
                self.encode_directly(expr, buffer, offset, vartab, cfg, ns.address_length.into())
            }
            Type::Bool => self.encode_directly(expr, buffer, offset, vartab, cfg, 1.into()),
            Type::Uint(width) | Type::Int(width) => {
                self.encode_int(expr, buffer, offset, ns, vartab, cfg, *width)
            }
            Type::Value => {
                self.encode_directly(expr, buffer, offset, vartab, cfg, ns.value_length.into())
            }
            Type::Bytes(length) => {
                self.encode_directly(expr, buffer, offset, vartab, cfg, (*length).into())
            }
            Type::String | Type::DynamicBytes => {
                self.encode_bytes(expr, buffer, offset, ns, vartab, cfg)
            }
            Type::Enum(_) => self.encode_directly(expr, buffer, offset, vartab, cfg, 1.into()),
            Type::Struct(ty) => {
                self.encode_struct(expr, buffer, offset.clone(), ty, arg_no, ns, vartab, cfg)
            }
            Type::Slice(ty) => {
                let dims = &[ArrayLength::Dynamic];
                self.encode_array(
                    expr, expr_ty, ty, dims, arg_no, buffer, offset, ns, vartab, cfg,
                )
            }
            Type::Array(ty, dims) => self.encode_array(
                expr, expr_ty, ty, dims, arg_no, buffer, offset, ns, vartab, cfg,
            ),
            Type::ExternalFunction { .. } => {
                self.encode_external_function(expr, buffer, offset, ns, vartab, cfg)
            }
            Type::FunctionSelector => {
                let size = ns.target.selector_length().into();
                self.encode_directly(expr, buffer, offset, vartab, cfg, size)
            }
            Type::Ref(r) => {
                if let Type::Struct(ty) = &**r {
                    // Structs references should not be dereferenced
                    return self.encode_struct(
                        expr,
                        buffer,
                        offset.clone(),
                        ty,
                        arg_no,
                        ns,
                        vartab,
                        cfg,
                    );
                }
                let loaded = Expression::Load {
                    loc: Codegen,
                    ty: *r.clone(),
                    expr: expr.clone().into(),
                };
                self.encode(&loaded, buffer, offset, arg_no, ns, vartab, cfg)
            }
            Type::StorageRef(..) => {
                let loaded = self.storage_cache_remove(arg_no).unwrap();
                self.encode(&loaded, buffer, offset, arg_no, ns, vartab, cfg)
            }
            Type::UserType(_) | Type::Unresolved | Type::Rational | Type::Unreachable => {
                unreachable!("Type should not exist in codegen")
            }
            Type::InternalFunction { .. }
            | Type::Void
            | Type::BufferPointer
            | Type::Mapping(..) => unreachable!("This type cannot be encoded"),
        }
    }

    /// Write whatever is inside the given `expr` into `buffer` without any modification.
    fn encode_directly(
        &mut self,
        expr: &Expression,
        buffer: &Expression,
        offset: &Expression,
        vartab: &mut Vartable,
        cfg: &mut ControlFlowGraph,
        size: BigInt,
    ) -> Expression {
        cfg.add(
            vartab,
            Instr::WriteBuffer {
                buf: buffer.clone(),
                offset: offset.clone(),
                value: expr.clone(),
            },
        );
        Expression::NumberLiteral {
            loc: Codegen,
            ty: Uint(32),
            value: size,
        }
    }

    /// Encode `expr` into `buffer` as an integer.
    fn encode_int(
        &mut self,
        expr: &Expression,
        buffer: &Expression,
        offset: &Expression,
        ns: &Namespace,
        vartab: &mut Vartable,
        cfg: &mut ControlFlowGraph,
        width: u16,
    ) -> Expression {
        let encoding_size = width.next_power_of_two();
        let expr = if encoding_size != width {
            if expr.ty().is_signed_int(ns) {
                Expression::SignExt {
                    loc: Codegen,
                    ty: Type::Int(encoding_size),
                    expr: expr.clone().into(),
                }
            } else {
                Expression::ZeroExt {
                    loc: Codegen,
                    ty: Type::Uint(encoding_size),
                    expr: expr.clone().into(),
                }
            }
        } else {
            expr.clone()
        };

        cfg.add(
            vartab,
            Instr::WriteBuffer {
                buf: buffer.clone(),
                offset: offset.clone(),
                value: expr,
            },
        );

        Expression::NumberLiteral {
            loc: Codegen,
            ty: Uint(32),
            value: (encoding_size / 8).into(),
        }
    }

    /// Encode `expr` into `buffer` as size hint for dynamically sized datastructures.
    fn encode_size(
        &mut self,
        expr: &Expression,
        buffer: &Expression,
        offset: &Expression,
        ns: &Namespace,
        vartab: &mut Vartable,
        cfg: &mut ControlFlowGraph,
    ) -> Expression;

    /// Encode `expr` into `buffer` as bytes.
    fn encode_bytes(
        &mut self,
        expr: &Expression,
        buffer: &Expression,
        offset: &Expression,
        ns: &Namespace,
        vartab: &mut Vartable,
        cfg: &mut ControlFlowGraph,
    ) -> Expression {
        let len = array_outer_length(expr, vartab, cfg);
        let (data_offset, size) = if self.is_packed() {
            (offset.clone(), None)
        } else {
            let size = self.encode_size(&len, buffer, offset, ns, vartab, cfg);
            (offset.clone().add_u32(size.clone()), Some(size))
        };
        // ptr + offset + size_of_integer
        let dest_address = Expression::AdvancePointer {
            pointer: buffer.clone().into(),
            bytes_offset: data_offset.into(),
        };
        cfg.add(
            vartab,
            Instr::MemCopy {
                source: expr.clone(),
                destination: dest_address,
                bytes: len.clone(),
            },
        );
        if let Some(size) = size {
            len.add_u32(size)
        } else {
            len
        }
    }

    /// Encode `expr` into `buffer` as a struct type.
    fn encode_struct(
        &mut self,
        expr: &Expression,
        buffer: &Expression,
        mut offset: Expression,
        struct_ty: &StructType,
        arg_no: usize,
        ns: &Namespace,
        vartab: &mut Vartable,
        cfg: &mut ControlFlowGraph,
    ) -> Expression {
        let size = ns.calculate_struct_non_padded_size(struct_ty);
        // If the size without padding equals the size with padding, memcpy this struct directly.
        if let Some(no_padding_size) = size.as_ref().filter(|no_pad| {
            *no_pad == &struct_ty.struct_padded_size(ns) && allow_memcpy(&expr.ty(), ns)
        }) {
            let size = Expression::NumberLiteral {
                loc: Codegen,
                ty: Uint(32),
                value: no_padding_size.clone(),
            };
            let dest_address = Expression::AdvancePointer {
                pointer: buffer.clone().into(),
                bytes_offset: offset.into(),
            };
            cfg.add(
                vartab,
                Instr::MemCopy {
                    source: expr.clone(),
                    destination: dest_address,
                    bytes: size.clone(),
                },
            );
            return size;
        }
        let size = size.map(|no_pad| Expression::NumberLiteral {
            loc: Codegen,
            ty: Uint(32),
            value: no_pad,
        });

        let qty = struct_ty.definition(ns).fields.len();
        let first_ty = struct_ty.definition(ns).fields[0].ty.clone();
        let loaded = load_struct_member(first_ty, expr.clone(), 0, ns);

        let mut advance = self.encode(&loaded, buffer, &offset, arg_no, ns, vartab, cfg);
        let mut runtime_size = advance.clone();
        for i in 1..qty {
            let ith_type = struct_ty.definition(ns).fields[i].ty.clone();
            offset = Expression::Add {
                loc: Codegen,
                ty: Uint(32),
                overflowing: false,
                left: offset.clone().into(),
                right: advance.into(),
            };
            let loaded = load_struct_member(ith_type.clone(), expr.clone(), i, ns);
            // After fetching the struct member, we can encode it
            advance = self.encode(&loaded, buffer, &offset, arg_no, ns, vartab, cfg);
            runtime_size = Expression::Add {
                loc: Codegen,
                ty: Uint(32),
                overflowing: false,
                left: runtime_size.into(),
                right: advance.clone().into(),
            };
        }

        size.unwrap_or(runtime_size)
    }

    /// Encode `expr` into `buffer` as an array.
    fn encode_array(
        &mut self,
        array: &Expression,
        array_ty: &Type,
        elem_ty: &Type,
        dims: &[ArrayLength],
        arg_no: usize,
        buffer: &Expression,
        offset: &Expression,
        ns: &Namespace,
        vartab: &mut Vartable,
        cfg: &mut ControlFlowGraph,
    ) -> Expression {
        assert!(!dims.is_empty());

        if allow_memcpy(array_ty, ns) {
            // Calculate number of elements
            let (bytes_size, offset, size_length) =
                if matches!(dims.last(), Some(&ArrayLength::Fixed(_))) {
                    let elem_no = calculate_direct_copy_bytes_size(dims, elem_ty, ns);
                    (
                        Expression::NumberLiteral {
                            loc: Codegen,
                            ty: Uint(32),
                            value: elem_no,
                        },
                        offset.clone(),
                        None,
                    )
                } else {
                    let value = array_outer_length(array, vartab, cfg);

                    let (new_offset, size_length) = if self.is_packed() {
                        (offset.clone(), None)
                    } else {
                        let encoded_size =
                            self.encode_size(&value, buffer, offset, ns, vartab, cfg);
                        (
                            offset.clone().add_u32(encoded_size.clone()),
                            Some(encoded_size),
                        )
                    };

                    if let Expression::Variable {
                        var_no: size_temp, ..
                    } = value
                    {
                        let size = calculate_array_bytes_size(size_temp, elem_ty, ns);
                        (size, new_offset, size_length)
                    } else {
                        unreachable!()
                    }
                };

            let dest_address = Expression::AdvancePointer {
                pointer: buffer.clone().into(),
                bytes_offset: offset.into(),
            };

            cfg.add(
                vartab,
                Instr::MemCopy {
                    source: array.clone(),
                    destination: dest_address,
                    bytes: bytes_size.clone(),
                },
            );

            // If the array is dynamic, we have written into the buffer its size and its elements
            return match (size_length, self.is_packed()) {
                (Some(len), false) => Expression::Add {
                    loc: Codegen,
                    ty: Uint(32),
                    overflowing: false,
                    left: bytes_size.into(),
                    right: len.into(),
                },
                _ => bytes_size,
            };
        }

        // In all other cases, we must loop through the array
        let mut indexes: Vec<usize> = Vec::new();
        let offset_var_no = vartab.temp_anonymous(&Uint(32));
        cfg.add(
            vartab,
            Instr::Set {
                loc: Codegen,
                res: offset_var_no,
                expr: offset.clone(),
            },
        );
        self.encode_complex_array(
            array,
            arg_no,
            dims,
            buffer,
            offset_var_no,
            dims.len() - 1,
            ns,
            vartab,
            cfg,
            &mut indexes,
        );

        // The offset variable minus the original offset obtains the vector size in bytes
        let offset_var = Expression::Variable {
            loc: Codegen,
            ty: Uint(32),
            var_no: offset_var_no,
        }
        .into();
        let sub = Expression::Subtract {
            loc: Codegen,
            ty: Uint(32),
            overflowing: false,
            left: offset_var,
            right: offset.clone().into(),
        };
        cfg.add(
            vartab,
            Instr::Set {
                loc: Codegen,
                res: offset_var_no,
                expr: sub,
            },
        );
        Expression::Variable {
            loc: Codegen,
            ty: Uint(32),
            var_no: offset_var_no,
        }
    }

    /// Encode `expr` into `buffer` as a complex array.
    /// This function indexes an array from its outer dimension to its inner one.
    ///
    /// Note: In the default implementation, `encode_array` decides when to use this method for you.
    fn encode_complex_array(
        &mut self,
        arr: &Expression,
        arg_no: usize,
        dims: &[ArrayLength],
        buffer: &Expression,
        offset_var: usize,
        dimension: usize,
        ns: &Namespace,
        vartab: &mut Vartable,
        cfg: &mut ControlFlowGraph,
        indexes: &mut Vec<usize>,
    ) {
        // If this dimension is dynamic, we must save its length before all elements
        if dims[dimension] == ArrayLength::Dynamic && !self.is_packed() {
            // TODO: This is wired up for the support of dynamic multidimensional arrays, like
            // TODO: 'int[3][][4] vec', but it needs testing, as soon as Solang works with them.
            // TODO: A discussion about this is under way here: https://github.com/hyperledger-solang/solang/issues/932
            // We only support dynamic arrays whose non-constant length is the outer one.
            let sub_array = index_array(arr.clone(), dims, indexes, false);

            let size = Expression::Builtin {
                loc: Codegen,
                tys: vec![Uint(32)],
                kind: Builtin::ArrayLength,
                args: vec![sub_array],
            };

            let offset_expr = Expression::Variable {
                loc: Codegen,
                ty: Uint(32),
                var_no: offset_var,
            };
            let encoded_size = self.encode_size(&size, buffer, &offset_expr, ns, vartab, cfg);
            cfg.add(
                vartab,
                Instr::Set {
                    loc: Codegen,
                    res: offset_var,
                    expr: offset_expr.add_u32(encoded_size),
                },
            );
        }
        let for_loop = set_array_loop(arr, dims, dimension, indexes, vartab, cfg);
        cfg.set_basic_block(for_loop.body_block);
        if 0 == dimension {
            // If we are indexing the last dimension, we have an element, so we can encode it.
            let deref = index_array(arr.clone(), dims, indexes, false);
            let offset_expr = Expression::Variable {
                loc: Codegen,
                ty: Uint(32),
                var_no: offset_var,
            };
            let elem_size = self.encode(&deref, buffer, &offset_expr, arg_no, ns, vartab, cfg);
            cfg.add(
                vartab,
                Instr::Set {
                    loc: Codegen,
                    res: offset_var,
                    expr: Expression::Add {
                        loc: Codegen,
                        ty: Uint(32),
                        overflowing: false,
                        left: elem_size.into(),
                        right: offset_expr.into(),
                    },
                },
            );
        } else {
            self.encode_complex_array(
                arr,
                arg_no,
                dims,
                buffer,
                offset_var,
                dimension - 1,
                ns,
                vartab,
                cfg,
                indexes,
            )
        }

        finish_array_loop(&for_loop, vartab, cfg);
    }

    /// Encode `expr` into `buffer` as an external function pointer.
    fn encode_external_function(
        &mut self,
        expr: &Expression,
        buffer: &Expression,
        offset: &Expression,
        ns: &Namespace,
        vartab: &mut Vartable,
        cfg: &mut ControlFlowGraph,
    ) -> Expression;

    /// Read a value of type 'ty' from the buffer at a given offset. Returns an expression
    /// containing the read value and the number of bytes read.
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
        match ty {
            Type::Uint(width) | Type::Int(width) => {
                let encoding_size = width.next_power_of_two();

                let size = Expression::NumberLiteral {
                    loc: Codegen,
                    ty: Uint(32),
                    value: (encoding_size / 8).into(),
                };
                validator.validate_offset_plus_size(offset, &size, ns, vartab, cfg);

                let read_value = Expression::Builtin {
                    loc: Codegen,
                    tys: vec![ty.clone()],
                    kind: Builtin::ReadFromBuffer,
                    args: vec![buffer.clone(), offset.clone()],
                };
                let read_var = vartab.temp_anonymous(ty);

                cfg.add(
                    vartab,
                    Instr::Set {
                        loc: Codegen,
                        res: read_var,
                        expr: if encoding_size == *width {
                            read_value
                        } else {
                            Expression::Trunc {
                                loc: Codegen,
                                ty: ty.clone(),
                                expr: Box::new(read_value),
                            }
                        },
                    },
                );

                let read_expr = Expression::Variable {
                    loc: Codegen,
                    ty: ty.clone(),
                    var_no: read_var,
                };
                (read_expr, size)
            }

            Type::Bool
            | Type::Address(_)
            | Type::Contract(_)
            | Type::Enum(_)
            | Type::Value
            | Type::Bytes(_) => {
                let read_bytes = ty.memory_size_of(ns);

                let size = Expression::NumberLiteral {
                    loc: Codegen,
                    ty: Uint(32),
                    value: read_bytes,
                };
                validator.validate_offset_plus_size(offset, &size, ns, vartab, cfg);

                let read_value = Expression::Builtin {
                    loc: Codegen,
                    tys: vec![ty.clone()],
                    kind: Builtin::ReadFromBuffer,
                    args: vec![buffer.clone(), offset.clone()],
                };

                let read_var = vartab.temp_anonymous(ty);
                cfg.add(
                    vartab,
                    Instr::Set {
                        loc: Codegen,
                        res: read_var,
                        expr: read_value,
                    },
                );

                let read_expr = Expression::Variable {
                    loc: Codegen,
                    ty: ty.clone(),
                    var_no: read_var,
                };

                (read_expr, size)
            }

            Type::DynamicBytes | Type::String => {
                // String and Dynamic bytes are encoded as size + elements
                let (array_length_var, size_length) =
                    self.retrieve_array_length(buffer, offset, vartab, cfg);
                let array_start = offset.clone().add_u32(size_length.clone());
                validator.validate_offset(array_start.clone(), ns, vartab, cfg);
                let array_length = Expression::Variable {
                    loc: Codegen,
                    ty: Uint(32),
                    var_no: array_length_var,
                };
                let total_size = array_length.clone().add_u32(size_length);
                validator.validate_offset(
                    offset.clone().add_u32(total_size.clone()),
                    ns,
                    vartab,
                    cfg,
                );

                let allocated_array = allocate_array(ty, array_length_var, vartab, cfg);
                let advanced_pointer = Expression::AdvancePointer {
                    pointer: buffer.clone().into(),
                    bytes_offset: array_start.into(),
                };
                cfg.add(
                    vartab,
                    Instr::MemCopy {
                        source: advanced_pointer,
                        destination: Expression::Variable {
                            loc: Codegen,
                            ty: ty.clone(),
                            var_no: allocated_array,
                        },
                        bytes: array_length,
                    },
                );
                (
                    Expression::Variable {
                        loc: Codegen,
                        ty: ty.clone(),
                        var_no: allocated_array,
                    },
                    total_size,
                )
            }

            Type::UserType(type_no) => {
                let usr_type = ns.user_types[*type_no].ty.clone();
                self.read_from_buffer(buffer, offset, &usr_type, validator, ns, vartab, cfg)
            }

            Type::ExternalFunction { .. } => {
                self.decode_external_function(buffer, offset, ty, validator, ns, vartab, cfg)
            }

            Type::Array(elem_ty, dims) => self.decode_array(
                buffer, offset, ty, elem_ty, dims, validator, ns, vartab, cfg,
            ),

            Type::Slice(elem_ty) => {
                let dims = vec![ArrayLength::Dynamic];
                self.decode_array(
                    buffer, offset, ty, elem_ty, &dims, validator, ns, vartab, cfg,
                )
            }

            Type::Struct(struct_ty) => self.decode_struct(
                buffer,
                offset.clone(),
                ty,
                struct_ty,
                validator,
                ns,
                vartab,
                cfg,
            ),

            Type::Rational
            | Type::Ref(_)
            | Type::StorageRef(..)
            | Type::BufferPointer
            | Type::Unresolved
            | Type::InternalFunction { .. }
            | Type::Unreachable
            | Type::Void
            | Type::FunctionSelector
            | Type::Mapping(..) => unreachable!("Type should not appear on an encoded buffer"),
        }
    }

    /// Retrieve a dynamic array length from the encoded buffer. It returns the variable number in which
    /// the length has been stored and the size width of the vector length.
    fn retrieve_array_length(
        &self,
        buffer: &Expression,
        offset: &Expression,
        vartab: &mut Vartable,
        cfg: &mut ControlFlowGraph,
    ) -> (usize, Expression);

    /// Given the buffer and the offset, decode an array.
    /// The function returns an expression containing the array and the number of bytes read.
    fn decode_array(
        &self,
        buffer: &Expression,
        offset: &Expression,
        array_ty: &Type,
        elem_ty: &Type,
        dims: &[ArrayLength],
        validator: &mut BufferValidator,
        ns: &Namespace,
        vartab: &mut Vartable,
        cfg: &mut ControlFlowGraph,
    ) -> (Expression, Expression) {
        // Checks if we can memcpy the elements from the buffer directly to the allocated array
        if allow_memcpy(array_ty, ns) {
            // Calculate number of elements
            let (array_bytes_size, size_width, offset, var_no) =
                if matches!(dims.last(), Some(&ArrayLength::Fixed(_))) {
                    let elem_no = calculate_direct_copy_bytes_size(dims, elem_ty, ns);
                    let allocated_vector = vartab.temp_anonymous(array_ty);
                    let expr = Expression::ArrayLiteral {
                        loc: Codegen,
                        ty: array_ty.clone(),
                        dimensions: vec![],
                        values: vec![],
                    };
                    cfg.add(
                        vartab,
                        Instr::Set {
                            loc: Codegen,
                            res: allocated_vector,
                            expr,
                        },
                    );
                    (
                        Expression::NumberLiteral {
                            loc: Codegen,
                            ty: Uint(32),
                            value: elem_no,
                        },
                        Expression::NumberLiteral {
                            loc: Codegen,
                            ty: Uint(32),
                            value: 0.into(),
                        },
                        offset.clone(),
                        allocated_vector,
                    )
                } else {
                    let (array_length, size_width) =
                        self.retrieve_array_length(buffer, offset, vartab, cfg);
                    let array_start = offset.clone().add_u32(size_width.clone());
                    validator.validate_offset(array_start.clone(), ns, vartab, cfg);
                    (
                        calculate_array_bytes_size(array_length, elem_ty, ns),
                        size_width,
                        array_start,
                        allocate_array(array_ty, array_length, vartab, cfg),
                    )
                };

            validator.validate_offset_plus_size(&offset, &array_bytes_size, ns, vartab, cfg);

            let source_address = Expression::AdvancePointer {
                pointer: Box::new(buffer.clone()),
                bytes_offset: Box::new(offset),
            };

            let array_expr = Expression::Variable {
                loc: Codegen,
                ty: array_ty.clone(),
                var_no,
            };
            cfg.add(
                vartab,
                Instr::MemCopy {
                    source: source_address,
                    destination: array_expr.clone(),
                    bytes: array_bytes_size.clone(),
                },
            );

            let bytes_size = if matches!(dims.last(), Some(ArrayLength::Dynamic)) {
                array_bytes_size.add_u32(size_width)
            } else {
                array_bytes_size
            };

            (array_expr, bytes_size)
        } else {
            let mut indexes: Vec<usize> = Vec::new();
            let array_var = vartab.temp_anonymous(array_ty);

            // The function decode_complex_array assumes that, if the dimension is fixed,
            // there is no need to allocate an array
            if matches!(dims.last(), Some(ArrayLength::Fixed(_))) {
                cfg.add(
                    vartab,
                    Instr::Set {
                        loc: Codegen,
                        res: array_var,
                        expr: Expression::ArrayLiteral {
                            loc: Codegen,
                            ty: array_ty.clone(),
                            dimensions: vec![],
                            values: vec![],
                        },
                    },
                );
            }

            let offset_var = vartab.temp_anonymous(&Uint(32));
            cfg.add(
                vartab,
                Instr::Set {
                    loc: Codegen,
                    res: offset_var,
                    expr: offset.clone(),
                },
            );
            let array_var_expr = Expression::Variable {
                loc: Codegen,
                ty: array_ty.clone(),
                var_no: array_var,
            };
            let offset_expr = Expression::Variable {
                loc: Codegen,
                ty: Uint(32),
                var_no: offset_var,
            };
            self.decode_complex_array(
                &array_var_expr,
                buffer,
                offset_var,
                &offset_expr,
                dims.len() - 1,
                elem_ty,
                dims,
                validator,
                ns,
                vartab,
                cfg,
                &mut indexes,
            );
            // Subtract the original offset from
            // the offset variable to obtain the vector size in bytes
            cfg.add(
                vartab,
                Instr::Set {
                    loc: Codegen,
                    res: offset_var,
                    expr: Expression::Subtract {
                        loc: Codegen,
                        ty: Uint(32),
                        overflowing: false,
                        left: Box::new(offset_expr.clone()),
                        right: Box::new(offset.clone()),
                    },
                },
            );
            (array_var_expr, offset_expr)
        }
    }

    /// Decodes a complex array from a borsh encoded buffer
    /// Complex arrays are either dynamic arrays or arrays of dynamic types, like structs.
    /// If this is an array of structs, whose representation in memory is padded, the array is
    /// also complex, because it cannot be memcpy'ed
    fn decode_complex_array(
        &self,
        array_var: &Expression,
        buffer: &Expression,
        offset_var: usize,
        offset_expr: &Expression,
        dimension: usize,
        elem_ty: &Type,
        dims: &[ArrayLength],
        validator: &mut BufferValidator,
        ns: &Namespace,
        vartab: &mut Vartable,
        cfg: &mut ControlFlowGraph,
        indexes: &mut Vec<usize>,
    ) {
        // If we have a 'int[3][4][] vec', we can only validate the buffer after we have
        // allocated the outer dimension, i.e., we are about to read a 'int[3][4]' item.
        // Arrays whose elements are dynamic cannot be verified.
        if validator.validation_necessary()
            && !dims[0..(dimension + 1)]
                .iter()
                .any(|d| *d == ArrayLength::Dynamic)
            && !elem_ty.is_dynamic(ns)
        {
            let mut elems = BigInt::one();
            for item in &dims[0..(dimension + 1)] {
                elems.mul_assign(item.array_length().unwrap());
            }
            elems.mul_assign(elem_ty.memory_size_of(ns));
            let elems_size = Expression::NumberLiteral {
                loc: Codegen,
                ty: Uint(32),
                value: elems,
            };
            validator.validate_offset_plus_size(offset_expr, &elems_size, ns, vartab, cfg);
            validator.validate_array();
        }

        // Dynamic dimensions mean that the subarray we are processing must be allocated in memory.
        if dims[dimension] == ArrayLength::Dynamic {
            let (array_length, size_length) =
                self.retrieve_array_length(buffer, offset_expr, vartab, cfg);
            let array_start = offset_expr.clone().add_u32(size_length);
            validator.validate_offset(array_start.clone(), ns, vartab, cfg);
            cfg.add(
                vartab,
                Instr::Set {
                    loc: Codegen,
                    res: offset_var,
                    expr: array_start,
                },
            );
            let new_ty = Type::Array(Box::new(elem_ty.clone()), dims[0..(dimension + 1)].to_vec());
            let allocated_array = allocate_array(&new_ty, array_length, vartab, cfg);

            if indexes.is_empty() {
                if let Expression::Variable { var_no, .. } = array_var {
                    cfg.add(
                        vartab,
                        Instr::Set {
                            loc: Codegen,
                            res: *var_no,
                            expr: Expression::Variable {
                                loc: Codegen,
                                ty: new_ty.clone(),
                                var_no: allocated_array,
                            },
                        },
                    );
                } else {
                    unreachable!("array_var must be a variable");
                }
            } else {
                // TODO: This is wired up for multidimensional dynamic arrays, but they do no work yet
                // Check https://github.com/hyperledger-solang/solang/issues/932 for more information
                let sub_arr = index_array(array_var.clone(), dims, indexes, true);
                cfg.add(
                    vartab,
                    Instr::Store {
                        dest: sub_arr,
                        data: Expression::Variable {
                            loc: Codegen,
                            ty: new_ty.clone(),
                            var_no: allocated_array,
                        },
                    },
                );
            }
        }

        let for_loop = set_array_loop(array_var, dims, dimension, indexes, vartab, cfg);
        cfg.set_basic_block(for_loop.body_block);
        if 0 == dimension {
            let (read_expr, advance) =
                self.read_from_buffer(buffer, offset_expr, elem_ty, validator, ns, vartab, cfg);
            let ptr = index_array(array_var.clone(), dims, indexes, true);

            cfg.add(
                vartab,
                Instr::Store {
                    dest: ptr,
                    data: if matches!(read_expr.ty(), Type::Struct(_)) {
                        // Type::Struct is a pointer to a struct. If we are dealing with a vector
                        // of structs, we need to dereference the pointer before storing it at a
                        // given vector index.
                        Expression::Load {
                            loc: Codegen,
                            ty: read_expr.ty(),
                            expr: Box::new(read_expr),
                        }
                    } else {
                        read_expr
                    },
                },
            );
            cfg.add(
                vartab,
                Instr::Set {
                    loc: Codegen,
                    res: offset_var,
                    expr: Expression::Add {
                        loc: Codegen,
                        ty: Uint(32),
                        overflowing: false,
                        left: Box::new(advance),
                        right: Box::new(offset_expr.clone()),
                    },
                },
            );
        } else {
            self.decode_complex_array(
                array_var,
                buffer,
                offset_var,
                offset_expr,
                dimension - 1,
                elem_ty,
                dims,
                validator,
                ns,
                vartab,
                cfg,
                indexes,
            );
        }

        finish_array_loop(&for_loop, vartab, cfg);
    }

    /// Read a struct from the buffer
    fn decode_struct(
        &self,
        buffer: &Expression,
        mut offset: Expression,
        expr_ty: &Type,
        struct_ty: &StructType,
        validator: &mut BufferValidator,
        ns: &Namespace,
        vartab: &mut Vartable,
        cfg: &mut ControlFlowGraph,
    ) -> (Expression, Expression) {
        let size = ns.calculate_struct_non_padded_size(struct_ty);
        // If the size without padding equals the size with padding, memcpy this struct directly.
        if let Some(no_padding_size) = size.as_ref().filter(|no_pad| {
            *no_pad == &struct_ty.struct_padded_size(ns) && allow_memcpy(expr_ty, ns)
        }) {
            let size = Expression::NumberLiteral {
                loc: Codegen,
                ty: Uint(32),
                value: no_padding_size.clone(),
            };
            validator.validate_offset_plus_size(&offset, &size, ns, vartab, cfg);
            let source_address = Expression::AdvancePointer {
                pointer: Box::new(buffer.clone()),
                bytes_offset: Box::new(offset),
            };
            let allocated_struct = vartab.temp_anonymous(expr_ty);
            cfg.add(
                vartab,
                Instr::Set {
                    loc: Codegen,
                    res: allocated_struct,
                    expr: Expression::StructLiteral {
                        loc: Codegen,
                        ty: expr_ty.clone(),
                        values: vec![],
                    },
                },
            );
            let struct_var = Expression::Variable {
                loc: Codegen,
                ty: expr_ty.clone(),
                var_no: allocated_struct,
            };
            cfg.add(
                vartab,
                Instr::MemCopy {
                    source: source_address,
                    destination: struct_var.clone(),
                    bytes: size.clone(),
                },
            );
            return (struct_var, size);
        };
        let size = size.map(|no_pad| Expression::NumberLiteral {
            loc: Codegen,
            ty: Uint(32),
            value: no_pad,
        });

        let struct_tys = struct_ty
            .definition(ns)
            .fields
            .iter()
            .map(|item| item.ty.clone())
            .collect::<Vec<Type>>();

        // If it was not possible to validate the struct beforehand, we validate each field
        // during recursive calls to 'read_from_buffer'
        let mut struct_validator = validator.create_sub_validator(&struct_tys);

        let qty = struct_ty.definition(ns).fields.len();

        if validator.validation_necessary() {
            struct_validator.initialize_validation(&offset, ns, vartab, cfg);
        }

        let (mut read_expr, mut advance) = self.read_from_buffer(
            buffer,
            &offset,
            &struct_tys[0],
            &mut struct_validator,
            ns,
            vartab,
            cfg,
        );
        let mut runtime_size = advance.clone();

        let mut read_items = vec![Expression::Poison; qty];
        read_items[0] = read_expr;
        for i in 1..qty {
            struct_validator.set_argument_number(i);
            struct_validator.validate_buffer(&offset, ns, vartab, cfg);
            offset = Expression::Add {
                loc: Codegen,
                ty: Uint(32),
                overflowing: false,
                left: Box::new(offset.clone()),
                right: Box::new(advance),
            };
            (read_expr, advance) = self.read_from_buffer(
                buffer,
                &offset,
                &struct_tys[i],
                &mut struct_validator,
                ns,
                vartab,
                cfg,
            );
            read_items[i] = read_expr;
            runtime_size = Expression::Add {
                loc: Codegen,
                ty: Uint(32),
                overflowing: false,
                left: Box::new(runtime_size),
                right: Box::new(advance.clone()),
            };
        }

        let allocated_struct = vartab.temp_anonymous(expr_ty);
        cfg.add(
            vartab,
            Instr::Set {
                loc: Codegen,
                res: allocated_struct,
                expr: Expression::StructLiteral {
                    loc: Codegen,
                    ty: expr_ty.clone(),
                    values: read_items,
                },
            },
        );

        let struct_var = Expression::Variable {
            loc: Codegen,
            ty: expr_ty.clone(),
            var_no: allocated_struct,
        };
        (struct_var, size.unwrap_or(runtime_size))
    }

    /// Calculate the size of a single codegen::Expression
    fn get_expr_size(
        &mut self,
        arg_no: usize,
        expr: &Expression,
        ns: &Namespace,
        vartab: &mut Vartable,
        cfg: &mut ControlFlowGraph,
    ) -> Expression {
        let ty = expr.ty().unwrap_user_type(ns);
        match &ty {
            Type::Value => Expression::NumberLiteral {
                loc: Codegen,
                ty: Uint(32),
                value: BigInt::from(ns.value_length),
            },
            Type::Uint(n) | Type::Int(n) => Expression::NumberLiteral {
                loc: Codegen,
                ty: Uint(32),
                value: BigInt::from(n.next_power_of_two() / 8),
            },
            Type::Enum(_) | Type::Contract(_) | Type::Bool | Type::Address(_) | Type::Bytes(_) => {
                Expression::NumberLiteral {
                    loc: Codegen,
                    ty: Uint(32),
                    value: ty.memory_size_of(ns),
                }
            }
            Type::FunctionSelector => Expression::NumberLiteral {
                loc: Codegen,
                ty: Uint(32),
                value: BigInt::from(ns.target.selector_length()),
            },
            Type::Struct(struct_ty) => {
                self.calculate_struct_size(arg_no, expr, struct_ty, ns, vartab, cfg)
            }
            Type::Slice(ty) => {
                let dims = vec![ArrayLength::Dynamic];
                self.calculate_array_size(expr, ty, &dims, arg_no, ns, vartab, cfg)
            }
            Type::Array(ty, dims) => {
                self.calculate_array_size(expr, ty, dims, arg_no, ns, vartab, cfg)
            }
            Type::ExternalFunction { .. } => {
                let selector_len: BigInt = ns.target.selector_length().into();
                let address_size = Type::Address(false).memory_size_of(ns);
                Expression::NumberLiteral {
                    loc: Codegen,
                    ty: Uint(32),
                    value: address_size + selector_len,
                }
            }
            Type::Ref(r) => {
                if let Type::Struct(struct_ty) = &**r {
                    return self.calculate_struct_size(arg_no, expr, struct_ty, ns, vartab, cfg);
                }
                let loaded = Expression::Load {
                    loc: Codegen,
                    ty: *r.clone(),
                    expr: expr.clone().into(),
                };
                self.get_expr_size(arg_no, &loaded, ns, vartab, cfg)
            }
            Type::StorageRef(_, r) => {
                let var = load_storage(&Codegen, r, expr.clone(), cfg, vartab, None);
                let size = self.get_expr_size(arg_no, &var, ns, vartab, cfg);
                self.storage_cache_insert(arg_no, var.clone());
                size
            }
            Type::String | Type::DynamicBytes => self.calculate_string_size(expr, vartab, cfg),
            Type::InternalFunction { .. }
            | Type::Void
            | Type::Unreachable
            | Type::BufferPointer
            | Type::Mapping(..) => unreachable!("This type cannot be encoded"),
            Type::UserType(_) | Type::Unresolved | Type::Rational => {
                unreachable!("Type should not exist in codegen")
            }
        }
    }

    fn decode_external_function(
        &self,
        buffer: &Expression,
        offset: &Expression,
        ty: &Type,
        validator: &mut BufferValidator,
        ns: &Namespace,
        vartab: &mut Vartable,
        cfg: &mut ControlFlowGraph,
    ) -> (Expression, Expression);

    /// Calculate the size of an array
    fn calculate_array_size(
        &mut self,
        array: &Expression,
        elem_ty: &Type,
        dims: &Vec<ArrayLength>,
        arg_no: usize,
        ns: &Namespace,
        vartab: &mut Vartable,
        cfg: &mut ControlFlowGraph,
    ) -> Expression {
        let dyn_dims = dims.iter().filter(|d| **d == ArrayLength::Dynamic).count();

        // If the array does not have variable length elements,
        // we can calculate its size using a simple multiplication (direct_assessment)
        // i.e. 'uint8[3][] vec' has size vec.length*2*size_of(uint8)
        // In cases like 'uint [3][][2] v' this is not possible, as v[0] and v[1] have different sizes
        let direct_assessment =
            dyn_dims == 0 || (dyn_dims == 1 && dims.last() == Some(&ArrayLength::Dynamic));

        // Check if the array contains only fixed sized elements
        let primitive_size = if elem_ty.is_primitive() && direct_assessment {
            Some(elem_ty.memory_size_of(ns))
        } else if let Type::Struct(struct_ty) = elem_ty {
            if direct_assessment {
                ns.calculate_struct_non_padded_size(struct_ty)
            } else {
                None
            }
        } else {
            None
        };

        if let Some(compile_type_size) = primitive_size {
            // If the array saves primitive-type elements, its size is sizeof(type)*vec.length
            let mut size = if let ArrayLength::Fixed(dim) = &dims.last().unwrap() {
                Expression::NumberLiteral {
                    loc: Codegen,
                    ty: Uint(32),
                    value: dim.clone(),
                }
            } else {
                Expression::Builtin {
                    loc: Codegen,
                    tys: vec![Uint(32)],
                    kind: Builtin::ArrayLength,
                    args: vec![array.clone()],
                }
            };

            for item in dims.iter().take(dims.len() - 1) {
                let local_size = Expression::NumberLiteral {
                    loc: Codegen,
                    ty: Uint(32),
                    value: item.array_length().unwrap().clone(),
                };
                size = Expression::Multiply {
                    loc: Codegen,
                    ty: Uint(32),
                    overflowing: false,
                    left: size.into(),
                    right: local_size.clone().into(),
                };
            }

            let size_width = self.size_width(&size, vartab, cfg);

            let type_size = Expression::NumberLiteral {
                loc: Codegen,
                ty: Uint(32),
                value: compile_type_size,
            };
            let size = Expression::Multiply {
                loc: Codegen,
                ty: Uint(32),
                overflowing: false,
                left: size.into(),
                right: type_size.into(),
            };
            let size_var = vartab.temp_anonymous(&Uint(32));
            cfg.add(
                vartab,
                Instr::Set {
                    loc: Codegen,
                    res: size_var,
                    expr: size,
                },
            );
            let size_var = Expression::Variable {
                loc: Codegen,
                ty: Uint(32),
                var_no: size_var,
            };
            if self.is_packed() || !matches!(&dims.last().unwrap(), ArrayLength::Dynamic) {
                return size_var;
            }
            Expression::Add {
                loc: Codegen,
                ty: Uint(32),
                overflowing: false,
                left: size_var.into(),
                right: size_width.into(),
            }
        } else {
            let size_var =
                vartab.temp_name(format!("array_bytes_size_{arg_no}").as_str(), &Uint(32));
            cfg.add(
                vartab,
                Instr::Set {
                    loc: Codegen,
                    res: size_var,
                    expr: Expression::NumberLiteral {
                        loc: Codegen,
                        ty: Uint(32),
                        value: BigInt::from(0u8),
                    },
                },
            );
            let mut index_vec: Vec<usize> = Vec::new();
            self.calculate_complex_array_size(
                arg_no,
                array,
                dims,
                dims.len() - 1,
                size_var,
                ns,
                &mut index_vec,
                vartab,
                cfg,
            );
            Expression::Variable {
                loc: Codegen,
                ty: Uint(32),
                var_no: size_var,
            }
        }
    }

    /// Calculate the size of a complex array.
    /// This function indexes an array from its outer dimension to its inner one and
    /// accounts for the encoded length size for dynamic dimensions.
    fn calculate_complex_array_size(
        &mut self,
        arg_no: usize,
        arr: &Expression,
        dims: &Vec<ArrayLength>,
        dimension: usize,
        size_var_no: usize,
        ns: &Namespace,
        indexes: &mut Vec<usize>,
        vartab: &mut Vartable,
        cfg: &mut ControlFlowGraph,
    ) {
        // If this dimension is dynamic, account for the encoded vector length variable.
        if !self.is_packed() && dims[dimension] == ArrayLength::Dynamic {
            let arr = index_array(arr.clone(), dims, indexes, false);
            let size = Expression::Builtin {
                loc: Codegen,
                tys: vec![Uint(32)],
                kind: Builtin::ArrayLength,
                args: vec![arr],
            };
            let size_width = self.size_width(&size, vartab, cfg);
            let size_var = Expression::Variable {
                loc: Codegen,
                ty: Uint(32),
                var_no: size_var_no,
            };
            cfg.add(
                vartab,
                Instr::Set {
                    loc: Codegen,
                    res: size_var_no,
                    expr: Expression::Add {
                        loc: Codegen,
                        ty: Uint(32),
                        overflowing: false,
                        left: size_var.into(),
                        right: size_width.into(),
                    },
                },
            );
        }

        let for_loop = set_array_loop(arr, dims, dimension, indexes, vartab, cfg);
        cfg.set_basic_block(for_loop.body_block);
        if 0 == dimension {
            let deref = index_array(arr.clone(), dims, indexes, false);
            let elem_size = self.get_expr_size(arg_no, &deref, ns, vartab, cfg);
            let size_var = Expression::Variable {
                loc: Codegen,
                ty: Uint(32),
                var_no: size_var_no,
            };
            cfg.add(
                vartab,
                Instr::Set {
                    loc: Codegen,
                    res: size_var_no,
                    expr: Expression::Add {
                        loc: Codegen,
                        ty: Uint(32),
                        overflowing: false,
                        left: size_var.into(),
                        right: elem_size.into(),
                    },
                },
            );
        } else {
            self.calculate_complex_array_size(
                arg_no,
                arr,
                dims,
                dimension - 1,
                size_var_no,
                ns,
                indexes,
                vartab,
                cfg,
            );
        }
        finish_array_loop(&for_loop, vartab, cfg);
    }

    /// Retrieves the size of a struct
    fn calculate_struct_size(
        &mut self,
        arg_no: usize,
        expr: &Expression,
        struct_ty: &StructType,
        ns: &Namespace,
        vartab: &mut Vartable,
        cfg: &mut ControlFlowGraph,
    ) -> Expression {
        if let Some(struct_size) = ns.calculate_struct_non_padded_size(struct_ty) {
            return Expression::NumberLiteral {
                loc: Codegen,
                ty: Uint(32),
                value: struct_size,
            };
        }
        let first_type = struct_ty.definition(ns).fields[0].ty.clone();
        let first_field = load_struct_member(first_type, expr.clone(), 0, ns);
        let mut size = self.get_expr_size(arg_no, &first_field, ns, vartab, cfg);
        for i in 1..struct_ty.definition(ns).fields.len() {
            let ty = struct_ty.definition(ns).fields[i].ty.clone();
            let field = load_struct_member(ty.clone(), expr.clone(), i, ns);
            let expr_size = self.get_expr_size(arg_no, &field, ns, vartab, cfg).into();
            size = Expression::Add {
                loc: Codegen,
                ty: Uint(32),
                overflowing: false,
                left: size.clone().into(),
                right: expr_size,
            };
        }
        size
    }

    fn calculate_string_size(
        &self,
        expr: &Expression,
        _vartab: &mut Vartable,
        _cfg: &mut ControlFlowGraph,
    ) -> Expression;

    /// Encoding happens in two steps. First, we look at each argument to calculate its size. If an
    /// argument is a storage variable, we load it and save it to a local variable.
    ///
    /// During a second pass, we copy each argument to a buffer. To copy storage variables properly into
    /// the buffer, we must load them from storage and save them in a local variable. As we have
    /// already done this, we can cache the Expression::Variable, containing the items we loaded before.
    /// In addition, loading from storage can be an expensive operation if it's done with large structs
    /// or vectors.
    ///
    /// This function serves only to cache Expression::Variable, containing items loaded from storage.
    /// Nothing else should be stored here.
    fn storage_cache_insert(&mut self, arg_no: usize, expr: Expression);

    fn storage_cache_remove(&mut self, arg_no: usize) -> Option<Expression>;

    /// Returns if the we are packed encoding
    fn is_packed(&self) -> bool;

    /// Encode constant data at compile time.
    ///
    /// Returns `None` if the data can not be encoded at compile time.
    fn const_encode(&self, _args: &[Expression]) -> Option<Vec<u8>> {
        None
    }
}

/// This function should return the correct encoder, given the target
pub(crate) fn create_encoder(ns: &Namespace, packed: bool) -> Box<dyn AbiEncoding> {
    match &ns.target {
        Target::Solana => Box::new(BorshEncoding::new(packed)),
        // Solana utilizes Borsh encoding and Polkadot, SCALE encoding.
        // All other targets are using the SCALE encoding, because we have tests for a
        // fake Ethereum target that checks the presence of Instr::AbiDecode and
        // Expression::AbiEncode.
        // If a new target is added, this piece of code needs to change.
        _ => Box::new(ScaleEncoding::new(packed)),
    }
}

/// Indexes an array. If we have 'int[3][][4] vec' and we need 'int[3][]',
/// 'int[3]' or 'int' (the array element) this function returns so.
///
/// * `arr` - The expression that represents the array
/// * `dims` - is the vector containing the array's dimensions
/// * `index` - is the list of indexes to use for each dimension
/// * `coerce_pointer_return` - forces the return of a pointer in this function.
///
/// When applying Expression::Subscript to a fixed-sized array, like 'int[3][4] vec', we
/// have a pointer to a 'int[3]'. If we would like to index it again, there is no need to load,
/// because a pointer to 'int[3]' is what the LLVM GEP instruction requires.
///
/// The underlying representation of a dynamic array is a C struct called 'struct vector'. In this
/// sense, a vector like 'uint16[][] vec is a 'struct vector', whose buffer elements are all pointers to
/// other 'struct vector's. When we index the first dimension of 'vec', we have a pointer to a pointer
/// to a 'struct vector', which is not compatible with LLVM GEP instruction for further indexing.
/// Therefore, we need an Expression::Load to obtain a pointer to 'struct vector' to be able to index
/// it again.
///
/// Even though all the types this function returns are pointers in the LLVM IR representation,
/// the argument `coerce_pointer_return` must be true when we are dealing with dynamic arrays that are
/// going to be the destination address of a store instruction.
///
/// In a case like this,
///
/// uint16[][] vec;
/// uint16[] vec1;
/// vec[0] = vec1;
///
/// 'vec[0]' must be a pointer to a pointer to a 'struct vector' so that the LLVM Store instruction
/// can be executed properly, as the value we are trying to store there is a pointer to a 'struct vector'.
/// In this case, we must coerce the return of a pointer. Everywhere else, the load is necessary.
///
/// `coerce_pointer_return` has not effect for fixed sized arrays.
fn index_array(
    mut arr: Expression,
    dims: &[ArrayLength],
    indexes: &[usize],
    coerce_pointer_return: bool,
) -> Expression {
    let mut ty = arr.ty();
    let elem_ty = ty.elem_ty();
    let begin = dims.len() - indexes.len();

    for i in (begin..dims.len()).rev() {
        // If we are indexing the last dimension, the type should be that of the array element.
        let local_ty = if i == 0 {
            elem_ty.clone()
        } else {
            Type::Array(Box::new(elem_ty.clone()), dims[0..i].to_vec())
        };
        arr = Expression::Subscript {
            loc: Codegen,
            ty: Type::Ref(local_ty.clone().into()),
            array_ty: ty,
            expr: Box::new(arr),
            index: Box::new(Expression::Variable {
                loc: Loc::Codegen,
                ty: Type::Uint(32),
                var_no: indexes[dims.len() - i - 1],
            }),
        };

        // We should only load if the dimension is dynamic.
        if i > 0 && dims[i - 1] == ArrayLength::Dynamic {
            arr = Expression::Load {
                loc: Loc::Codegen,
                ty: local_ty.clone(),
                expr: arr.into(),
            };
        }

        ty = local_ty;
    }

    if coerce_pointer_return && !matches!(arr.ty(), Type::Ref(_)) {
        if let Expression::Load { expr, .. } = arr {
            return *expr;
        } else {
            unreachable!("Expression should be a load");
        }
    }

    arr
}

/// This struct manages for-loops created when iterating over arrays
struct ForLoop {
    pub cond_block: usize,
    pub next_block: usize,
    pub body_block: usize,
    pub end_block: usize,
    pub index: usize,
}

/// Set up the loop to iterate over an array
fn set_array_loop(
    arr: &Expression,
    dims: &[ArrayLength],
    dimension: usize,
    indexes: &mut Vec<usize>,
    vartab: &mut Vartable,
    cfg: &mut ControlFlowGraph,
) -> ForLoop {
    let index_temp = vartab.temp_name(format!("for_i_{dimension}").as_str(), &Uint(32));

    cfg.add(
        vartab,
        Instr::Set {
            loc: Codegen,
            res: index_temp,
            expr: Expression::NumberLiteral {
                loc: Codegen,
                ty: Uint(32),
                value: 0u8.into(),
            },
        },
    );

    indexes.push(index_temp);
    let cond_block = cfg.new_basic_block("cond".to_string());
    let next_block = cfg.new_basic_block("next".to_string());
    let body_block = cfg.new_basic_block("body".to_string());
    let end_block = cfg.new_basic_block("end_for".to_string());

    vartab.new_dirty_tracker();
    cfg.add(vartab, Instr::Branch { block: cond_block });
    cfg.set_basic_block(cond_block);
    // Get the array length at dimension 'index'
    let bound = if let ArrayLength::Fixed(dim) = &dims[dimension] {
        Expression::NumberLiteral {
            loc: Codegen,
            ty: Uint(32),
            value: dim.clone(),
        }
    } else {
        let sub_array = index_array(arr.clone(), dims, &indexes[..indexes.len() - 1], false);
        Expression::Builtin {
            loc: Codegen,
            tys: vec![Uint(32)],
            kind: Builtin::ArrayLength,
            args: vec![sub_array],
        }
    };
    let cond_expr = Expression::Less {
        loc: Codegen,
        signed: false,
        left: Expression::Variable {
            loc: Codegen,
            ty: Uint(32),
            var_no: index_temp,
        }
        .into(),
        right: bound.into(),
    };
    cfg.add(
        vartab,
        Instr::BranchCond {
            cond: cond_expr,
            true_block: body_block,
            false_block: end_block,
        },
    );

    ForLoop {
        cond_block,
        next_block,
        body_block,
        end_block,
        index: index_temp,
    }
}

/// Closes the for-loop when iterating over an array
fn finish_array_loop(for_loop: &ForLoop, vartab: &mut Vartable, cfg: &mut ControlFlowGraph) {
    cfg.add(
        vartab,
        Instr::Branch {
            block: for_loop.next_block,
        },
    );
    cfg.set_basic_block(for_loop.next_block);
    let index_var = Expression::Variable {
        loc: Codegen,
        ty: Uint(32),
        var_no: for_loop.index,
    };
    let one = Expression::NumberLiteral {
        loc: Codegen,
        ty: Uint(32),
        value: 1u8.into(),
    };
    cfg.add(
        vartab,
        Instr::Set {
            loc: Codegen,
            res: for_loop.index,
            expr: Expression::Add {
                loc: Codegen,
                ty: Uint(32),
                overflowing: false,
                left: index_var.into(),
                right: one.into(),
            },
        },
    );
    cfg.add(
        vartab,
        Instr::Branch {
            block: for_loop.cond_block,
        },
    );
    cfg.set_basic_block(for_loop.end_block);
    let phis = vartab.pop_dirty_tracker();
    cfg.set_phis(for_loop.next_block, phis.clone());
    cfg.set_phis(for_loop.end_block, phis.clone());
    cfg.set_phis(for_loop.cond_block, phis);
}

/// Loads a struct member
fn load_struct_member(ty: Type, expr: Expression, member: usize, ns: &Namespace) -> Expression {
    if ty.is_fixed_reference_type(ns) {
        // We should not dereference a struct or fixed array
        return Expression::StructMember {
            loc: Codegen,
            ty,
            expr: expr.into(),
            member,
        };
    }
    let s = Expression::StructMember {
        loc: Codegen,
        ty: Type::Ref(ty.clone().into()),
        expr: expr.into(),
        member,
    };
    Expression::Load {
        loc: Codegen,
        ty,
        expr: s.into(),
    }
}

/// Get the outer array length inside a variable (cannot be used for any dimension).
fn array_outer_length(
    arr: &Expression,
    vartab: &mut Vartable,
    cfg: &mut ControlFlowGraph,
) -> Expression {
    let get_size = Expression::Builtin {
        loc: Codegen,
        tys: vec![Uint(32)],
        kind: Builtin::ArrayLength,
        args: vec![arr.clone()],
    };
    let array_length = vartab.temp_anonymous(&Uint(32));
    cfg.add(
        vartab,
        Instr::Set {
            loc: Codegen,
            res: array_length,
            expr: get_size,
        },
    );
    Expression::Variable {
        loc: Codegen,
        ty: Uint(32),
        var_no: array_length,
    }
}

/// Check if we can MemCpy a type to/from a buffer
fn allow_memcpy(ty: &Type, ns: &Namespace) -> bool {
    match ty {
        Type::Struct(struct_ty) => {
            if let Some(no_padded_size) = ns.calculate_struct_non_padded_size(struct_ty) {
                let padded_size = struct_ty.struct_padded_size(ns);
                // This remainder tells us if padding is needed between the elements of an array
                let remainder = padded_size.mod_floor(&ty.struct_elem_alignment(ns));
                let ty_allowed = struct_ty
                    .definition(ns)
                    .fields
                    .iter()
                    .all(|f| allow_memcpy(&f.ty, ns));
                return no_padded_size == padded_size && remainder.is_zero() && ty_allowed;
            }
            false
        }
        Type::Bytes(n) => *n < 2, // When n >= 2, the bytes must be reversed
        // If this is a dynamic array, we mempcy if its elements allow it and we don't need to index it.
        Type::Array(t, dims) if ty.is_dynamic(ns) => dims.len() == 1 && allow_memcpy(t, ns),
        // If the array is not dynamic, we mempcy if its elements allow it
        Type::Array(t, _) => allow_memcpy(t, ns),
        Type::UserType(t) => allow_memcpy(&ns.user_types[*t].ty, ns),
        _ => ty.is_primitive(),
    }
}

/// Calculate the number of bytes needed to memcpy an entire vector
fn calculate_direct_copy_bytes_size(
    dims: &[ArrayLength],
    elem_ty: &Type,
    ns: &Namespace,
) -> BigInt {
    let mut elem_no = BigInt::one();
    for item in dims {
        debug_assert!(matches!(item, &ArrayLength::Fixed(_)));
        elem_no.mul_assign(item.array_length().unwrap());
    }
    let bytes = elem_ty.memory_size_of(ns);
    elem_no.mul_assign(bytes);
    elem_no
}

/// Calculate the size in bytes of a dynamic array, whose dynamic dimension is the outer.
/// It needs the variable saving the array's length.
fn calculate_array_bytes_size(length_var: usize, elem_ty: &Type, ns: &Namespace) -> Expression {
    let var = Expression::Variable {
        loc: Codegen,
        ty: Uint(32),
        var_no: length_var,
    };
    let size = Expression::NumberLiteral {
        loc: Codegen,
        ty: Uint(32),
        value: elem_ty.memory_size_of(ns),
    };
    Expression::Multiply {
        loc: Codegen,
        ty: Uint(32),
        overflowing: false,
        left: var.into(),
        right: size.into(),
    }
}

/// Allocate an array in memory and return its variable number.
fn allocate_array(
    ty: &Type,
    length_variable: usize,
    vartab: &mut Vartable,
    cfg: &mut ControlFlowGraph,
) -> usize {
    let array_var = vartab.temp_anonymous(ty);
    let length_var = Expression::Variable {
        loc: Codegen,
        ty: Uint(32),
        var_no: length_variable,
    };
    cfg.add(
        vartab,
        Instr::Set {
            loc: Codegen,
            res: array_var,
            expr: Expression::AllocDynamicBytes {
                loc: Codegen,
                ty: ty.clone(),
                size: length_var.into(),
                initializer: None,
            },
        },
    );
    array_var
}

impl StructType {
    /// Calculate a struct size in memory considering the padding, if necessary
    fn struct_padded_size(&self, ns: &Namespace) -> BigInt {
        let mut total = BigInt::zero();
        for item in &self.definition(ns).fields {
            let ty_align = item.ty.struct_elem_alignment(ns);
            let remainder = total.mod_floor(&ty_align);
            if !remainder.is_zero() {
                let padding = ty_align.sub(remainder);
                total.add_assign(padding);
            }
            total.add_assign(item.ty.memory_size_of(ns));
        }
        total
    }
}
