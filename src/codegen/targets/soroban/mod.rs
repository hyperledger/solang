// SPDX-License-Identifier: Apache-2.0

pub(crate) mod arrays;
pub(crate) mod bytes;
pub(crate) mod dispatch;
pub(crate) mod encoding;
pub(crate) mod events;
pub(crate) mod map;
pub(crate) mod storage_path;

use self::encoding::{
    soroban_decode, soroban_decode_arg, soroban_encode, soroban_encode_arg,
    soroban_storage_decode_arg, soroban_storage_encode_arg,
};
use self::events::SorobanEventEmitter;
use self::storage_path::{
    is_descent_storage_expr, lower_storage_path, path_delete_map, path_load, path_load_map,
    path_store, Idx,
};
use crate::codegen::cfg::{ASTFunction, ControlFlowGraph, Instr, InternalCallTy};
use crate::codegen::error::CodegenError;
use crate::codegen::expression::{expression, load_storage, storage_type};
use crate::codegen::interface::{EventEmitter, TargetCodegen};
use crate::codegen::vartable::Vartable;
use crate::codegen::Options;
use crate::codegen::{Expression, HostFunctions};
use crate::sema::ast;
use crate::sema::ast::{Function, Namespace, RetrieveType, StructType, Type};
use num_bigint::{BigInt, Sign};
use num_traits::{ToPrimitive, Zero};
use solang_parser::helpers::CodeLocation;
use solang_parser::{diagnostics::Diagnostic, pt};

/// Codegen for the Soroban target. All Soroban-specific lowering lives under this module
/// (`dispatch`, `encoding`, `events`, plus the validation and storage helpers below).
pub(crate) struct SorobanTarget;

impl TargetCodegen for SorobanTarget {
    fn validate_contract(&self, contract_no: usize, ns: &mut Namespace) {
        validate_accessor_abi_types(contract_no, ns);
        if ns.diagnostics.any_errors() {
            return;
        }
        validate_event_abi_types(contract_no, ns);
    }

    fn validate_cfgs(&self, all_cfg: &[ControlFlowGraph], ns: &mut Namespace) {
        validate_abi_types(all_cfg, ns);
    }

    fn function_dispatch(
        &self,
        contract_no: usize,
        all_cfg: &mut [ControlFlowGraph],
        ns: &mut Namespace,
        opt: &Options,
    ) -> Vec<ControlFlowGraph> {
        dispatch::function_dispatch(contract_no, all_cfg, ns, opt)
    }

    fn lower_storage_array_length(
        &self,
        loc: &pt::Loc,
        ty: &Type,
        array: Expression,
        _elem_ty: &Type,
        cfg: &mut ControlFlowGraph,
        vartab: &mut Vartable,
        ns: &Namespace,
    ) -> Expression {
        arrays::soroban_storage_array_length(loc, ty, array, cfg, vartab, ns)
    }

    /// Soroban lazy decode path: if memory contains encoded handles, decode on demand.
    fn lower_load(
        &self,
        load: Expression,
        cfg: &mut ControlFlowGraph,
        vartab: &mut Vartable,
        ns: &Namespace,
    ) -> Expression {
        if let Expression::Load { ref expr, .. } = load {
            if let Type::Ref(inner) = expr.ty() {
                if matches!(inner.as_ref(), Type::SorobanHandle(_)) {
                    return soroban_decode_arg(load, cfg, vartab, ns, None);
                }
            }
        }
        load
    }

    fn prepare_storage_value(
        &self,
        value: Expression,
        dest: &Expression,
        cfg: &mut ControlFlowGraph,
        vartab: &mut Vartable,
        ns: &Namespace,
    ) -> Expression {
        // For Store to a non-SorobanHandle Ref, pass the value through unchanged.
        if let Type::Ref(inner) = dest.ty() {
            if !matches!(inner.as_ref(), Type::SorobanHandle(_)) {
                return value;
            }
        }
        soroban_storage_encode_arg(value, cfg, vartab, ns)
    }

    fn default_storage_value(
        &self,
        loc: &pt::Loc,
        ty: &Type,
        cfg: &mut ControlFlowGraph,
        vartab: &mut Vartable,
        ns: &Namespace,
    ) -> Option<Expression> {
        match ty {
            Type::DynamicBytes | Type::String | Type::Bytes(_) | Type::Slice(_) => {
                Some(soroban_default_handle(loc, ty, cfg, vartab, ns))
            }
            Type::Array(_, dims) if dims.last() == Some(&ast::ArrayLength::Dynamic) => {
                Some(soroban_default_handle(loc, ty, cfg, vartab, ns))
            }
            Type::Array(..) if ty.array_length().is_some() => {
                Some(soroban_default_handle(loc, ty, cfg, vartab, ns))
            }
            Type::Struct(StructType::UserDefined(_)) => {
                Some(soroban_default_handle(loc, ty, cfg, vartab, ns))
            }
            Type::Mapping(..) => Some(soroban_default_handle(loc, ty, cfg, vartab, ns)),
            _ => None,
        }
    }

    fn abi_encode(
        &self,
        loc: &pt::Loc,
        args: Vec<Expression>,
        ns: &Namespace,
        vartab: &mut Vartable,
        cfg: &mut ControlFlowGraph,
        packed: bool,
    ) -> (Expression, Expression) {
        // Soroban encodes to ScVal handles; soroban_encode returns a 3-tuple, drop the spread.
        let (buffer, size, _) = soroban_encode(loc, args, ns, vartab, cfg, packed);
        (buffer, size)
    }

    fn abi_decode(
        &self,
        loc: &pt::Loc,
        buffer: &Expression,
        types: &[Type],
        ns: &Namespace,
        vartab: &mut Vartable,
        cfg: &mut ControlFlowGraph,
        buffer_size_expr: Option<Expression>,
    ) -> Vec<Expression> {
        soroban_decode(loc, buffer, types, ns, vartab, cfg, buffer_size_expr)
    }

    fn storage_array_push(
        &self,
        loc: &pt::Loc,
        args: &[ast::Expression],
        cfg: &mut ControlFlowGraph,
        contract_no: usize,
        func: Option<&Function>,
        ns: &Namespace,
        vartab: &mut Vartable,
        opt: &Options,
    ) -> Expression {
        if args[0].ty().is_storage_bytes() {
            return bytes::soroban_bytes_push(
                loc,
                args,
                cfg,
                contract_no,
                func,
                ns,
                vartab,
                opt,
                self,
            );
        }
        arrays::soroban_storage_push(loc, args, cfg, contract_no, func, ns, vartab, opt, self)
    }

    fn storage_array_pop(
        &self,
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
        // `bytes` in storage is a host BytesObject (not a Vec): pop via the dedicated
        // host `bytes_pop`, read-modify-write on the stored handle.
        if args[0].ty().is_storage_bytes() {
            bytes::soroban_bytes_pop(
                loc,
                args,
                return_ty,
                cfg,
                contract_no,
                func,
                ns,
                vartab,
                opt,
                self,
            )
        } else {
            arrays::soroban_storage_pop(
                loc,
                args,
                return_ty,
                cfg,
                contract_no,
                func,
                ns,
                vartab,
                opt,
                self,
            )
        }
    }

