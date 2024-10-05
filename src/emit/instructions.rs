// SPDX-License-Identifier: Apache-2.0

use crate::codegen::{
    cfg::{ControlFlowGraph, Instr, InternalCallTy, ReturnCode},
    revert::PanicCode,
    Expression,
};
use crate::emit::binary::Binary;
use crate::emit::cfg::{create_block, BasicBlock, Work};
use crate::emit::expression::expression;
use crate::emit::{ContractArgs, TargetRuntime, Variable};
use crate::sema::ast::{Contract, ExternalCallAccounts, Namespace, RetrieveType, Type};
use crate::Target;
use inkwell::types::BasicType;
use inkwell::values::{
    BasicMetadataValueEnum, BasicValue, BasicValueEnum, FunctionValue, IntValue, PointerValue,
};
use inkwell::{AddressSpace, IntPredicate};
use num_traits::ToPrimitive;
use solang_parser::pt::CodeLocation;
use std::collections::{HashMap, VecDeque};

use super::expression::expression_to_slice;

pub(super) fn process_instruction<'a, T: TargetRuntime<'a> + ?Sized>(
    target: &mut T,
    ins: &Instr,
    bin: &Binary<'a>,
    w: &mut Work<'a>,
    function: FunctionValue<'a>,
    ns: &Namespace,
    cfg: &ControlFlowGraph,
    work: &mut VecDeque<Work<'a>>,
    blocks: &mut HashMap<usize, BasicBlock<'a>>,
    contract: &Contract,
) {
    match ins {
        Instr::Nop => (),
        Instr::Return { value } if value.is_empty() && ns.target != Target::Soroban => {
            bin.builder
                .build_return(Some(&bin.return_values[&ReturnCode::Success]))
                .unwrap();
        }
        Instr::Return { value } if ns.target != Target::Soroban => {
            let returns_offset = cfg.params.len();
            for (i, val) in value.iter().enumerate() {
                let arg = function.get_nth_param((returns_offset + i) as u32).unwrap();
                let retval = expression(target, bin, val, &w.vars, function, ns);

                bin.builder
                    .build_store(arg.into_pointer_value(), retval)
                    .unwrap();
            }

            bin.builder
                .build_return(Some(&bin.return_values[&ReturnCode::Success]))
                .unwrap();
        }
        Instr::Return { value } => match value.iter().next() {
            Some(val) => {
                let retval = expression(target, bin, val, &w.vars, function, ns);
                bin.builder.build_return(Some(&retval)).unwrap();
            }
            None => {
                bin.builder.build_return(None).unwrap();
            }
        },
        Instr::Set { res, expr, .. } => {
            if let Expression::Undefined { ty: expr_type } = expr {
                // If the variable has been declared as undefined, but we can
                // initialize it with a default value
                if let Some(default_expr) = expr_type.default(ns) {
                    w.vars.get_mut(res).unwrap().value =
                        expression(target, bin, &default_expr, &w.vars, function, ns);
                }
            } else {
                w.vars.get_mut(res).unwrap().value =
                    expression(target, bin, expr, &w.vars, function, ns);
            }
        }
        Instr::Branch { block: dest } => {
            let pos = bin.builder.get_insert_block().unwrap();

            let bb = add_or_retrieve_block(*dest, pos, bin, function, blocks, work, w, cfg, ns);

            bin.builder.position_at_end(pos);
            bin.builder.build_unconditional_branch(bb).unwrap();
        }
        Instr::Store { dest, data } => {
            let value_ref = expression(target, bin, data, &w.vars, function, ns);
            let dest_ref =
                expression(target, bin, dest, &w.vars, function, ns).into_pointer_value();
            bin.builder.build_store(dest_ref, value_ref).unwrap();
        }
        Instr::BranchCond {
            cond,
            true_block: true_,
            false_block: false_,
        } => {
            let cond = expression(target, bin, cond, &w.vars, function, ns);

            let pos = bin.builder.get_insert_block().unwrap();

            let bb_true =
                add_or_retrieve_block(*true_, pos, bin, function, blocks, work, w, cfg, ns);

            let bb_false =
                add_or_retrieve_block(*false_, pos, bin, function, blocks, work, w, cfg, ns);

            bin.builder.position_at_end(pos);
            bin.builder
                .build_conditional_branch(cond.into_int_value(), bb_true, bb_false)
                .unwrap();
        }
        Instr::LoadStorage {
            res,
            ty,
            storage,
            storage_type,
        } => {
            let mut slot = expression(target, bin, storage, &w.vars, function, ns).into_int_value();

            w.vars.get_mut(res).unwrap().value =
                target.storage_load(bin, ty, &mut slot, function, ns, storage_type);
        }
        Instr::ClearStorage { ty, storage } => {
            let mut slot = expression(target, bin, storage, &w.vars, function, ns).into_int_value();

            target.storage_delete(bin, ty, &mut slot, function, ns);
        }
        Instr::SetStorage {
            ty,
            value,
            storage,
            storage_type,
        } => {
            let value = expression(target, bin, value, &w.vars, function, ns);

            let mut slot = expression(target, bin, storage, &w.vars, function, ns).into_int_value();

            target.storage_store(bin, ty, true, &mut slot, value, function, ns, storage_type);
        }
        Instr::SetStorageBytes {
            storage,
            value,
            offset,
        } => {
            let index_loc = offset.loc();
            let value = expression(target, bin, value, &w.vars, function, ns);

            let slot = expression(target, bin, storage, &w.vars, function, ns).into_int_value();
            let offset = expression(target, bin, offset, &w.vars, function, ns).into_int_value();

            target.set_storage_bytes_subscript(
                bin,
                function,
                slot,
                offset,
                value.into_int_value(),
                ns,
                index_loc,
            );
        }
        Instr::PushStorage {
            res,
            ty,
            storage,
            value,
        } => {
            let val = value
                .as_ref()
                .map(|expr| expression(target, bin, expr, &w.vars, function, ns));
            let slot = expression(target, bin, storage, &w.vars, function, ns).into_int_value();

            w.vars.get_mut(res).unwrap().value =
                target.storage_push(bin, function, ty, slot, val, ns);
        }
        Instr::PopStorage { res, ty, storage } => {
            let loc = storage.loc();
            let slot = expression(target, bin, storage, &w.vars, function, ns).into_int_value();

            let value = target.storage_pop(bin, function, ty, slot, res.is_some(), ns, loc);

            if let Some(res) = res {
                w.vars.get_mut(res).unwrap().value = value.unwrap();
            }
        }
        Instr::PushMemory {
            res,
            ty,
            array,
            value,
        } => {
            let arr = w.vars[array].value;

            let llvm_ty = bin.llvm_type(ty, ns);
            let elem_ty = ty.array_elem();

            // Calculate total size for reallocation
            let llvm_elem_ty = bin.llvm_field_ty(&elem_ty, ns);
            let elem_size = llvm_elem_ty
                .size_of()
                .unwrap()
                .const_cast(bin.context.i32_type(), false);
            let len = bin.vector_len(arr);
            let new_len = bin
                .builder
                .build_int_add(len, bin.context.i32_type().const_int(1, false), "")
                .unwrap();
            let vec_size = bin
                .module
                .get_struct_type("struct.vector")
                .unwrap()
                .size_of()
                .unwrap()
                .const_cast(bin.context.i32_type(), false);
            let size = bin.builder.build_int_mul(elem_size, new_len, "").unwrap();
            let size = bin.builder.build_int_add(size, vec_size, "").unwrap();

            // Reallocate and reassign the array pointer
            let new = bin
                .builder
                .build_call(
                    bin.module.get_function("__realloc").unwrap(),
                    &[arr.into(), size.into()],
                    "",
                )
                .unwrap()
                .try_as_basic_value()
                .left()
                .unwrap()
                .into_pointer_value();
            w.vars.get_mut(array).unwrap().value = new.into();

            // Store the value into the last element
            let slot_ptr = unsafe {
                bin.builder
                    .build_gep(
                        llvm_ty,
                        new,
                        &[
                            bin.context.i32_type().const_zero(),
                            bin.context.i32_type().const_int(2, false),
                            bin.builder.build_int_mul(len, elem_size, "").unwrap(),
                        ],
                        "data",
                    )
                    .unwrap()
            };
            let value = expression(target, bin, value, &w.vars, function, ns);
            let value = if elem_ty.is_fixed_reference_type(ns) {
                w.vars.get_mut(res).unwrap().value = slot_ptr.into();
                let load_ty = bin.llvm_type(&elem_ty, ns);
                bin.builder
                    .build_load(load_ty, value.into_pointer_value(), "elem")
                    .unwrap()
            } else {
                w.vars.get_mut(res).unwrap().value = value;
                value
            };
            bin.builder.build_store(slot_ptr, value).unwrap();

            // Update the len and size field of the vector struct
            let len_ptr = unsafe {
                bin.builder
                    .build_gep(
                        llvm_ty,
                        new,
                        &[
                            bin.context.i32_type().const_zero(),
                            bin.context.i32_type().const_zero(),
                        ],
                        "len",
                    )
                    .unwrap()
            };
            bin.builder.build_store(len_ptr, new_len).unwrap();

            let size_ptr = unsafe {
                bin.builder
                    .build_gep(
                        llvm_ty,
                        new,
                        &[
                            bin.context.i32_type().const_zero(),
                            bin.context.i32_type().const_int(1, false),
                        ],
                        "size",
                    )
                    .unwrap()
            };
            bin.builder.build_store(size_ptr, new_len).unwrap();
        }
        Instr::PopMemory {
            res,
            ty,
            array,
            loc,
        } => {
            let a = w.vars[array].value.into_pointer_value();
            let len = unsafe {
                bin.builder
                    .build_gep(
                        bin.module.get_struct_type("struct.vector").unwrap(),
                        a,
                        &[
                            bin.context.i32_type().const_zero(),
                            bin.context.i32_type().const_zero(),
                        ],
                        "a_len",
                    )
                    .unwrap()
            };
            let len = bin
                .builder
                .build_load(bin.context.i32_type(), len, "a_len")
                .unwrap()
                .into_int_value();

            // First check if the array is empty
            let is_array_empty = bin
                .builder
                .build_int_compare(
                    IntPredicate::EQ,
                    len,
                    bin.context.i32_type().const_zero(),
                    "is_array_empty",
                )
                .unwrap();
            let error = bin.context.append_basic_block(function, "error");
            let pop = bin.context.append_basic_block(function, "pop");
            bin.builder
                .build_conditional_branch(is_array_empty, error, pop)
                .unwrap();

            bin.builder.position_at_end(error);
            bin.log_runtime_error(target, "pop from empty array".to_string(), Some(*loc), ns);
            let (revert_out, revert_out_len) = bin.panic_data_const(ns, PanicCode::EmptyArrayPop);
            target.assert_failure(bin, revert_out, revert_out_len);

            bin.builder.position_at_end(pop);
            let llvm_ty = bin.llvm_type(ty, ns);

            let elem_ty = ty.array_elem();
            let llvm_elem_ty = bin.llvm_field_ty(&elem_ty, ns);

            // Calculate total size for reallocation
            let elem_size = llvm_elem_ty
                .size_of()
                .unwrap()
                .const_cast(bin.context.i32_type(), false);
            let new_len = bin
                .builder
                .build_int_sub(len, bin.context.i32_type().const_int(1, false), "")
                .unwrap();
            let vec_size = bin
                .module
                .get_struct_type("struct.vector")
                .unwrap()
                .size_of()
                .unwrap()
                .const_cast(bin.context.i32_type(), false);
            let size = bin.builder.build_int_mul(elem_size, new_len, "").unwrap();
            let size = bin.builder.build_int_add(size, vec_size, "").unwrap();

            // Get the pointer to the last element and return it
            let slot_ptr = unsafe {
                bin.builder
                    .build_gep(
                        bin.module.get_struct_type("struct.vector").unwrap(),
                        a,
                        &[
                            bin.context.i32_type().const_zero(),
                            bin.context.i32_type().const_int(2, false),
                            bin.builder.build_int_mul(new_len, elem_size, "").unwrap(),
                        ],
                        "data",
                    )
                    .unwrap()
            };
            if elem_ty.is_fixed_reference_type(ns) {
                w.vars.get_mut(res).unwrap().value = slot_ptr.into();
            } else {
                let ret_val = bin
                    .builder
                    .build_load(bin.llvm_type(&elem_ty, ns), slot_ptr, "")
                    .unwrap();
                w.vars.get_mut(res).unwrap().value = ret_val;
            }

            // Reallocate and reassign the array pointer
            let new = bin
                .builder
                .build_call(
                    bin.module.get_function("__realloc").unwrap(),
                    &[a.into(), size.into()],
                    "",
                )
                .unwrap()
                .try_as_basic_value()
                .left()
                .unwrap()
                .into_pointer_value();
            w.vars.get_mut(array).unwrap().value = new.into();

            // Update the len and size field of the vector struct
            let len_ptr = unsafe {
                bin.builder
                    .build_gep(
                        llvm_ty,
                        new,
                        &[
                            bin.context.i32_type().const_zero(),
                            bin.context.i32_type().const_zero(),
                        ],
                        "len",
                    )
                    .unwrap()
            };
            bin.builder.build_store(len_ptr, new_len).unwrap();

            let size_ptr = unsafe {
                bin.builder
                    .build_gep(
                        llvm_ty,
                        new,
                        &[
                            bin.context.i32_type().const_zero(),
                            bin.context.i32_type().const_int(1, false),
                        ],
                        "size",
                    )
                    .unwrap()
            };
            bin.builder.build_store(size_ptr, new_len).unwrap();
        }
        Instr::AssertFailure { encoded_args: None } => {
            target.assert_failure(
                bin,
                bin.context
                    .i8_type()
                    .ptr_type(AddressSpace::default())
                    .const_null(),
                bin.context.i32_type().const_zero(),
            );
        }
        Instr::AssertFailure {
            encoded_args: Some(expr),
        } => {
            let data = expression(target, bin, expr, &w.vars, function, ns);
            let vector_bytes = bin.vector_bytes(data);
            let len = bin.vector_len(data);

            target.assert_failure(bin, vector_bytes, len);
        }
        Instr::Print { expr } => {
            let expr = expression(target, bin, expr, &w.vars, function, ns);

            target.print(bin, bin.vector_bytes(expr), bin.vector_len(expr));
        }
        Instr::Call {
            res,
            call: InternalCallTy::Static { cfg_no },
            args,
            ..
        } => {
            let f = &contract.cfg[*cfg_no];

            let mut parms = args
                .iter()
                .map(|p| expression(target, bin, p, &w.vars, function, ns).into())
                .collect::<Vec<BasicMetadataValueEnum>>();

            // Soroban doesn't write return values to imported memory
            if !res.is_empty() && ns.target != Target::Soroban {
                for v in f.returns.iter() {
                    parms.push(if ns.target == Target::Solana {
                        bin.build_alloca(function, bin.llvm_var_ty(&v.ty, ns), v.name_as_str())
                            .into()
                    } else {
                        bin.builder
                            .build_alloca(bin.llvm_var_ty(&v.ty, ns), v.name_as_str())
                            .unwrap()
                            .into()
                    });
                }
            }

            if let Some(parameters) = bin.parameters {
                parms.push(parameters.into());
            }

            let ret = bin
                .builder
                .build_call(bin.functions[cfg_no], &parms, "")
                .unwrap()
                .try_as_basic_value()
                .left();

            // Soroban doesn't have return codes, and only returns a single i64 value
            if ns.target != Target::Soroban {
                let success = bin
                    .builder
                    .build_int_compare(
                        IntPredicate::EQ,
                        ret.unwrap().into_int_value(),
                        bin.return_values[&ReturnCode::Success],
                        "success",
                    )
                    .unwrap();

                let success_block = bin.context.append_basic_block(function, "success");
                let bail_block = bin.context.append_basic_block(function, "bail");
                bin.builder
                    .build_conditional_branch(success, success_block, bail_block)
                    .unwrap();

                bin.builder.position_at_end(bail_block);

                bin.builder.build_return(Some(&ret.unwrap())).unwrap();
                bin.builder.position_at_end(success_block);

                if !res.is_empty() {
                    for (i, v) in f.returns.iter().enumerate() {
                        let load_ty = bin.llvm_var_ty(&v.ty, ns);
                        let val = bin
                            .builder
                            .build_load(
                                load_ty,
                                parms[args.len() + i].into_pointer_value(),
                                v.name_as_str(),
                            )
                            .unwrap();
                        let dest = w.vars[&res[i]].value;

                        if dest.is_pointer_value()
                            && !(v.ty.is_reference_type(ns)
                                || matches!(v.ty, Type::ExternalFunction { .. }))
                        {
                            bin.builder
                                .build_store(dest.into_pointer_value(), val)
                                .unwrap();
                        } else {
                            w.vars.get_mut(&res[i]).unwrap().value = val;
                        }
                    }
                }
            } else if let Some(value) = ret {
                w.vars.get_mut(&res[0]).unwrap().value = value;
            }
        }
        Instr::Call {
            res,
            call: InternalCallTy::Builtin { ast_func_no },
            args,
            ..
        } => {
            let mut parms = args
                .iter()
                .map(|p| expression(target, bin, p, &w.vars, function, ns).into())
                .collect::<Vec<BasicMetadataValueEnum>>();

            let callee = &ns.functions[*ast_func_no];

            if !res.is_empty() {
                for v in callee.returns.iter() {
                    parms.push(if ns.target == Target::Solana {
                        bin.build_alloca(function, bin.llvm_var_ty(&v.ty, ns), v.name_as_str())
                            .into()
                    } else {
                        bin.builder
                            .build_alloca(bin.llvm_var_ty(&v.ty, ns), v.name_as_str())
                            .unwrap()
                            .into()
                    });
                }
            }

            if let Some(ret) = target.builtin_function(
                bin,
                function,
                callee,
                &parms,
                args.first().map(|arg| bin.llvm_type(&arg.ty(), ns)),
                ns,
            ) {
                let success = bin
                    .builder
                    .build_int_compare(
                        IntPredicate::EQ,
                        ret.into_int_value(),
                        bin.return_values[&ReturnCode::Success],
                        "success",
                    )
                    .unwrap();
                let success_block = bin.context.append_basic_block(function, "success");
                let bail_block = bin.context.append_basic_block(function, "bail");
                bin.builder
                    .build_conditional_branch(success, success_block, bail_block)
                    .unwrap();

                bin.builder.position_at_end(bail_block);
                bin.builder.build_return(Some(&ret)).unwrap();

                bin.builder.position_at_end(success_block);
            }

            if !res.is_empty() {
                for (i, v) in callee.returns.iter().enumerate() {
                    let load_ty = if v.ty.is_reference_type(ns) {
                        bin.llvm_type(&v.ty, ns)
                            .ptr_type(AddressSpace::default())
                            .as_basic_type_enum()
                    } else {
                        bin.llvm_type(&v.ty, ns)
                    };
                    let val = bin
                        .builder
                        .build_load(
                            load_ty,
                            parms[args.len() + i].into_pointer_value(),
                            v.name_as_str(),
                        )
                        .unwrap();

                    let dest = w.vars[&res[i]].value;

                    if dest.is_pointer_value()
                        && !(v.ty.is_reference_type(ns)
                            || matches!(v.ty, Type::ExternalFunction { .. }))
                    {
                        bin.builder
                            .build_store(dest.into_pointer_value(), val)
                            .unwrap();
                    } else {
                        w.vars.get_mut(&res[i]).unwrap().value = val;
                    }
                }
            }
        }
        Instr::Call {
            res,
            call: InternalCallTy::Dynamic(call_expr),
            args,
            ..
        } => {
            let ty = call_expr.ty();

            let (llvm_func, returns) = if let Type::InternalFunction {
                params, returns, ..
            } = ty.deref_any()
            {
                (bin.function_type(params, returns, ns), returns)
            } else {
                panic!("should be Type::InternalFunction type");
            };

            let mut parms = args
                .iter()
                .map(|p| expression(target, bin, p, &w.vars, function, ns).into())
                .collect::<Vec<BasicMetadataValueEnum>>();

            if !res.is_empty() {
                for ty in returns.iter() {
                    parms.push(
                        bin.build_alloca(function, bin.llvm_var_ty(ty, ns), "")
                            .into(),
                    );
                }
            }

            // on Solana, we need to pass the accounts parameter around
            if let Some(parameters) = bin.parameters {
                parms.push(parameters.into());
            }

            let callable =
                expression(target, bin, call_expr, &w.vars, function, ns).into_pointer_value();

            let ptr_ok = bin.context.append_basic_block(function, "fn_ptr_ok");
            let ptr_nil_block = bin.context.append_basic_block(function, "fn_ptr_nil");
            let nil_ptr = bin
                .context
                .i8_type()
                .ptr_type(AddressSpace::default())
                .const_null();
            let is_ptr_nil = bin
                .builder
                .build_int_compare(IntPredicate::EQ, nil_ptr, callable, "check_nil_ptr")
                .unwrap();
            bin.builder
                .build_conditional_branch(is_ptr_nil, ptr_nil_block, ptr_ok)
                .unwrap();

            bin.builder.position_at_end(ptr_nil_block);
            bin.log_runtime_error(
                target,
                "internal function uninitialized".to_string(),
                None,
                ns,
            );
            let (revert_out, revert_out_len) =
                bin.panic_data_const(ns, PanicCode::InternalFunctionUninitialized);
            target.assert_failure(bin, revert_out, revert_out_len);

            bin.builder.position_at_end(ptr_ok);
            let ret = bin
                .builder
                .build_indirect_call(llvm_func, callable, &parms, "")
                .unwrap()
                .try_as_basic_value()
                .left()
                .unwrap();

            let success = bin
                .builder
                .build_int_compare(
                    IntPredicate::EQ,
                    ret.into_int_value(),
                    bin.return_values[&ReturnCode::Success],
                    "success",
                )
                .unwrap();

            let success_block = bin.context.append_basic_block(function, "success");
            let bail_block = bin.context.append_basic_block(function, "bail");
            bin.builder
                .build_conditional_branch(success, success_block, bail_block)
                .unwrap();

            bin.builder.position_at_end(bail_block);

            bin.builder.build_return(Some(&ret)).unwrap();
            bin.builder.position_at_end(success_block);

            if !res.is_empty() {
                for (i, ty) in returns.iter().enumerate() {
                    let load_ty = bin.llvm_var_ty(ty, ns);
                    let val = bin
                        .builder
                        .build_load(load_ty, parms[args.len() + i].into_pointer_value(), "")
                        .unwrap();

                    let dest = w.vars[&res[i]].value;

                    if dest.is_pointer_value() && !ty.is_reference_type(ns) {
                        bin.builder
                            .build_store(dest.into_pointer_value(), val)
                            .unwrap();
                    } else {
                        w.vars.get_mut(&res[i]).unwrap().value = val;
                    }
                }
            }
        }
        Instr::Constructor {
            success,
            res,
            contract_no,
            encoded_args,
            value,
            gas,
            salt,
            address,
            seeds,
            loc,
            accounts,
            constructor_no: _,
        } => {
            let encoded_args = expression(target, bin, encoded_args, &w.vars, function, ns);
            let encoded_args_len = bin.vector_len(encoded_args).as_basic_value_enum();

            let address_stack = bin.build_alloca(function, bin.address_type(ns), "address");

            let gas = expression(target, bin, gas, &w.vars, function, ns).into_int_value();
            let value = value
                .as_ref()
                .map(|v| expression(target, bin, v, &w.vars, function, ns).into_int_value());
            let salt = salt
                .as_ref()
                .map(|v| expression(target, bin, v, &w.vars, function, ns).into_int_value());

            let llvm_accounts = process_account_metas(target, accounts, bin, &w.vars, function, ns);

            if let Some(address) = address {
                let address =
                    expression(target, bin, address, &w.vars, function, ns).into_array_value();

                bin.builder.build_store(address_stack, address).unwrap();
            }

            let seeds = if let Some(seeds) = seeds {
                let len = seeds.ty().array_length().unwrap().to_u64().unwrap();
                let seeds_ty = bin.llvm_type(
                    &Type::Slice(Box::new(Type::Slice(Box::new(Type::Bytes(1))))),
                    ns,
                );

                let output_seeds = bin.build_array_alloca(
                    function,
                    seeds_ty,
                    bin.context.i64_type().const_int(len, false),
                    "seeds",
                );

                if let Expression::ArrayLiteral { values, .. } = seeds {
                    for i in 0..len {
                        let val =
                            expression(target, bin, &values[i as usize], &w.vars, function, ns);

                        let seed_count = values[i as usize]
                            .ty()
                            .deref_memory()
                            .array_length()
                            .unwrap()
                            .to_u64()
                            .unwrap();

                        let dest = unsafe {
                            bin.builder
                                .build_gep(
                                    seeds_ty,
                                    output_seeds,
                                    &[
                                        bin.context.i32_type().const_int(i, false),
                                        bin.context.i32_type().const_zero(),
                                    ],
                                    "dest",
                                )
                                .unwrap()
                        };

                        bin.builder.build_store(dest, val).unwrap();

                        let dest = unsafe {
                            bin.builder
                                .build_gep(
                                    seeds_ty,
                                    output_seeds,
                                    &[
                                        bin.context.i32_type().const_int(i, false),
                                        bin.context.i32_type().const_int(1, false),
                                    ],
                                    "dest",
                                )
                                .unwrap()
                        };

                        let val = bin.context.i64_type().const_int(seed_count, false);

                        bin.builder.build_store(dest, val).unwrap();
                    }
                }

                Some((output_seeds, bin.context.i64_type().const_int(len, false)))
            } else {
                None
            };

            target.create_contract(
                bin,
                function,
                success.map(|n| &mut w.vars.get_mut(&n).unwrap().value),
                *contract_no,
                address_stack,
                encoded_args,
                encoded_args_len,
                ContractArgs {
                    program_id: None,
                    accounts: llvm_accounts,
                    gas: Some(gas),
                    value,
                    salt,
                    seeds,
                    flags: None,
                },
                ns,
                *loc,
            );

            w.vars.get_mut(res).unwrap().value = bin
                .builder
                .build_load(bin.address_type(ns), address_stack, "address")
                .unwrap();
        }
        Instr::ExternalCall {
            success,
            address,
            payload,
            value,
            gas,
            callty,
            accounts,
            seeds,
            flags,
            ..
        } => {
            let loc = payload.loc();
            let gas = expression(target, bin, gas, &w.vars, function, ns).into_int_value();
            let value = expression(target, bin, value, &w.vars, function, ns).into_int_value();
            let payload_ty = payload.ty();
            let payload = expression(target, bin, payload, &w.vars, function, ns);

            let address = if let Some(address) = address {
                let address = expression(target, bin, address, &w.vars, function, ns);

                let addr = bin.build_array_alloca(
                    function,
                    bin.context.i8_type(),
                    bin.context
                        .i32_type()
                        .const_int(ns.address_length as u64, false),
                    "address",
                );

                bin.builder.build_store(addr, address).unwrap();

                Some(addr)
            } else {
                None
            };

            let accounts = process_account_metas(target, accounts, bin, &w.vars, function, ns);

            let (payload_ptr, payload_len) = if payload_ty == Type::DynamicBytes {
                (bin.vector_bytes(payload), bin.vector_len(payload))
            } else {
                let ptr = payload.into_pointer_value();
                let len = bin.llvm_type(&payload_ty, ns).size_of().unwrap();

                (ptr, len)
            };

            // sol_invoke_signed_c() takes of a slice of a slice of slice of bytes
            // 1. A single seed value is a slice of bytes.
            // 2. A signer for single address can have multiple seeds
            // 3. A single call to sol_invoke_signed_c can sign for multiple addresses
            let seeds_ty =
                Type::Slice(Type::Slice(Type::Slice(Type::Bytes(1).into()).into()).into());

            let seeds = seeds.as_ref().map(|seeds| {
                expression_to_slice(target, bin, seeds, &seeds_ty, &w.vars, function, ns)
            });

            let flags = flags
                .as_ref()
                .map(|e| expression(target, bin, e, &w.vars, function, ns).into_int_value());
            let success = match success {
                Some(n) => Some(&mut w.vars.get_mut(n).unwrap().value),
                None => None,
            };

            target.external_call(
                bin,
                function,
                success,
                payload_ptr,
                payload_len,
                address,
                ContractArgs {
                    program_id: None,
                    value: Some(value),
                    gas: Some(gas),
                    salt: None,
                    seeds,
                    accounts,
                    flags,
                },
                callty.clone(),
                ns,
                loc,
            );
        }
        Instr::ValueTransfer {
            success,
            address,
            value,
        } => {
            let loc = value.loc();
            let value = expression(target, bin, value, &w.vars, function, ns).into_int_value();
            let address =
                expression(target, bin, address, &w.vars, function, ns).into_array_value();

            let addr = bin.build_alloca(function, bin.address_type(ns), "address");

            bin.builder.build_store(addr, address).unwrap();
            let success = match success {
                Some(n) => Some(&mut w.vars.get_mut(n).unwrap().value),
                None => None,
            };

            target.value_transfer(bin, function, success, addr, value, ns, loc);
        }
        Instr::SelfDestruct { recipient } => {
            let recipient =
                expression(target, bin, recipient, &w.vars, function, ns).into_array_value();

            target.selfdestruct(bin, recipient, ns);
        }
        Instr::EmitEvent { data, topics, .. } => {
            let data = expression(target, bin, data, &w.vars, function, ns);
            let topics = topics
                .iter()
                .map(|a| expression(target, bin, a, &w.vars, function, ns))
                .collect::<Vec<BasicValueEnum>>();
            target.emit_event(bin, function, data, &topics);
        }
        Instr::WriteBuffer { buf, offset, value } => {
            let v = expression(target, bin, buf, &w.vars, function, ns);
            let data = bin.vector_bytes(v);

            let offset = expression(target, bin, offset, &w.vars, function, ns).into_int_value();
            let emit_value = expression(target, bin, value, &w.vars, function, ns);

            let start = unsafe {
                bin.builder
                    .build_gep(bin.context.i8_type(), data, &[offset], "start")
                    .unwrap()
            };

            let is_bytes = if let Type::Bytes(n) = value.ty().unwrap_user_type(ns) {
                n
            } else if value.ty() == Type::FunctionSelector {
                ns.target.selector_length()
            } else {
                0
            };

            if is_bytes > 1 {
                let value_ptr = bin.build_alloca(
                    function,
                    emit_value.into_int_value().get_type(),
                    &format!("bytes{is_bytes}"),
                );
                bin.builder
                    .build_store(value_ptr, emit_value.into_int_value())
                    .unwrap();
                bin.builder
                    .build_call(
                        bin.module.get_function("__leNtobeN").unwrap(),
                        &[
                            value_ptr.into(),
                            start.into(),
                            bin.context
                                .i32_type()
                                .const_int(is_bytes as u64, false)
                                .into(),
                        ],
                        "",
                    )
                    .unwrap();
            } else {
                bin.builder.build_store(start, emit_value).unwrap();
            }
        }
        Instr::MemCopy {
            source: from,
            destination: to,
            bytes,
        } => {
            let src = if from.ty().is_dynamic_memory() {
                bin.vector_bytes(expression(target, bin, from, &w.vars, function, ns))
            } else {
                expression(target, bin, from, &w.vars, function, ns).into_pointer_value()
            };

            let dest = if to.ty().is_dynamic_memory() {
                bin.vector_bytes(expression(target, bin, to, &w.vars, function, ns))
            } else {
                expression(target, bin, to, &w.vars, function, ns).into_pointer_value()
            };

            let size = expression(target, bin, bytes, &w.vars, function, ns);

            bin.builder
                .build_call(
                    bin.module.get_function("__memcpy").unwrap(),
                    &[dest.into(), src.into(), size.into()],
                    "",
                )
                .unwrap();
        }
        Instr::Switch {
            cond,
            cases,
            default,
        } => {
            let pos = bin.builder.get_insert_block().unwrap();
            let cond = expression(target, bin, cond, &w.vars, function, ns);
            let cases = cases
                .iter()
                .map(|(exp, block_no)| {
                    let exp = expression(target, bin, exp, &w.vars, function, ns);
                    let bb = add_or_retrieve_block(
                        *block_no, pos, bin, function, blocks, work, w, cfg, ns,
                    );
                    (exp.into_int_value(), bb)
                })
                .collect::<Vec<(IntValue, inkwell::basic_block::BasicBlock)>>();

            let default_bb =
                add_or_retrieve_block(*default, pos, bin, function, blocks, work, w, cfg, ns);
            bin.builder.position_at_end(pos);
            bin.builder
                .build_switch(cond.into_int_value(), default_bb, cases.as_ref())
                .unwrap();
        }

        Instr::ReturnData { data, data_len } => {
            let data = if data.ty().is_reference_type(ns) {
                bin.vector_bytes(expression(target, bin, data, &w.vars, function, ns))
            } else {
                expression(target, bin, data, &w.vars, function, ns).into_pointer_value()
            };

            let data_len = expression(target, bin, data_len, &w.vars, function, ns);
            target.return_abi_data(bin, data, data_len);
        }

        Instr::ReturnCode { code } => {
            target.return_code(bin, bin.return_values[code]);
        }

        Instr::Unimplemented { .. } => unimplemented!(),
        Instr::AccountAccess { .. } => {
            unreachable!("Instr::AccountAccess shall never appear in the CFG")
        }
    }
}