    fn event_emitter<'a>(
        &self,
        _loc: &pt::Loc,
        event_no: usize,
        args: &'a [ast::Expression],
        ns: &'a Namespace,
    ) -> Box<dyn EventEmitter + 'a> {
        Box::new(SorobanEventEmitter { args, ns, event_no })
    }

    fn lower_builtin(
        &self,
        loc: &pt::Loc,
        builtin: ast::Builtin,
        args: &[ast::Expression],
        cfg: &mut ControlFlowGraph,
        contract_no: usize,
        func: Option<&Function>,
        ns: &Namespace,
        vartab: &mut Vartable,
        opt: &Options,
    ) -> Option<Expression> {
        match builtin {
            ast::Builtin::GetAddress => {
                // // In soroban, address is retrieved via a host function call
                if let Some(constant_id) = &ns.contracts[contract_no].program_id {
                    return Some(Expression::NumberLiteral {
                        loc: *loc,
                        ty: Type::Address(false),
                        value: BigInt::from_bytes_be(Sign::Plus, constant_id),
                    });
                }
                let address_var_no = vartab.temp_anonymous(&Type::Uint(64));
                let address_var = Expression::Variable {
                    loc: *loc,
                    ty: Type::Address(false),
                    var_no: address_var_no,
                };
                cfg.add(
                    vartab,
                    Instr::Call {
                        res: vec![address_var_no],
                        return_tys: vec![Type::Uint(64)],
                        call: InternalCallTy::HostFunction {
                            name: HostFunctions::GetCurrentContractAddress.name().to_string(),
                        },
                        args: vec![],
                    },
                );
                Some(address_var)
            }
            ast::Builtin::Sha256 => {
                assert_eq!(args.len(), 1, "sha256 takes exactly one argument");
                let input = expression(&args[0], cfg, contract_no, func, ns, vartab, opt, self);
                let bytes_obj = soroban_encode_arg(input, cfg, vartab, ns);
                let hash_obj = vartab.temp_name("sha256_hash", &Type::Uint(64));
                cfg.add(
                    vartab,
                    Instr::Call {
                        res: vec![hash_obj],
                        call: InternalCallTy::HostFunction {
                            name: HostFunctions::ComputeHashSha256.name().to_string(),
                        },
                        args: vec![bytes_obj],
                        return_tys: vec![Type::Uint(64)],
                    },
                );
                let hash_expr = Expression::Variable {
                    loc: pt::Loc::Codegen,
                    ty: Type::Uint(64),
                    var_no: hash_obj,
                };
                Some(soroban_decode_arg(
                    hash_expr,
                    cfg,
                    vartab,
                    ns,
                    Some(Type::Bytes(32)),
                ))
            }
            ast::Builtin::Keccak256 => {
                assert_eq!(args.len(), 1, "Keccak256 takes exactly one argument");
                let input = expression(&args[0], cfg, contract_no, func, ns, vartab, opt, self);
                let bytes_obj = soroban_encode_arg(input, cfg, vartab, ns);
                let hash_obj = vartab.temp_name("keccak256_hash", &Type::Uint(64));
                cfg.add(
                    vartab,
                    Instr::Call {
                        res: vec![hash_obj],
                        call: InternalCallTy::HostFunction {
                            name: HostFunctions::ComputeHashKeccak256.name().to_string(),
                        },
                        args: vec![bytes_obj],
                        return_tys: vec![Type::Uint(64)],
                    },
                );
                let hash_expr = Expression::Variable {
                    loc: pt::Loc::Codegen,
                    ty: Type::Uint(64),
                    var_no: hash_obj,
                };
                Some(soroban_decode_arg(
                    hash_expr,
                    cfg,
                    vartab,
                    ns,
                    Some(Type::Bytes(32)),
                ))
            }
            ast::Builtin::ToXdr => {
                assert_eq!(args.len(), 1, "to_xdr takes exactly one argument");
                let input = expression(&args[0], cfg, contract_no, func, ns, vartab, opt, self);
                let val = soroban_encode_arg(input, cfg, vartab, ns);
                let xdr_obj = vartab.temp_name("xdr_bytes", &Type::Uint(64));
                cfg.add(
                    vartab,
                    Instr::Call {
                        res: vec![xdr_obj],
                        call: InternalCallTy::HostFunction {
                            name: HostFunctions::SerializeToBytes.name().to_string(),
                        },
                        args: vec![val],
                        return_tys: vec![Type::Uint(64)],
                    },
                );
                let xdr_expr = Expression::Variable {
                    loc: pt::Loc::Codegen,
                    ty: Type::Uint(64),
                    var_no: xdr_obj,
                };
                Some(soroban_decode_arg(
                    xdr_expr,
                    cfg,
                    vartab,
                    ns,
                    Some(Type::DynamicBytes),
                ))
            }
            ast::Builtin::Timestamp => {
                assert_eq!(args.len(), 0, "timestamp expects no arguments");
                let timestamp_var_no = vartab.temp_name("timestamp", &Type::Uint(64));
                let timestamp_var = Expression::Variable {
                    loc: *loc,
                    ty: Type::Uint(64),
                    var_no: timestamp_var_no,
                };
                cfg.add(
                    vartab,
                    Instr::Call {
                        res: vec![timestamp_var_no],
                        return_tys: vec![Type::Uint(64)],
                        call: InternalCallTy::HostFunction {
                            name: HostFunctions::GetLedgerTimestamp.name().to_string(),
                        },
                        args: vec![],
                    },
                );
                Some(soroban_decode_arg(
                    timestamp_var,
                    cfg,
                    vartab,
                    ns,
                    Some(Type::Uint(64)),
                ))
            }
            ast::Builtin::BlockNumber => {
                assert_eq!(args.len(), 0, "Block Number expects no arguments");
                let block_var_no = vartab.temp_name("block_number", &Type::Uint(64));
                let block_var = Expression::Variable {
                    loc: *loc,
                    ty: Type::Uint(64),
                    var_no: block_var_no,
                };
                cfg.add(
                    vartab,
                    Instr::Call {
                        res: vec![block_var_no],
                        return_tys: vec![Type::Uint(64)],
                        call: InternalCallTy::HostFunction {
                            name: HostFunctions::GetLedgerSequence.name().to_string(),
                        },
                        args: vec![],
                    },
                );
                Some(soroban_decode_arg(
                    block_var,
                    cfg,
                    vartab,
                    ns,
                    Some(Type::Uint(64)),
                ))
            }

            ast::Builtin::RequireAuth => {
                let var_temp = vartab.temp(
                    &pt::Identifier {
                        name: "auth".to_owned(),
                        loc: *loc,
                    },
                    &Type::Bool,
                );

                let var = Expression::Variable {
                    loc: *loc,
                    ty: Type::Address(false),
                    var_no: var_temp,
                };
                let expr = if let Type::StorageRef(_, _) = args[0].ty() {
                    soroban_storage_load(
                        loc,
                        &args[0],
                        &Type::Address(false),
                        cfg,
                        contract_no,
                        func,
                        ns,
                        vartab,
                        opt,
                        self,
                    )
                } else {
                    expression(&args[0], cfg, contract_no, func, ns, vartab, opt, self)
                };

                let instr = Instr::Call {
                    res: vec![var_temp],
                    return_tys: vec![Type::Void],
                    call: InternalCallTy::HostFunction {
                        name: HostFunctions::RequireAuth.name().to_string(),
                    },
                    args: vec![expr],
                };

                cfg.add(vartab, instr);

                Some(var)
            }
            // This is the trickiest host function to implement. The reason is takes `InvokerContractAuthEntry` enum as an argument.
            // let x = SubContractInvocation {
            //     context: ContractContext {
            //         contract: c.clone(),
            //         fn_name: symbol_short!("increment"),
            //          args: vec![&env, current_contract.into_val(&env)],
            //     },
            //     sub_invocations: vec![&env],
            //  };
            //  let auth_context = auth::InvokerContractAuthEntry::Contract(x);
            // Most of the logic done here is just to encode the above struct as the host expects it.
            // FIXME: This uses a series of MapNew, and multiple inserts to create the struct.
            // This is not efficient and should be optimized.
            // Instead, we should use MapNewFromLinearMemory to create the struct in one go.
            ast::Builtin::AuthAsCurrContract => {
                let symbol_key_1 = Expression::BytesLiteral {
                    loc: pt::Loc::Codegen,
                    ty: Type::String,
                    value: "contract".as_bytes().to_vec(),
                };
                let symbol_key_2 = Expression::BytesLiteral {
                    loc: pt::Loc::Codegen,
                    ty: Type::String,
                    value: "fn_name".as_bytes().to_vec(),
                };
                let symbol_key_3 = Expression::BytesLiteral {
                    loc: pt::Loc::Codegen,
                    ty: Type::String,
                    value: "args".as_bytes().to_vec(),
                };

                let symbols = soroban_encode(
                    loc,
                    vec![symbol_key_1, symbol_key_2, symbol_key_3],
                    ns,
                    vartab,
                    cfg,
                    false,
                )
                .2;

                let contract_value =
                    expression(&args[0], cfg, contract_no, func, ns, vartab, opt, self);
                let fn_name_symbol =
                    expression(&args[1], cfg, contract_no, func, ns, vartab, opt, self);

                let symbol_string =
                    if let Expression::BytesLiteral { loc, ty: _, value } = fn_name_symbol {
                        Expression::BytesLiteral {
                            loc,
                            ty: Type::String,
                            value,
                        }
                    } else {
                        unreachable!()
                    };
                let encode_func_symbol =
                    soroban_encode(loc, vec![symbol_string], ns, vartab, cfg, false).2[0].clone();

                let mut args_vec = Vec::new();
                for arg in args.iter().skip(2) {
                    let arg = expression(arg, cfg, contract_no, func, ns, vartab, opt, self);
                    args_vec.push(arg);
                }

                let args_encoded = self.abi_encode(loc, args_vec.clone(), ns, vartab, cfg, false);

                let args_buf = args_encoded.0;

                let args_buf_ptr = Expression::VectorData {
                    pointer: Box::new(args_buf.clone()),
                };

                let args_buf_extended = Expression::ZeroExt {
                    loc: pt::Loc::Codegen,
                    ty: Type::Uint(64),
                    expr: Box::new(args_buf_ptr.clone()),
                };

                let args_buf_shifted = Expression::ShiftLeft {
                    loc: pt::Loc::Codegen,
                    ty: Type::Uint(64),
                    left: Box::new(args_buf_extended.clone()),
                    right: Box::new(Expression::NumberLiteral {
                        loc: pt::Loc::Codegen,
                        ty: Type::Uint(64),
                        value: BigInt::from(32),
                    }),
                };

                let args_buf_pos = Expression::Add {
                    loc: pt::Loc::Codegen,
                    ty: Type::Uint(64),
                    left: Box::new(args_buf_shifted.clone()),
                    right: Box::new(Expression::NumberLiteral {
                        loc: pt::Loc::Codegen,
                        ty: Type::Uint(64),
                        value: BigInt::from(4),
                    }),
                    overflowing: false,
                };

                let args_len = Expression::NumberLiteral {
                    loc: pt::Loc::Codegen,
                    ty: Type::Uint(64),
                    value: BigInt::from(args_vec.len()),
                };
                let args_len_encoded = Expression::ShiftLeft {
                    loc: pt::Loc::Codegen,
                    ty: Type::Uint(64),
                    left: Box::new(args_len.clone()),
                    right: Box::new(Expression::NumberLiteral {
                        loc: pt::Loc::Codegen,
                        ty: Type::Uint(64),
                        value: BigInt::from(32),
                    }),
                };
                let args_len_encoded = Expression::Add {
                    loc: pt::Loc::Codegen,
                    ty: Type::Uint(64),
                    left: Box::new(args_len_encoded.clone()),
                    right: Box::new(Expression::NumberLiteral {
                        loc: pt::Loc::Codegen,
                        ty: Type::Uint(64),
                        value: BigInt::from(4),
                    }),
                    overflowing: false,
                };

                let args_vec_var_no = vartab.temp_anonymous(&Type::Uint(64));
                let args_vec_var = Expression::Variable {
                    loc: pt::Loc::Codegen,
                    ty: Type::Uint(64),
                    var_no: args_vec_var_no,
                };

                let vec_new_from_linear_mem = Instr::Call {
                    res: vec![args_vec_var_no],
                    return_tys: vec![Type::Uint(64)],
                    call: InternalCallTy::HostFunction {
                        name: HostFunctions::VectorNewFromLinearMemory.name().to_string(),
                    },
                    args: vec![args_buf_pos.clone(), args_len_encoded],
                };

                cfg.add(vartab, vec_new_from_linear_mem);

                let context_map = vartab.temp_anonymous(&Type::Uint(64));
                let context_map_var = Expression::Variable {
                    loc: pt::Loc::Codegen,
                    ty: Type::Uint(64),
                    var_no: context_map,
                };

                let context_map_new = Instr::Call {
                    res: vec![context_map],
                    return_tys: vec![Type::Uint(64)],
                    call: InternalCallTy::HostFunction {
                        name: HostFunctions::MapNew.name().to_string(),
                    },
                    args: vec![],
                };

                cfg.add(vartab, context_map_new);

                let context_map_put = Instr::Call {
                    res: vec![context_map],
                    return_tys: vec![Type::Uint(64)],
                    call: InternalCallTy::HostFunction {
                        name: HostFunctions::MapPut.name().to_string(),
                    },
                    args: vec![context_map_var.clone(), symbols[0].clone(), contract_value],
                };

                cfg.add(vartab, context_map_put);

                let context_map_put_2 = Instr::Call {
                    res: vec![context_map],
                    return_tys: vec![Type::Uint(64)],
                    call: InternalCallTy::HostFunction {
                        name: HostFunctions::MapPut.name().to_string(),
                    },
                    args: vec![
                        context_map_var.clone(),
                        symbols[1].clone(),
                        encode_func_symbol,
                    ],
                };

                cfg.add(vartab, context_map_put_2);

                let context_map_put_3 = Instr::Call {
                    res: vec![context_map],
                    return_tys: vec![Type::Uint(64)],
                    call: InternalCallTy::HostFunction {
                        name: HostFunctions::MapPut.name().to_string(),
                    },
                    args: vec![
                        context_map_var.clone(),
                        symbols[2].clone(),
                        args_vec_var.clone(),
                    ],
                };

                cfg.add(vartab, context_map_put_3);

                // Now forming "sub invocations" map
                // FIXME: This should eventually be fixed to take other sub_invocations as arguments. For now, it is hardcoded to take an empty vector.

                let key_1 = Expression::BytesLiteral {
                    loc: pt::Loc::Codegen,
                    ty: Type::String,
                    value: "context".as_bytes().to_vec(),
                };

                let key_2 = Expression::BytesLiteral {
                    loc: pt::Loc::Codegen,
                    ty: Type::String,
                    value: "sub_invocations".as_bytes().to_vec(),
                };

                let keys = soroban_encode(loc, vec![key_1, key_2], ns, vartab, cfg, false).2;

                let sub_invocations_map = vartab.temp_anonymous(&Type::Uint(64));
                let sub_invocations_map_var = Expression::Variable {
                    loc: pt::Loc::Codegen,
                    ty: Type::Uint(64),
                    var_no: sub_invocations_map,
                };

                let sub_invocations_map_new = Instr::Call {
                    res: vec![sub_invocations_map],
                    return_tys: vec![Type::Uint(64)],
                    call: InternalCallTy::HostFunction {
                        name: HostFunctions::MapNew.name().to_string(),
                    },
                    args: vec![],
                };

                cfg.add(vartab, sub_invocations_map_new);

                let sub_invocations_map_put = Instr::Call {
                    res: vec![sub_invocations_map],
                    return_tys: vec![Type::Uint(64)],
                    call: InternalCallTy::HostFunction {
                        name: HostFunctions::MapPut.name().to_string(),
                    },
                    args: vec![
                        sub_invocations_map_var.clone(),
                        keys[0].clone(),
                        context_map_var,
                    ],
                };

                cfg.add(vartab, sub_invocations_map_put);

                let empy_vec_var = vartab.temp_anonymous(&Type::Uint(64));
                let empty_vec_expr = Expression::Variable {
                    loc: pt::Loc::Codegen,
                    ty: Type::Uint(64),
                    var_no: empy_vec_var,
                };
                let empty_vec = Instr::Call {
                    res: vec![empy_vec_var],
                    return_tys: vec![Type::Uint(64)],
                    call: InternalCallTy::HostFunction {
                        name: HostFunctions::VectorNew.name().to_string(),
                    },
                    args: vec![],
                };

                cfg.add(vartab, empty_vec);

                let sub_invocations_map_put_2 = Instr::Call {
                    res: vec![sub_invocations_map],
                    return_tys: vec![Type::Uint(64)],
                    call: InternalCallTy::HostFunction {
                        name: HostFunctions::MapPut.name().to_string(),
                    },
                    args: vec![
                        sub_invocations_map_var.clone(),
                        keys[1].clone(),
                        empty_vec_expr,
                    ],
                };

                cfg.add(vartab, sub_invocations_map_put_2);

                // now forming the enum. The enum is a VecObject[Symbol("Contract"), sub invokations map].
                // FIXME: This should use VecNewFromLinearMemory to create the enum in one go.

                let contract_capitalized = Expression::BytesLiteral {
                    loc: pt::Loc::Codegen,
                    ty: Type::String,
                    value: "Contract".as_bytes().to_vec(),
                };

                let contract_capitalized =
                    soroban_encode(loc, vec![contract_capitalized], ns, vartab, cfg, false).2[0]
                        .clone();

                let enum_vec = vartab.temp_anonymous(&Type::Uint(64));
                let enum_vec_var = Expression::Variable {
                    loc: pt::Loc::Codegen,
                    ty: Type::Uint(64),
                    var_no: enum_vec,
                };

                let enum_vec_new = Instr::Call {
                    res: vec![enum_vec],
                    return_tys: vec![Type::Uint(64)],
                    call: InternalCallTy::HostFunction {
                        name: HostFunctions::VectorNew.name().to_string(),
                    },
                    args: vec![],
                };

                cfg.add(vartab, enum_vec_new);

                let enum_vec_put = Instr::Call {
                    res: vec![enum_vec],
                    return_tys: vec![Type::Uint(64)],
                    call: InternalCallTy::HostFunction {
                        name: HostFunctions::VecPushBack.name().to_string(),
                    },
                    args: vec![enum_vec_var.clone(), contract_capitalized],
                };

                cfg.add(vartab, enum_vec_put);

                let enum_vec_put_2 = Instr::Call {
                    res: vec![enum_vec],
                    return_tys: vec![Type::Uint(64)],
                    call: InternalCallTy::HostFunction {
                        name: HostFunctions::VecPushBack.name().to_string(),
                    },
                    args: vec![enum_vec_var.clone(), sub_invocations_map_var],
                };

                cfg.add(vartab, enum_vec_put_2);

                let vec = vartab.temp_anonymous(&Type::Uint(64));
                let vec_var = Expression::Variable {
                    loc: pt::Loc::Codegen,
                    ty: Type::Uint(64),
                    var_no: vec,
                };

                let vec_new = Instr::Call {
                    res: vec![vec],
                    return_tys: vec![Type::Uint(64)],
                    call: InternalCallTy::HostFunction {
                        name: HostFunctions::VectorNew.name().to_string(),
                    },
                    args: vec![],
                };

                cfg.add(vartab, vec_new);

                let vec_push_back = Instr::Call {
                    res: vec![vec],
                    return_tys: vec![Type::Uint(64)],
                    call: InternalCallTy::HostFunction {
                        name: HostFunctions::VecPushBack.name().to_string(),
                    },
                    args: vec![vec_var.clone(), enum_vec_var],
                };

                cfg.add(vartab, vec_push_back);

                let call_res = vartab.temp_anonymous(&Type::Uint(64));
                let call_res_var = Expression::Variable {
                    loc: pt::Loc::Codegen,
                    ty: Type::Uint(64),
                    var_no: call_res,
                };

                let auth_call = Instr::Call {
                    res: vec![call_res],
                    return_tys: vec![Type::Void],
                    call: InternalCallTy::HostFunction {
                        name: HostFunctions::AuthAsCurrContract.name().to_string(),
                    },
                    args: vec![vec_var],
                };

                cfg.add(vartab, auth_call);

                Some(call_res_var)
            }
            ast::Builtin::ExtendTtl => {
                assert_eq!(args.len(), 3, "extendTtl expects 3 arguments");
                let arguments: Vec<Expression> = args
                    .iter()
                    .map(|v| expression(v, cfg, contract_no, func, ns, vartab, opt, self))
                    .collect();

                let var_no = match arguments[0].clone() {
                    Expression::NumberLiteral { value, .. } => value,
                    _ => panic!("First argument of extendTtl() must be a number literal"),
                }
                .to_usize()
                .expect("Unable to convert var_no to usize");
                let var = ns.contracts[contract_no].variables.get(var_no).unwrap();
                let storage_type_usize = match var
                    .storage_type
                    .clone()
                    .expect("Unable to get storage type")
                {
                    pt::StorageType::Temporary(_) => 0,
                    pt::StorageType::Persistent(_) => 1,
                    pt::StorageType::Instance(_) => panic!(
                        "Calling extendTtl() on instance storage is not allowed. Use `extendInstanceTtl()` instead."
                    ),
                };

                let slot = Expression::NumberLiteral {
                    loc: *loc,
                    ty: Type::Uint(64),
                    value: BigInt::from(var_no),
                };
                let storage_type = Expression::NumberLiteral {
                    loc: *loc,
                    ty: Type::Uint(64),
                    value: BigInt::from(storage_type_usize),
                };
                let threshold =
                    encoding::encode_object(*loc, arguments[1].clone(), 32, encoding::tags::U32);
                let extend_to =
                    encoding::encode_object(*loc, arguments[2].clone(), 32, encoding::tags::U32);

                // The host functions return Void, so the result is not decoded.
                let res = vartab.temp_name("extend_ttl", &Type::Int(64));
                cfg.add(
                    vartab,
                    Instr::Call {
                        res: vec![res],
                        return_tys: vec![Type::Int(64)],
                        call: InternalCallTy::HostFunction {
                            name: HostFunctions::ExtendContractDataTtl.name().to_string(),
                        },
                        args: vec![slot, storage_type, threshold, extend_to],
                    },
                );
                Some(Expression::Variable {
                    loc: *loc,
                    ty: Type::Int(64),
                    var_no: res,
                })
            }
            ast::Builtin::ExtendInstanceTtl => {
                assert_eq!(args.len(), 2, "extendInstanceTtl expects 2 arguments");
                let arguments: Vec<Expression> = args
                    .iter()
                    .map(|v| expression(v, cfg, contract_no, func, ns, vartab, opt, self))
                    .collect();

                let threshold =
                    encoding::encode_object(*loc, arguments[0].clone(), 32, encoding::tags::U32);
                let extend_to =
                    encoding::encode_object(*loc, arguments[1].clone(), 32, encoding::tags::U32);

                // The host functions return Void, so the result is not decoded.
                let res = vartab.temp_name("extend_instance_ttl", &Type::Int(64));
                cfg.add(
                    vartab,
                    Instr::Call {
                        res: vec![res],
                        return_tys: vec![Type::Int(64)],
                        call: InternalCallTy::HostFunction {
                            name: HostFunctions::ExtendCurrentContractInstanceAndCodeTtl
                                .name()
                                .to_string(),
                        },
                        args: vec![threshold, extend_to],
                    },
                );
                Some(Expression::Variable {
                    loc: *loc,
                    ty: Type::Int(64),
                    var_no: res,
                })
            }
            ast::Builtin::UpdateCurrentContractWasm => {
                assert_eq!(
                    args.len(),
                    1,
                    "updateCurrentContractWasm expects 1 argument"
                );

                let input = expression(&args[0], cfg, contract_no, func, ns, vartab, opt, self);
                let wasm_hash = soroban_encode_arg(input, cfg, vartab, ns);

                let res = vartab.temp_name("update_current_contract_wasm", &Type::Uint(64));
                cfg.add(
                    vartab,
                    Instr::Call {
                        res: vec![res],
                        return_tys: vec![Type::Void],
                        call: InternalCallTy::HostFunction {
                            name: HostFunctions::UpdateCurrentContractWasm.name().to_string(),
                        },
                        args: vec![wasm_hash],
                    },
                );

                Some(Expression::Variable {
                    loc: *loc,
                    ty: Type::Uint(64),
                    var_no: res,
                })
            }
            _ => None,
        }
    }

    fn lower_storage_struct_member(
        &self,
        loc: &pt::Loc,
        var_expr: Expression,
        struct_ty: &StructType,
        field_no: usize,
        ns: &Namespace,
        cfg: &mut ControlFlowGraph,
        vartab: &mut Vartable,
    ) -> Expression {
        let offset: BigInt = struct_ty.definition(ns).fields[..field_no]
            .iter()
            .filter(|field| !field.infinite_size)
            .map(|field| field.ty.storage_slots(ns))
            .sum();
        let offset_expr = Expression::NumberLiteral {
            loc: *loc,
            ty: Type::Uint(32),
            value: offset,
        };
        let offset_encoded = soroban_encode_arg(offset_expr, cfg, vartab, ns);
        let res = vartab.temp_name("vec_push_codegen", &Type::Uint(64));
        cfg.add(
            vartab,
            Instr::Call {
                res: vec![res],
                return_tys: vec![Type::Uint(64)],
                call: InternalCallTy::HostFunction {
                    name: HostFunctions::VecPushBack.name().to_string(),
                },
                args: vec![var_expr, offset_encoded],
            },
        );
        Expression::Variable {
            loc: pt::Loc::Codegen,
            ty: Type::Uint(64),
            var_no: res,
        }
    }

    fn lower_load_storage(
        &self,
        value: Expression,
        cfg: &mut ControlFlowGraph,
        vartab: &mut Vartable,
        ns: &Namespace,
    ) -> Expression {
        soroban_storage_decode_arg(value, cfg, vartab, ns, None)
    }

    fn post_process_program(&self, _ns: &mut Namespace, _opt: &Options) {}

    fn selector_hash_algorithm(&self) -> ast::Builtin {
        ast::Builtin::Keccak256
    }

    fn initial_storage_slot(&self) -> BigInt {
        BigInt::zero()
    }

    fn align_storage_slot(&self, slot: BigInt, _ty: &Type, _ns: &Namespace) -> BigInt {
        slot
    }

    fn default_gas_builtin(&self) -> BigInt {
        BigInt::zero()
    }

    fn lower_print_expr(&self, expr: Expression) -> Expression {
        expr
    }

    fn lower_mapping_subscript(
        &self,
        loc: &pt::Loc,
        elem_ty: &Type,
        array_ty: &Type,
        array: Expression,
        index: Expression,
    ) -> Expression {
        Expression::Subscript {
            loc: *loc,
            ty: elem_ty.clone(),
            array_ty: array_ty.clone(),
            expr: Box::new(array),
            index: Box::new(index),
        }
    }
}

pub(crate) fn soroban_storage_assign(
    left: &ast::Expression,
    cfg_right: Expression,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
    target: &dyn TargetCodegen,
) -> Option<Expression> {
    if !left.ty().is_contract_storage() {
        return None;
    }
    if let ast::Expression::Subscript {
        array_ty,
        array,
        index,
        ..
    } = left
    {
        if array_ty.is_storage_bytes() {
            if !is_descent_storage_expr(array) {
                return None;
            }
            let val_ty = cfg_right.ty();
            let pos = vartab.temp_anonymous(&val_ty);
            cfg.add(
                vartab,
                Instr::Set {
                    loc: left.loc(),
                    res: pos,
                    expr: cfg_right,
                },
            );
            let value = Expression::Variable {
                loc: left.loc(),
                ty: val_ty,
                var_no: pos,
            };
            bytes::soroban_storage_bytes_subscript_write(
                array,
                index,
                value.clone(),
                cfg,
                contract_no,
                func,
                ns,
                vartab,
                opt,
                target,
            );
            return Some(value);
        }
    }

    let val_ty = cfg_right.ty();
    let pos = vartab.temp_anonymous(&val_ty);
    cfg.add(
        vartab,
        Instr::Set {
            loc: left.loc(),
            res: pos,
            expr: cfg_right,
        },
    );
    let value = Expression::Variable {
        loc: left.loc(),
        ty: val_ty,
        var_no: pos,
    };
    let encoded = soroban_storage_encode_arg(value.clone(), cfg, vartab, ns);
    let (_, loc, storage_type) =
        lower_storage_path(left, cfg, contract_no, func, ns, vartab, opt, target);
    path_store(&loc, encoded, &storage_type, cfg, vartab, ns);

    Some(value)
}