/// Add or retrieve a basic block from the blocks' hashmap
fn add_or_retrieve_block<'a>(
    block_no: usize,
    pos: inkwell::basic_block::BasicBlock<'a>,
    bin: &Binary<'a>,
    function: FunctionValue<'a>,
    blocks: &mut HashMap<usize, BasicBlock<'a>>,
    work: &mut VecDeque<Work<'a>>,
    w: &mut Work<'a>,
    cfg: &ControlFlowGraph,
    ns: &Namespace,
) -> inkwell::basic_block::BasicBlock<'a> {
    if let std::collections::hash_map::Entry::Vacant(e) = blocks.entry(block_no) {
        e.insert(create_block(block_no, bin, cfg, function, ns));
        work.push_back(Work {
            block_no,
            vars: w.vars.clone(),
        });
    }

    let bb = blocks.get(&block_no).unwrap();

    for (v, phi) in bb.phis.iter() {
        phi.add_incoming(&[(&w.vars[v].value, pos)]);
    }

    bb.bb
}

/// If the AccountMeta array has been provided in an external call or contract creation,
/// we process its codegen representation here and return the pointer to it and its size.
fn process_account_metas<'a, T: TargetRuntime<'a> + ?Sized>(
    target: &T,
    accounts: &ExternalCallAccounts<Expression>,
    bin: &Binary<'a>,
    vartab: &HashMap<usize, Variable<'a>>,
    function: FunctionValue<'a>,
    ns: &Namespace,
) -> Option<(PointerValue<'a>, IntValue<'a>)> {
    if let ExternalCallAccounts::Present(accounts) = accounts {
        let ty = accounts.ty();
        let expr = expression(target, bin, accounts, vartab, function, ns);

        if let Some(n) = ty.array_length() {
            let accounts = expr.into_pointer_value();
            let len = bin.context.i32_type().const_int(n.to_u64().unwrap(), false);

            Some((accounts, len))
        } else {
            unreachable!("dynamic array not allowed for the 'accounts' parameter");
        }
    } else {
        None
    }
}