pub(crate) fn soroban_storage_delete(
    expr: &ast::Expression,
    ty: &Type,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
    target: &dyn TargetCodegen,
) -> bool {
    if !is_descent_storage_expr(expr) {
        return false;
    }
    let (_, path, storage_type) =
        lower_storage_path(expr, cfg, contract_no, func, ns, vartab, opt, target);
    match path.idxs.last() {
        Some(Idx::Map { .. }) => path_delete_map(&path, &storage_type, cfg, vartab, ns),
        Some(Idx::Field(_) | Idx::Array(_)) => {
            let def = soroban_default_handle(&expr.loc(), ty, cfg, vartab, ns);
            path_store(&path, def, &storage_type, cfg, vartab, ns);
        }
        None => return false,
    }
    true
}

pub(crate) fn soroban_storage_load(
    loc: &pt::Loc,
    base: &ast::Expression,
    ty: &Type,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
    target: &dyn TargetCodegen,
) -> Expression {
    let is_bytes_subscript = matches!(base,
        ast::Expression::Subscript { array_ty, .. } if array_ty.is_storage_bytes());

    // Struct member / array element: descend the single VecObject ledger entry
    // (load root handle, vec_get each index) then decode the value.
    if !is_bytes_subscript && is_descent_storage_expr(base) {
        let (_, path, storage_type) =
            lower_storage_path(base, cfg, contract_no, func, ns, vartab, opt, target);
        let handle = if path.idxs.iter().any(|i| matches!(i, Idx::Map { .. })) {
            path_load_map(&path, ty, &storage_type, cfg, vartab, ns)
        } else {
            path_load(&path, &storage_type, cfg, vartab, ns)
        };
        return soroban_storage_decode_arg(handle, cfg, vartab, ns, Some(ty.clone()));
    }

    if matches!(ty, Type::Struct(_)) {
        let storage_ty = storage_type(base, ns);
        let storage = expression(base, cfg, contract_no, func, ns, vartab, opt, target);
        let handle = load_raw_handle(loc, storage, storage_ty, cfg, vartab);
        return soroban_storage_decode_arg(handle, cfg, vartab, ns, Some(ty.clone()));
    }

    let storage_type = storage_type(base, ns);
    let storage = expression(base, cfg, contract_no, func, ns, vartab, opt, target);
    load_storage(loc, ty, storage, cfg, vartab, storage_type, ns, target)
}

pub(crate) fn soroban_storage_incdec(
    loc: &pt::Loc,
    var: &ast::Expression,
    ty: &Type,
    node: &ast::Expression,
    overflowing: bool,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
    target: &dyn TargetCodegen,
) -> Option<Expression> {
    if !is_descent_storage_expr(var) {
        return None;
    }

    let (is_decrement, is_post) = match node {
        ast::Expression::PostDecrement { .. } => (true, true),
        ast::Expression::PostIncrement { .. } => (false, true),
        ast::Expression::PreDecrement { .. } => (true, false),
        ast::Expression::PreIncrement { .. } => (false, false),
        _ => unreachable!("soroban_storage_incdec: not an inc/dec node"),
    };

    let (_, path, storage_type) =
        lower_storage_path(var, cfg, contract_no, func, ns, vartab, opt, target);

    let handle = path_load(&path, &storage_type, cfg, vartab, ns);
    let old = soroban_storage_decode_arg(handle, cfg, vartab, ns, Some(ty.clone()));
    let old_no = vartab.temp_anonymous(ty);
    cfg.add(
        vartab,
        Instr::Set {
            loc: *loc,
            res: old_no,
            expr: old,
        },
    );
    let old_var = Expression::Variable {
        loc: *loc,
        ty: ty.clone(),
        var_no: old_no,
    };

    let one = Box::new(Expression::NumberLiteral {
        loc: *loc,
        ty: ty.clone(),
        value: BigInt::from(1u8),
    });
    let new_expr = if is_decrement {
        Expression::Subtract {
            loc: *loc,
            ty: ty.clone(),
            overflowing,
            left: Box::new(old_var.clone()),
            right: one,
        }
    } else {
        Expression::Add {
            loc: *loc,
            ty: ty.clone(),
            overflowing,
            left: Box::new(old_var.clone()),
            right: one,
        }
    };
    let new_no = vartab.temp_anonymous(ty);
    cfg.add(
        vartab,
        Instr::Set {
            loc: *loc,
            res: new_no,
            expr: new_expr,
        },
    );
    let new_var = Expression::Variable {
        loc: *loc,
        ty: ty.clone(),
        var_no: new_no,
    };

    let encoded = soroban_storage_encode_arg(new_var.clone(), cfg, vartab, ns);
    path_store(&path, encoded, &storage_type, cfg, vartab, ns);

    Some(if is_post { old_var } else { new_var })
}

pub(crate) fn soroban_storage_array_length_ast(
    array: &ast::Expression,
    ty: &Type,
    cfg: &mut ControlFlowGraph,
    contract_no: usize,
    func: Option<&Function>,
    ns: &Namespace,
    vartab: &mut Vartable,
    opt: &Options,
    target: &dyn TargetCodegen,
) -> Option<Expression> {
    let aty = array.ty();
    let len_kind = match &aty {
        Type::StorageRef(_, inner) => match inner.as_ref() {
            Type::Array(_, dims) if dims.last() == Some(&ast::ArrayLength::Dynamic) => {
                LengthKind::Vec
            }
            Type::DynamicBytes => LengthKind::Bytes,
            Type::String => LengthKind::String,
            _ => return None,
        },
        _ => return None,
    };

    let (_, path, storage_type) =
        lower_storage_path(array, cfg, contract_no, func, ns, vartab, opt, target);
    let handle = path_load(&path, &storage_type, cfg, vartab, ns);
    Some(match len_kind {
        LengthKind::Vec => arrays::soroban_vec_len(&array.loc(), ty, handle, cfg, vartab, ns),
        LengthKind::Bytes => bytes::soroban_obj_length(
            &array.loc(),
            handle,
            HostFunctions::BytesLen,
            cfg,
            vartab,
            ns,
        ),
        LengthKind::String => bytes::soroban_obj_length(
            &array.loc(),
            handle,
            HostFunctions::StringLen,
            cfg,
            vartab,
            ns,
        ),
    })
}

enum LengthKind {
    Vec,
    Bytes,
    String,
}

pub(crate) fn soroban_host_call(
    loc: &pt::Loc,
    name: &str,
    host_fn: HostFunctions,
    ret_ty: &Type,
    args: Vec<Expression>,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
) -> Expression {
    let no = vartab.temp_name(name, ret_ty);
    cfg.add(
        vartab,
        Instr::Call {
            res: vec![no],
            return_tys: vec![ret_ty.clone()],
            call: InternalCallTy::HostFunction {
                name: host_fn.name().to_string(),
            },
            args,
        },
    );
    Expression::Variable {
        loc: *loc,
        ty: ret_ty.clone(),
        var_no: no,
    }
}

pub(super) fn validate_accessor_abi_types(contract_no: usize, ns: &mut Namespace) {
    for variable in &ns.contracts[contract_no].variables {
        if !matches!(variable.visibility, pt::Visibility::Public(_)) {
            continue;
        }

        if let Some(unsupported_type) = unsupported_accessor_type(&variable.ty, ns) {
            ns.diagnostics.push(Diagnostic::error(
                variable.loc,
                format!(
                    "type '{unsupported_type}' is not supported as a Soroban public variable accessor return value"
                ),
            ));
        }
    }
}

pub(super) fn validate_event_abi_types(contract_no: usize, ns: &mut Namespace) {
    for event_no in ns.contracts[contract_no].emits_events.clone() {
        for field in &ns.events[event_no].fields {
            if let Some(unsupported_type) = unsupported_event_type(&field.ty, ns) {
                ns.diagnostics.push(Diagnostic::error(
                    field.ty_loc.unwrap_or(field.loc),
                    format!(
                        "type '{unsupported_type}' is not supported as a Soroban event parameter"
                    ),
                ));
            }
        }
    }
}

pub(super) fn validate_abi_types(all_cfg: &[ControlFlowGraph], ns: &mut Namespace) {
    for cfg in all_cfg {
        if !cfg.public {
            continue;
        }

        if is_public_accessor(cfg, ns) {
            continue;
        }

        if cfg.returns.len() > 1 {
            let loc = cfg.returns[1].ty_loc.unwrap_or(cfg.returns[1].loc);
            ns.diagnostics.push(Diagnostic::error(
                loc,
                "Soroban external functions can return at most one value".to_string(),
            ));
            continue;
        }

        for param in cfg.params.as_ref() {
            if let Some(unsupported_type) = unsupported_parameter_type(&param.ty, ns) {
                ns.diagnostics.push(Diagnostic::error(
                    param.ty_loc.unwrap_or(param.loc),
                    format!(
                        "type '{unsupported_type}' is not supported as a Soroban external function parameter"
                    ),
                ));
            }
        }

        for ret in cfg.returns.as_ref() {
            if let Some(unsupported_type) = unsupported_return_type(&ret.ty, ns) {
                ns.diagnostics.push(Diagnostic::error(
                    ret.ty_loc.unwrap_or(ret.loc),
                    format!(
                        "type '{unsupported_type}' is not supported as a Soroban external function return value"
                    ),
                ));
            }
        }
    }

    validate_unsupported_codegen_paths(all_cfg, ns);
}

fn soroban_struct_field_unsupported(ty: &Type, ns: &Namespace) -> Option<String> {
    match ty {
        Type::Array(elem, dims) => {
            if dims.last() == Some(&ast::ArrayLength::AnyFixed)
                || has_unsupported_soroban_array_element(elem.as_ref())
            {
                Some(ty.to_string(ns))
            } else {
                soroban_struct_field_unsupported(elem.as_ref(), ns)
            }
        }
        Type::Struct(struct_ty) => struct_ty
            .definition(ns)
            .fields
            .iter()
            .find_map(|field| soroban_struct_field_unsupported(&field.ty, ns)),
        _ => None,
    }
}

fn unsupported_parameter_type(ty: &Type, ns: &Namespace) -> Option<String> {
    match ty {
        Type::Struct(_) => {
            soroban_struct_field_unsupported(ty, ns).map(|_| format!("{} memory", ty.to_string(ns)))
        }
        Type::Array(elem, _) if has_unsupported_soroban_array_element(elem.as_ref()) => {
            Some(format!("{} memory", ty.to_string(ns)))
        }
        _ => None,
    }
}

fn is_public_accessor(cfg: &ControlFlowGraph, ns: &Namespace) -> bool {
    match cfg.function_no {
        ASTFunction::SolidityFunction(function_no) => ns.functions[function_no].is_accessor,
        _ => false,
    }
}

fn unsupported_accessor_type(ty: &Type, ns: &Namespace) -> Option<String> {
    match ty {
        Type::Mapping(mapping) => unsupported_accessor_type(mapping.value.as_ref(), ns),
        Type::Array(elem, _) => unsupported_accessor_type(elem.as_ref(), ns),
        Type::Struct(struct_ty) => {
            let fields = &struct_ty.definition(ns).fields;

            if fields.len() > 1 {
                Some(ty.to_string(ns))
            } else {
                fields
                    .first()
                    .and_then(|field| unsupported_accessor_type(&field.ty, ns))
            }
        }
        _ => None,
    }
}

fn unsupported_event_type(ty: &Type, ns: &Namespace) -> Option<String> {
    match ty {
        Type::Struct(_) => Some(ty.to_string(ns)),
        _ => None,
    }
}

fn unsupported_return_type(ty: &Type, ns: &Namespace) -> Option<String> {
    match ty {
        Type::Struct(_) => {
            soroban_struct_field_unsupported(ty, ns).map(|_| format!("{} memory", ty.to_string(ns)))
        }
        Type::Array(elem, _) if has_unsupported_soroban_array_element(elem.as_ref()) => {
            Some(format!("{} memory", ty.to_string(ns)))
        }
        _ => None,
    }
}

fn has_unsupported_soroban_array_element(ty: &Type) -> bool {
    match ty {
        Type::Mapping(..) => true,
        Type::Array(elem, _) => has_unsupported_soroban_array_element(elem.as_ref()),
        _ => false,
    }
}

fn validate_unsupported_codegen_paths(all_cfg: &[ControlFlowGraph], ns: &mut Namespace) {
    for cfg in all_cfg {
        for block in &cfg.blocks {
            for instr in &block.instr {
                validate_unsupported_codegen_instr(instr, ns);
            }
        }
    }
}

fn validate_unsupported_codegen_instr(instr: &Instr, ns: &mut Namespace) {
    match instr {
        Instr::LoadStorage { ty, storage, .. } => {
            if let Some(unsupported_type) = unsupported_soroban_storage_type(ty, ns) {
                push_codegen_error(
                    ns,
                    CodegenError::unsupported_soroban_type(
                        storage.loc(),
                        "in storage load",
                        unsupported_type,
                    ),
                );
            }
        }
        Instr::SetStorage { ty, storage, .. } => {
            if let Some(unsupported_type) = unsupported_soroban_storage_type(ty, ns) {
                push_codegen_error(
                    ns,
                    CodegenError::unsupported_soroban_type(
                        storage.loc(),
                        "in storage store",
                        unsupported_type,
                    ),
                );
            }
        }
        Instr::PushStorage { ty, storage, .. } => {
            if let Some(unsupported_type) = unsupported_soroban_storage_type(ty, ns) {
                push_codegen_error(
                    ns,
                    CodegenError::unsupported_soroban_type(
                        storage.loc(),
                        "in storage push",
                        unsupported_type,
                    ),
                );
            }
        }
        Instr::PopStorage { ty, storage, .. } => {
            if let Some(unsupported_type) = unsupported_soroban_storage_type(ty, ns) {
                push_codegen_error(
                    ns,
                    CodegenError::unsupported_soroban_type(
                        storage.loc(),
                        "in storage pop",
                        unsupported_type,
                    ),
                );
            }
        }
        Instr::Constructor { loc, .. } => {
            push_codegen_error(
                ns,
                CodegenError::unsupported_soroban_operation(*loc, "contract construction"),
            );
        }
        Instr::ValueTransfer { address, .. } => {
            push_codegen_error(
                ns,
                CodegenError::unsupported_soroban_operation(address.loc(), "value transfer"),
            );
        }
        Instr::SelfDestruct { recipient } => {
            push_codegen_error(
                ns,
                CodegenError::unsupported_soroban_operation(recipient.loc(), "selfdestruct"),
            );
        }
        _ => (),
    }
}

fn unsupported_soroban_storage_type(ty: &Type, ns: &Namespace) -> Option<String> {
    match ty.deref_any() {
        Type::ExternalFunction { .. } => Some(ty.to_string(ns)),
        Type::Array(elem, _) => unsupported_soroban_storage_type(elem.as_ref(), ns),
        Type::Mapping(mapping) => unsupported_soroban_storage_type(mapping.value.as_ref(), ns),
        Type::Struct(struct_ty) => struct_ty
            .definition(ns)
            .fields
            .iter()
            .find_map(|field| unsupported_soroban_storage_type(&field.ty, ns)),
        _ => None,
    }
}

fn push_codegen_error(ns: &mut Namespace, err: CodegenError) {
    if let Some(diagnostic) = err.diagnostic() {
        ns.diagnostics.push(diagnostic);
    }
}

pub(crate) fn soroban_vec_handle_ty(vec_ty: &Type) -> Type {
    let inner_ty = if let Type::StorageRef(_, inner) = vec_ty {
        inner.as_ref().clone()
    } else {
        vec_ty.clone()
    };

    Type::SorobanHandle(Box::new(inner_ty))
}

pub(crate) fn soroban_vec_new(
    loc: &pt::Loc,
    vec_ty: &Type,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
) -> Expression {
    let handle_ty = soroban_vec_handle_ty(vec_ty);
    let empty_vec_no = vartab.temp_name("soroban_vec_new", &handle_ty);

    let empty_vec_var = Expression::Variable {
        loc: *loc,
        ty: handle_ty.clone(),
        var_no: empty_vec_no,
    };

    cfg.add(
        vartab,
        Instr::Call {
            call: InternalCallTy::HostFunction {
                name: HostFunctions::VectorNew.name().to_string(),
            },
            args: vec![],
            return_tys: vec![handle_ty],
            res: vec![empty_vec_no],
        },
    );

    empty_vec_var
}

fn soroban_field_index_val(
    loc: &pt::Loc,
    field_no: usize,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    ns: &Namespace,
) -> Expression {
    soroban_encode_arg(
        Expression::NumberLiteral {
            loc: *loc,
            ty: Type::Uint(32),
            value: BigInt::from(field_no),
        },
        cfg,
        vartab,
        ns,
    )
}

fn soroban_struct_default_vec(
    loc: &pt::Loc,
    struct_no: usize,
    struct_ty: &Type,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    ns: &Namespace,
) -> Expression {
    let handle_ty = Type::SorobanHandle(Box::new(struct_ty.clone()));
    let vec_no = vartab.temp_name("struct_default", &handle_ty);
    cfg.add(
        vartab,
        Instr::Call {
            res: vec![vec_no],
            return_tys: vec![handle_ty.clone()],
            call: InternalCallTy::HostFunction {
                name: HostFunctions::VectorNew.name().to_string(),
            },
            args: vec![],
        },
    );

    let fields: Vec<Type> = ns.structs[struct_no]
        .fields
        .iter()
        .map(|f| f.ty.clone())
        .collect();

    let mut current_vec_no = vec_no;
    for field_ty in &fields {
        let elem = soroban_default_handle(loc, field_ty, cfg, vartab, ns);
        let prev_vec = Expression::Variable {
            loc: *loc,
            ty: handle_ty.clone(),
            var_no: current_vec_no,
        };
        let next_vec_no = vartab.temp_name("struct_default", &handle_ty);
        cfg.add(
            vartab,
            Instr::Call {
                res: vec![next_vec_no],
                return_tys: vec![handle_ty.clone()],
                call: InternalCallTy::HostFunction {
                    name: HostFunctions::VecPushBack.name().to_string(),
                },
                args: vec![prev_vec, elem],
            },
        );
        current_vec_no = next_vec_no;
    }

    Expression::Variable {
        loc: *loc,
        ty: handle_ty,
        var_no: current_vec_no,
    }
}

fn soroban_scval_zero(
    loc: &pt::Loc,
    ty: &Type,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
) -> Expression {
    use encoding::tags;
    let tag: u64 = match ty {
        Type::Bool => tags::FALSE,
        Type::Uint(32) => tags::U32,
        Type::Int(32) => tags::I32,
        Type::Uint(64) => tags::U64_SML,
        Type::Int(64) => tags::I64_SML,
        Type::Uint(128) => tags::U128_SML,
        Type::Int(128) => tags::I128_SML,
        Type::Uint(256) => tags::U256_SML,
        Type::Int(256) => tags::I256_SML,
        Type::Enum(_) => tags::U32,
        _ => tags::VOID,
    };
    let handle_ty = Type::SorobanHandle(Box::new(ty.clone()));
    let tmp = vartab.temp_anonymous(&handle_ty);
    cfg.add(
        vartab,
        Instr::Set {
            loc: *loc,
            res: tmp,
            expr: Expression::NumberLiteral {
                loc: *loc,
                ty: Type::Uint(64),
                value: BigInt::from(tag),
            },
        },
    );
    Expression::Variable {
        loc: *loc,
        ty: handle_ty,
        var_no: tmp,
    }
}

fn soroban_as_handle(expr: Expression, inner_ty: &Type) -> Expression {
    match expr {
        Expression::Variable { loc, var_no, .. } => Expression::Variable {
            loc,
            ty: Type::SorobanHandle(Box::new(inner_ty.clone())),
            var_no,
        },
        other => other,
    }
}

pub(crate) fn soroban_default_handle(
    loc: &pt::Loc,
    ty: &Type,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    ns: &Namespace,
) -> Expression {
    match ty {
        Type::Bool | Type::Uint(_) | Type::Int(_) | Type::Enum(_) => {
            soroban_scval_zero(loc, ty, cfg, vartab)
        }
        Type::Address(_) => soroban_scval_zero(loc, &Type::Void, cfg, vartab),
        Type::DynamicBytes => bytes::soroban_bytes_new(loc, cfg, vartab),
        Type::Bytes(_) => {
            let zero = Expression::NumberLiteral {
                loc: *loc,
                ty: ty.clone(),
                value: BigInt::zero(),
            };
            soroban_as_handle(soroban_encode_arg(zero, cfg, vartab, ns), ty)
        }
        Type::String => {
            let buf = vartab.temp_name("empty_str", ty);
            cfg.add(
                vartab,
                Instr::Set {
                    loc: *loc,
                    res: buf,
                    expr: Expression::AllocDynamicBytes {
                        loc: *loc,
                        ty: ty.clone(),
                        size: Box::new(Expression::NumberLiteral {
                            loc: *loc,
                            ty: Type::Uint(32),
                            value: BigInt::zero(),
                        }),
                        initializer: Some(vec![]),
                    },
                },
            );
            let buf_var = Expression::Variable {
                loc: *loc,
                ty: ty.clone(),
                var_no: buf,
            };
            soroban_as_handle(soroban_encode_arg(buf_var, cfg, vartab, ns), ty)
        }
        Type::Struct(StructType::UserDefined(n)) => {
            soroban_struct_default_vec(loc, *n, ty, cfg, vartab, ns)
        }
        Type::Slice(_) => soroban_vec_new(loc, ty, cfg, vartab),
        Type::Array(_, dims) if dims.last() == Some(&ast::ArrayLength::Dynamic) => {
            soroban_vec_new(loc, ty, cfg, vartab)
        }
        Type::Array(..) if ty.array_length().is_some() => {
            soroban_fixed_array_default_vec(loc, ty, cfg, vartab, ns)
        }
        Type::Mapping(..) => map::soroban_map_new(loc, ty, cfg, vartab),
        _ => unreachable!("Type has no default storage value"),
    }
}

fn soroban_fixed_array_default_vec(
    loc: &pt::Loc,
    array_ty: &Type,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    ns: &Namespace,
) -> Expression {
    let handle_ty = soroban_vec_handle_ty(array_ty);
    let elem_ty = array_ty.array_elem();
    let count = array_ty
        .array_length()
        .expect("fixed array has a constant length")
        .clone();

    let vec_no = vartab.temp_name("array_default", &handle_ty);
    cfg.add(
        vartab,
        Instr::Call {
            res: vec![vec_no],
            return_tys: vec![handle_ty.clone()],
            call: InternalCallTy::HostFunction {
                name: HostFunctions::VectorNew.name().to_string(),
            },
            args: vec![],
        },
    );

    let mut current_vec_no = vec_no;
    let mut i = BigInt::zero();
    while i < count {
        let elem = soroban_default_handle(loc, &elem_ty, cfg, vartab, ns);
        let prev_vec = Expression::Variable {
            loc: *loc,
            ty: handle_ty.clone(),
            var_no: current_vec_no,
        };
        let next_vec_no = vartab.temp_name("array_default", &handle_ty);
        cfg.add(
            vartab,
            Instr::Call {
                res: vec![next_vec_no],
                return_tys: vec![handle_ty.clone()],
                call: InternalCallTy::HostFunction {
                    name: HostFunctions::VecPushBack.name().to_string(),
                },
                args: vec![prev_vec, elem],
            },
        );
        current_vec_no = next_vec_no;
        i += 1;
    }

    Expression::Variable {
        loc: *loc,
        ty: handle_ty,
        var_no: current_vec_no,
    }
}

pub(crate) fn load_raw_handle(
    loc: &pt::Loc,
    storage: Expression,
    storage_type: Option<pt::StorageType>,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
) -> Expression {
    let handle_no = vartab.temp_name("storage_handle", &Type::Uint(64));
    cfg.add(
        vartab,
        Instr::LoadStorage {
            res: handle_no,
            ty: Type::Uint(64),
            storage,
            storage_type,
        },
    );
    Expression::Variable {
        loc: *loc,
        ty: Type::Uint(64),
        var_no: handle_no,
    }
}
