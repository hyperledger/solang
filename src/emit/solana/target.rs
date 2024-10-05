// SPDX-License-Identifier: Apache-2.0

use crate::codegen;
use crate::codegen::cfg::{HashTy, ReturnCode};
use crate::emit::binary::Binary;
use crate::emit::expression::expression;
use crate::emit::loop_builder::LoopBuilder;
use crate::emit::solana::SolanaTarget;
use crate::emit::{ContractArgs, TargetRuntime, Variable};
use crate::sema::ast::{self, Namespace};
use inkwell::types::{BasicType, BasicTypeEnum, IntType};
use inkwell::values::{
    ArrayValue, BasicMetadataValueEnum, BasicValueEnum, FunctionValue, IntValue, PointerValue,
};
use inkwell::{AddressSpace, IntPredicate};
use num_traits::ToPrimitive;
use solang_parser::pt::{Loc, StorageType};
use std::collections::HashMap;

impl<'a> TargetRuntime<'a> for SolanaTarget {
    /// Solana does not use slot based-storage so override
    fn storage_delete(
        &self,
        binary: &Binary<'a>,
        ty: &ast::Type,
        slot: &mut IntValue<'a>,
        function: FunctionValue<'a>,
        ns: &ast::Namespace,
    ) {
        // binary storage is in 2nd account
        let data = self.contract_storage_data(binary);

        self.storage_free(binary, ty, data, *slot, function, true, ns);
    }

    fn set_storage_extfunc(
        &self,
        _binary: &Binary,
        _function: FunctionValue,
        _slot: PointerValue,
        _dest: PointerValue,
        _dest_ty: BasicTypeEnum,
    ) {
        unimplemented!();
    }
    fn get_storage_extfunc(
        &self,
        _binary: &Binary<'a>,
        _function: FunctionValue,
        _slot: PointerValue<'a>,
        _ns: &ast::Namespace,
    ) -> PointerValue<'a> {
        unimplemented!();
    }

    fn set_storage_string(
        &self,
        _binary: &Binary<'a>,
        _function: FunctionValue<'a>,
        _slot: PointerValue<'a>,
        _dest: BasicValueEnum<'a>,
    ) {
        // unused
        unreachable!();
    }

    fn get_storage_string(
        &self,
        _binary: &Binary<'a>,
        _function: FunctionValue,
        _slot: PointerValue<'a>,
    ) -> PointerValue<'a> {
        // unused
        unreachable!();
    }

    fn get_storage_bytes_subscript(
        &self,
        binary: &Binary<'a>,
        function: FunctionValue,
        slot: IntValue<'a>,
        index: IntValue<'a>,
        loc: Loc,
        ns: &Namespace,
    ) -> IntValue<'a> {
        let data = self.contract_storage_data(binary);

        let member = unsafe {
            binary
                .builder
                .build_gep(binary.context.i8_type(), data, &[slot], "data")
                .unwrap()
        };

        let offset = binary
            .builder
            .build_load(binary.context.i32_type(), member, "offset")
            .unwrap()
            .into_int_value();

        let length = binary
            .builder
            .build_call(
                binary.module.get_function("account_data_len").unwrap(),
                &[data.into(), offset.into()],
                "length",
            )
            .unwrap()
            .try_as_basic_value()
            .left()
            .unwrap()
            .into_int_value();

        // do bounds check on index
        let in_range = binary
            .builder
            .build_int_compare(IntPredicate::ULT, index, length, "index_in_range")
            .unwrap();

        let get_block = binary.context.append_basic_block(function, "in_range");
        let bang_block = binary.context.append_basic_block(function, "bang_block");

        binary
            .builder
            .build_conditional_branch(in_range, get_block, bang_block)
            .unwrap();

        binary.builder.position_at_end(bang_block);

        binary.log_runtime_error(
            self,
            "storage array index out of bounds".to_string(),
            Some(loc),
            ns,
        );
        self.assert_failure(
            binary,
            binary
                .context
                .i8_type()
                .ptr_type(AddressSpace::default())
                .const_null(),
            binary.context.i32_type().const_zero(),
        );

        binary.builder.position_at_end(get_block);

        let offset = binary
            .builder
            .build_int_add(offset, index, "offset")
            .unwrap();

        let member = unsafe {
            binary
                .builder
                .build_gep(binary.context.i8_type(), data, &[offset], "data")
                .unwrap()
        };

        binary
            .builder
            .build_load(binary.context.i8_type(), member, "val")
            .unwrap()
            .into_int_value()
    }

    fn set_storage_bytes_subscript(
        &self,
        binary: &Binary,
        function: FunctionValue,
        slot: IntValue,
        index: IntValue,
        val: IntValue,
        ns: &Namespace,
        loc: Loc,
    ) {
        let data = self.contract_storage_data(binary);

        let member = unsafe {
            binary
                .builder
                .build_gep(binary.context.i8_type(), data, &[slot], "data")
                .unwrap()
        };

        let offset = binary
            .builder
            .build_load(binary.context.i32_type(), member, "offset")
            .unwrap()
            .into_int_value();

        let length = binary
            .builder
            .build_call(
                binary.module.get_function("account_data_len").unwrap(),
                &[data.into(), offset.into()],
                "length",
            )
            .unwrap()
            .try_as_basic_value()
            .left()
            .unwrap()
            .into_int_value();

        // do bounds check on index
        let in_range = binary
            .builder
            .build_int_compare(IntPredicate::ULT, index, length, "index_in_range")
            .unwrap();

        let set_block = binary.context.append_basic_block(function, "in_range");
        let bang_block = binary.context.append_basic_block(function, "bang_block");

        binary
            .builder
            .build_conditional_branch(in_range, set_block, bang_block)
            .unwrap();

        binary.builder.position_at_end(bang_block);
        binary.log_runtime_error(
            self,
            "storage index out of bounds".to_string(),
            Some(loc),
            ns,
        );
        self.assert_failure(
            binary,
            binary
                .context
                .i8_type()
                .ptr_type(AddressSpace::default())
                .const_null(),
            binary.context.i32_type().const_zero(),
        );

        binary.builder.position_at_end(set_block);

        let offset = binary
            .builder
            .build_int_add(offset, index, "offset")
            .unwrap();

        let member = unsafe {
            binary
                .builder
                .build_gep(binary.context.i8_type(), data, &[offset], "data")
                .unwrap()
        };

        binary.builder.build_store(member, val).unwrap();
    }

    fn storage_subscript(
        &self,
        binary: &Binary<'a>,
        function: FunctionValue<'a>,
        ty: &ast::Type,
        slot: IntValue<'a>,
        index: BasicValueEnum<'a>,
        ns: &ast::Namespace,
    ) -> IntValue<'a> {
        let account = self.contract_storage_account(binary);

        if let ast::Type::Mapping(ast::Mapping { key, value, .. }) = ty.deref_any() {
            self.sparse_lookup(binary, function, key, value, slot, index, ns)
        } else if ty.is_sparse_solana(ns) {
            // sparse array
            let elem_ty = ty.storage_array_elem().deref_into();

            let key = ast::Type::Uint(256);

            self.sparse_lookup(binary, function, &key, &elem_ty, slot, index, ns)
        } else {
            // 3rd member of account is data pointer
            let data = unsafe {
                binary
                    .builder
                    .build_gep(
                        binary
                            .module
                            .get_struct_type("struct.SolAccountInfo")
                            .unwrap(),
                        account,
                        &[
                            binary.context.i32_type().const_zero(),
                            binary.context.i32_type().const_int(3, false),
                        ],
                        "data",
                    )
                    .unwrap()
            };

            let data = binary
                .builder
                .build_load(
                    binary.context.i8_type().ptr_type(AddressSpace::default()),
                    data,
                    "data",
                )
                .unwrap()
                .into_pointer_value();

            let member = unsafe {
                binary
                    .builder
                    .build_gep(binary.context.i8_type(), data, &[slot], "data")
                    .unwrap()
            };

            let offset = binary
                .builder
                .build_load(binary.context.i32_type(), member, "offset")
                .unwrap()
                .into_int_value();

            let elem_ty = ty.storage_array_elem().deref_into();

            let elem_size = binary
                .context
                .i32_type()
                .const_int(elem_ty.solana_storage_size(ns).to_u64().unwrap(), false);

            binary
                .builder
                .build_int_add(
                    offset,
                    binary
                        .builder
                        .build_int_mul(index.into_int_value(), elem_size, "")
                        .unwrap(),
                    "",
                )
                .unwrap()
        }
    }

    fn storage_push(
        &self,
        binary: &Binary<'a>,
        function: FunctionValue<'a>,
        ty: &ast::Type,
        slot: IntValue<'a>,
        val: Option<BasicValueEnum<'a>>,
        ns: &ast::Namespace,
    ) -> BasicValueEnum<'a> {
        let data = self.contract_storage_data(binary);
        let account = self.contract_storage_account(binary);

        let member = unsafe {
            binary
                .builder
                .build_gep(binary.context.i8_type(), data, &[slot], "data")
                .unwrap()
        };

        let offset = binary
            .builder
            .build_load(binary.context.i32_type(), member, "offset")
            .unwrap()
            .into_int_value();

        let length = binary
            .builder
            .build_call(
                binary.module.get_function("account_data_len").unwrap(),
                &[data.into(), offset.into()],
                "length",
            )
            .unwrap()
            .try_as_basic_value()
            .left()
            .unwrap()
            .into_int_value();

        let member_size = binary
            .context
            .i32_type()
            .const_int(ty.storage_slots(ns).to_u64().unwrap(), false);
        let new_length = binary
            .builder
            .build_int_add(length, member_size, "new_length")
            .unwrap();

        let rc = binary
            .builder
            .build_call(
                binary.module.get_function("account_data_realloc").unwrap(),
                &[
                    account.into(),
                    offset.into(),
                    new_length.into(),
                    member.into(),
                ],
                "new_offset",
            )
            .unwrap()
            .try_as_basic_value()
            .left()
            .unwrap()
            .into_int_value();

        let is_rc_zero = binary
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                rc,
                binary.context.i64_type().const_zero(),
                "is_rc_zero",
            )
            .unwrap();

        let rc_not_zero = binary.context.append_basic_block(function, "rc_not_zero");
        let rc_zero = binary.context.append_basic_block(function, "rc_zero");

        binary
            .builder
            .build_conditional_branch(is_rc_zero, rc_zero, rc_not_zero)
            .unwrap();

        binary.builder.position_at_end(rc_not_zero);

        self.return_code(
            binary,
            binary.context.i64_type().const_int(5u64 << 32, false),
        );

        binary.builder.position_at_end(rc_zero);

        let mut new_offset = binary
            .builder
            .build_int_add(
                binary
                    .builder
                    .build_load(binary.context.i32_type(), member, "offset")
                    .unwrap()
                    .into_int_value(),
                length,
                "",
            )
            .unwrap();

        if let Some(val) = val {
            self.storage_store(binary, ty, false, &mut new_offset, val, function, ns, &None);
        }

        if ty.is_reference_type(ns) {
            // Caller expects a reference to storage; note that storage_store() should not modify
            // new_offset even if the argument is mut
            new_offset.into()
        } else {
            val.unwrap()
        }
    }

    fn storage_pop(
        &self,
        binary: &Binary<'a>,
        function: FunctionValue<'a>,
        ty: &ast::Type,
        slot: IntValue<'a>,
        load: bool,
        ns: &ast::Namespace,
        loc: Loc,
    ) -> Option<BasicValueEnum<'a>> {
        let data = self.contract_storage_data(binary);
        let account = self.contract_storage_account(binary);

        let member = unsafe {
            binary
                .builder
                .build_gep(binary.context.i8_type(), data, &[slot], "data")
                .unwrap()
        };

        let offset = binary
            .builder
            .build_load(binary.context.i32_type(), member, "offset")
            .unwrap()
            .into_int_value();

        let length = binary
            .builder
            .build_call(
                binary.module.get_function("account_data_len").unwrap(),
                &[data.into(), offset.into()],
                "length",
            )
            .unwrap()
            .try_as_basic_value()
            .left()
            .unwrap()
            .into_int_value();

        // do bounds check on index
        let in_range = binary
            .builder
            .build_int_compare(
                IntPredicate::NE,
                binary.context.i32_type().const_zero(),
                length,
                "index_in_range",
            )
            .unwrap();

        let bang_block = binary.context.append_basic_block(function, "bang_block");
        let retrieve_block = binary.context.append_basic_block(function, "in_range");

        binary
            .builder
            .build_conditional_branch(in_range, retrieve_block, bang_block)
            .unwrap();

        binary.builder.position_at_end(bang_block);
        binary.log_runtime_error(
            self,
            "pop from empty storage array".to_string(),
            Some(loc),
            ns,
        );
        self.assert_failure(
            binary,
            binary
                .context
                .i8_type()
                .ptr_type(AddressSpace::default())
                .const_null(),
            binary.context.i32_type().const_zero(),
        );

        binary.builder.position_at_end(retrieve_block);

        let member_size = binary
            .context
            .i32_type()
            .const_int(ty.storage_slots(ns).to_u64().unwrap(), false);

        let new_length = binary
            .builder
            .build_int_sub(length, member_size, "new_length")
            .unwrap();

        let mut old_elem_offset = binary
            .builder
            .build_int_add(offset, new_length, "")
            .unwrap();

        let val = if load {
            Some(self.storage_load(binary, ty, &mut old_elem_offset, function, ns, &None))
        } else {
            None
        };

        // delete existing storage -- pointers need to be freed
        self.storage_free(binary, ty, data, old_elem_offset, function, false, ns);

        // we can assume pointer will stay the same after realloc to smaller size
        binary
            .builder
            .build_call(
                binary.module.get_function("account_data_realloc").unwrap(),
                &[
                    account.into(),
                    offset.into(),
                    new_length.into(),
                    member.into(),
                ],
                "new_offset",
            )
            .unwrap();

        val
    }

    fn storage_array_length(
        &self,
        binary: &Binary<'a>,
        _function: FunctionValue,
        slot: IntValue<'a>,
        elem_ty: &ast::Type,
        ns: &ast::Namespace,
    ) -> IntValue<'a> {
        let data = self.contract_storage_data(binary);

        // the slot is simply the offset after the magic
        let member = unsafe {
            binary
                .builder
                .build_gep(binary.context.i8_type(), data, &[slot], "data")
                .unwrap()
        };

        let offset = binary
            .builder
            .build_load(binary.context.i32_type(), member, "offset")
            .unwrap()
            .into_int_value();

        let member_size = binary
            .context
            .i32_type()
            .const_int(elem_ty.storage_slots(ns).to_u64().unwrap(), false);

        let length_bytes = binary
            .builder
            .build_call(
                binary.module.get_function("account_data_len").unwrap(),
                &[data.into(), offset.into()],
                "length",
            )
            .unwrap()
            .try_as_basic_value()
            .left()
            .unwrap()
            .into_int_value();

        binary
            .builder
            .build_int_unsigned_div(length_bytes, member_size, "")
            .unwrap()
    }

    fn get_storage_int(
        &self,
        _binary: &Binary<'a>,
        _function: FunctionValue,
        _slot: PointerValue<'a>,
        _ty: IntType<'a>,
    ) -> IntValue<'a> {
        // unused
        unreachable!();
    }

    /// Recursively load a type from binary storage. This overrides the default method
    /// in the trait, which is for chains with 256 bit storage keys.
    fn storage_load(
        &self,
        binary: &Binary<'a>,
        ty: &ast::Type,
        slot: &mut IntValue<'a>,
        function: FunctionValue<'a>,
        ns: &ast::Namespace,
        _storage_type: &Option<StorageType>,
    ) -> BasicValueEnum<'a> {
        let data = self.contract_storage_data(binary);

        // the slot is simply the offset after the magic
        let member = unsafe {
            binary
                .builder
                .build_gep(binary.context.i8_type(), data, &[*slot], "data")
                .unwrap()
        };

        match ty {
            ast::Type::String | ast::Type::DynamicBytes => {
                let offset = binary
                    .builder
                    .build_load(binary.context.i32_type(), member, "offset")
                    .unwrap()
                    .into_int_value();

                let string_length = binary
                    .builder
                    .build_call(
                        binary.module.get_function("account_data_len").unwrap(),
                        &[data.into(), offset.into()],
                        "free",
                    )
                    .unwrap()
                    .try_as_basic_value()
                    .left()
                    .unwrap()
                    .into_int_value();

                let string_data = unsafe {
                    binary
                        .builder
                        .build_gep(binary.context.i8_type(), data, &[offset], "string_data")
                        .unwrap()
                };

                binary
                    .builder
                    .build_call(
                        binary.module.get_function("vector_new").unwrap(),
                        &[
                            string_length.into(),
                            binary.context.i32_type().const_int(1, false).into(),
                            string_data.into(),
                        ],
                        "",
                    )
                    .unwrap()
                    .try_as_basic_value()
                    .left()
                    .unwrap()
            }
            ast::Type::Struct(struct_ty) => {
                let llvm_ty = binary.llvm_type(ty.deref_any(), ns);
                // LLVMSizeOf() produces an i64
                let size = binary
                    .builder
                    .build_int_truncate(
                        llvm_ty.size_of().unwrap(),
                        binary.context.i32_type(),
                        "size_of",
                    )
                    .unwrap();

                let new = binary
                    .builder
                    .build_call(
                        binary.module.get_function("__malloc").unwrap(),
                        &[size.into()],
                        "",
                    )
                    .unwrap()
                    .try_as_basic_value()
                    .left()
                    .unwrap()
                    .into_pointer_value();

                for (i, field) in struct_ty.definition(ns).fields.iter().enumerate() {
                    let field_offset = struct_ty.definition(ns).storage_offsets[i]
                        .to_u64()
                        .unwrap();

                    let mut offset = binary
                        .builder
                        .build_int_add(
                            *slot,
                            binary.context.i32_type().const_int(field_offset, false),
                            "field_offset",
                        )
                        .unwrap();

                    let val =
                        self.storage_load(binary, &field.ty, &mut offset, function, ns, &None);

                    let elem = unsafe {
                        binary
                            .builder
                            .build_gep(
                                llvm_ty,
                                new,
                                &[
                                    binary.context.i32_type().const_zero(),
                                    binary.context.i32_type().const_int(i as u64, false),
                                ],
                                field.name_as_str(),
                            )
                            .unwrap()
                    };

                    let val = if field.ty.is_fixed_reference_type(ns) {
                        let load_ty = binary.llvm_type(&field.ty, ns);
                        binary
                            .builder
                            .build_load(load_ty, val.into_pointer_value(), "elem")
                            .unwrap()
                    } else {
                        val
                    };

                    binary.builder.build_store(elem, val).unwrap();
                }

                new.into()
            }
            ast::Type::Array(elem_ty, dim) => {
                let llvm_ty = binary.llvm_type(ty.deref_any(), ns);

                let dest;
                let length;
                let mut slot = *slot;

                if matches!(dim.last().unwrap(), ast::ArrayLength::Fixed(_)) {
                    // LLVMSizeOf() produces an i64 and malloc takes i32
                    let size = binary
                        .builder
                        .build_int_truncate(
                            llvm_ty.size_of().unwrap(),
                            binary.context.i32_type(),
                            "size_of",
                        )
                        .unwrap();

                    dest = binary
                        .builder
                        .build_call(
                            binary.module.get_function("__malloc").unwrap(),
                            &[size.into()],
                            "",
                        )
                        .unwrap()
                        .try_as_basic_value()
                        .left()
                        .unwrap()
                        .into_pointer_value();

                    length = binary.context.i32_type().const_int(
                        if let Some(ast::ArrayLength::Fixed(d)) = dim.last() {
                            d.to_u64().unwrap()
                        } else {
                            unreachable!()
                        },
                        false,
                    );
                } else {
                    let llvm_elem_ty = binary.llvm_field_ty(elem_ty, ns);
                    let elem_size = binary
                        .builder
                        .build_int_truncate(
                            llvm_elem_ty.size_of().unwrap(),
                            binary.context.i32_type(),
                            "size_of",
                        )
                        .unwrap();

                    length = self.storage_array_length(binary, function, slot, elem_ty, ns);

                    slot = binary
                        .builder
                        .build_load(binary.context.i32_type(), member, "offset")
                        .unwrap()
                        .into_int_value();

                    dest = binary.vector_new(length, elem_size, None);
                };

                let elem_size = elem_ty.solana_storage_size(ns).to_u64().unwrap();

                // loop over the array
                let mut builder = LoopBuilder::new(binary, function);

                // we need a phi for the offset
                let offset_phi =
                    builder.add_loop_phi(binary, "offset", slot.get_type(), slot.into());

                let index = builder.over(binary, binary.context.i32_type().const_zero(), length);

                let elem = binary.array_subscript(ty.deref_any(), dest, index, ns);

                let elem_ty = ty.array_deref();

                let mut offset_val = offset_phi.into_int_value();

                let val = self.storage_load(
                    binary,
                    elem_ty.deref_memory(),
                    &mut offset_val,
                    function,
                    ns,
                    &None,
                );

                let val = if elem_ty.deref_memory().is_fixed_reference_type(ns) {
                    let load_ty = binary.llvm_type(elem_ty.deref_any(), ns);
                    binary
                        .builder
                        .build_load(load_ty, val.into_pointer_value(), "elem")
                        .unwrap()
                } else {
                    val
                };

                binary.builder.build_store(elem, val).unwrap();

                offset_val = binary
                    .builder
                    .build_int_add(
                        offset_val,
                        binary.context.i32_type().const_int(elem_size, false),
                        "new_offset",
                    )
                    .unwrap();

                // set the offset for the next iteration of the loop
                builder.set_loop_phi_value(binary, "offset", offset_val.into());

                // done
                builder.finish(binary);

                dest.into()
            }
            _ => binary
                .builder
                .build_load(binary.llvm_var_ty(ty, ns), member, "")
                .unwrap(),
        }
    }

    fn storage_store(
        &self,
        binary: &Binary<'a>,
        ty: &ast::Type,
        existing: bool,
        offset: &mut IntValue<'a>,
        val: BasicValueEnum<'a>,
        function: FunctionValue<'a>,
        ns: &ast::Namespace,
        _: &Option<StorageType>,
    ) {
        let data = self.contract_storage_data(binary);
        let account = self.contract_storage_account(binary);

        // the slot is simply the offset after the magic
        let member = unsafe {
            binary
                .builder
                .build_gep(binary.context.i8_type(), data, &[*offset], "data")
                .unwrap()
        };

        if *ty == ast::Type::String || *ty == ast::Type::DynamicBytes {
            let new_string_length = binary.vector_len(val);

            let offset = if existing {
                let offset = binary
                    .builder
                    .build_load(binary.context.i32_type(), member, "offset")
                    .unwrap()
                    .into_int_value();

                // get the length of the existing string in storage
                let existing_string_length = binary
                    .builder
                    .build_call(
                        binary.module.get_function("account_data_len").unwrap(),
                        &[data.into(), offset.into()],
                        "length",
                    )
                    .unwrap()
                    .try_as_basic_value()
                    .left()
                    .unwrap()
                    .into_int_value();

                // do we need to reallocate?
                let allocation_necessary = binary
                    .builder
                    .build_int_compare(
                        IntPredicate::NE,
                        existing_string_length,
                        new_string_length,
                        "allocation_necessary",
                    )
                    .unwrap();

                let entry = binary.builder.get_insert_block().unwrap();

                let realloc = binary.context.append_basic_block(function, "realloc");
                let memcpy = binary.context.append_basic_block(function, "memcpy");

                binary
                    .builder
                    .build_conditional_branch(allocation_necessary, realloc, memcpy)
                    .unwrap();

                binary.builder.position_at_end(realloc);

                // do not realloc since we're copying everything
                binary
                    .builder
                    .build_call(
                        binary.module.get_function("account_data_free").unwrap(),
                        &[data.into(), offset.into()],
                        "free",
                    )
                    .unwrap();

                // account_data_alloc will return offset = 0 if the string is length 0
                let rc = binary
                    .builder
                    .build_call(
                        binary.module.get_function("account_data_alloc").unwrap(),
                        &[account.into(), new_string_length.into(), member.into()],
                        "alloc",
                    )
                    .unwrap()
                    .try_as_basic_value()
                    .left()
                    .unwrap()
                    .into_int_value();

                let is_rc_zero = binary
                    .builder
                    .build_int_compare(
                        IntPredicate::EQ,
                        rc,
                        binary.context.i64_type().const_zero(),
                        "is_rc_zero",
                    )
                    .unwrap();

                let rc_not_zero = binary.context.append_basic_block(function, "rc_not_zero");
                let rc_zero = binary.context.append_basic_block(function, "rc_zero");

                binary
                    .builder
                    .build_conditional_branch(is_rc_zero, rc_zero, rc_not_zero)
                    .unwrap();

                binary.builder.position_at_end(rc_not_zero);

                self.return_code(
                    binary,
                    binary.context.i64_type().const_int(5u64 << 32, false),
                );

                binary.builder.position_at_end(rc_zero);

                let new_offset = binary
                    .builder
                    .build_load(binary.context.i32_type(), member, "new_offset")
                    .unwrap();

                binary.builder.build_unconditional_branch(memcpy).unwrap();

                binary.builder.position_at_end(memcpy);

                let offset_phi = binary
                    .builder
                    .build_phi(binary.context.i32_type(), "offset")
                    .unwrap();

                offset_phi.add_incoming(&[(&new_offset, rc_zero), (&offset, entry)]);

                offset_phi.as_basic_value().into_int_value()
            } else {
                // account_data_alloc will return offset = 0 if the string is length 0
                let rc = binary
                    .builder
                    .build_call(
                        binary.module.get_function("account_data_alloc").unwrap(),
                        &[account.into(), new_string_length.into(), member.into()],
                        "alloc",
                    )
                    .unwrap()
                    .try_as_basic_value()
                    .left()
                    .unwrap()
                    .into_int_value();

                let is_rc_zero = binary
                    .builder
                    .build_int_compare(
                        IntPredicate::EQ,
                        rc,
                        binary.context.i64_type().const_zero(),
                        "is_rc_zero",
                    )
                    .unwrap();

                let rc_not_zero = binary.context.append_basic_block(function, "rc_not_zero");
                let rc_zero = binary.context.append_basic_block(function, "rc_zero");

                binary
                    .builder
                    .build_conditional_branch(is_rc_zero, rc_zero, rc_not_zero)
                    .unwrap();

                binary.builder.position_at_end(rc_not_zero);

                self.return_code(
                    binary,
                    binary.context.i64_type().const_int(5u64 << 32, false),
                );

                binary.builder.position_at_end(rc_zero);

                binary
                    .builder
                    .build_load(binary.context.i32_type(), member, "new_offset")
                    .unwrap()
                    .into_int_value()
            };

            let dest_string_data = unsafe {
                binary
                    .builder
                    .build_gep(
                        binary.context.i8_type(),
                        data,
                        &[offset],
                        "dest_string_data",
                    )
                    .unwrap()
            };

            binary
                .builder
                .build_call(
                    binary.module.get_function("__memcpy").unwrap(),
                    &[
                        dest_string_data.into(),
                        binary.vector_bytes(val).into(),
                        new_string_length.into(),
                    ],
                    "copied",
                )
                .unwrap();
        } else if let ast::Type::Array(elem_ty, dim) = ty {
            // make sure any pointers are freed
            if existing {
                self.storage_free(binary, ty, data, *offset, function, false, ns);
            }

            let length = if let Some(ast::ArrayLength::Fixed(length)) = dim.last() {
                binary
                    .context
                    .i32_type()
                    .const_int(length.to_u64().unwrap(), false)
            } else {
                binary.vector_len(val)
            };

            let mut elem_slot = *offset;

            if Some(&ast::ArrayLength::Dynamic) == dim.last() {
                // reallocate to the right size
                let member_size = binary
                    .context
                    .i32_type()
                    .const_int(elem_ty.solana_storage_size(ns).to_u64().unwrap(), false);
                let new_length = binary
                    .builder
                    .build_int_mul(length, member_size, "new_length")
                    .unwrap();
                let offset = binary
                    .builder
                    .build_load(binary.context.i32_type(), member, "offset")
                    .unwrap()
                    .into_int_value();

                let rc = binary
                    .builder
                    .build_call(
                        binary.module.get_function("account_data_realloc").unwrap(),
                        &[
                            account.into(),
                            offset.into(),
                            new_length.into(),
                            member.into(),
                        ],
                        "new_offset",
                    )
                    .unwrap()
                    .try_as_basic_value()
                    .left()
                    .unwrap()
                    .into_int_value();

                let is_rc_zero = binary
                    .builder
                    .build_int_compare(
                        IntPredicate::EQ,
                        rc,
                        binary.context.i64_type().const_zero(),
                        "is_rc_zero",
                    )
                    .unwrap();

                let rc_not_zero = binary.context.append_basic_block(function, "rc_not_zero");
                let rc_zero = binary.context.append_basic_block(function, "rc_zero");

                binary
                    .builder
                    .build_conditional_branch(is_rc_zero, rc_zero, rc_not_zero)
                    .unwrap();

                binary.builder.position_at_end(rc_not_zero);

                self.return_code(
                    binary,
                    binary.context.i64_type().const_int(5u64 << 32, false),
                );

                binary.builder.position_at_end(rc_zero);

                elem_slot = binary
                    .builder
                    .build_load(binary.context.i32_type(), member, "offset")
                    .unwrap()
                    .into_int_value();
            }

            let elem_size = elem_ty.solana_storage_size(ns).to_u64().unwrap();

            // loop over the array
            let mut builder = LoopBuilder::new(binary, function);

            // we need a phi for the offset
            let offset_phi =
                builder.add_loop_phi(binary, "offset", offset.get_type(), elem_slot.into());

            let index = builder.over(binary, binary.context.i32_type().const_zero(), length);

            let elem = binary.array_subscript(ty, val.into_pointer_value(), index, ns);

            let mut offset_val = offset_phi.into_int_value();

            let elem_ty = ty.array_deref();

            self.storage_store(
                binary,
                elem_ty.deref_any(),
                false, // storage already freed with storage_free
                &mut offset_val,
                if elem_ty.deref_memory().is_fixed_reference_type(ns) {
                    elem.into()
                } else {
                    let load_ty = if elem_ty.is_dynamic(ns) {
                        binary
                            .llvm_type(elem_ty.deref_memory(), ns)
                            .ptr_type(AddressSpace::default())
                            .as_basic_type_enum()
                    } else {
                        binary.llvm_type(elem_ty.deref_memory(), ns)
                    };
                    binary
                        .builder
                        .build_load(load_ty, elem, "array_elem")
                        .unwrap()
                },
                function,
                ns,
                &None,
            );

            offset_val = binary
                .builder
                .build_int_add(
                    offset_val,
                    binary.context.i32_type().const_int(elem_size, false),
                    "new_offset",
                )
                .unwrap();

            // set the offset for the next iteration of the loop
            builder.set_loop_phi_value(binary, "offset", offset_val.into());

            // done
            builder.finish(binary);
        } else if let ast::Type::Struct(struct_ty) = ty {
            for (i, field) in struct_ty.definition(ns).fields.iter().enumerate() {
                let field_offset = struct_ty.definition(ns).storage_offsets[i]
                    .to_u64()
                    .unwrap();

                let mut offset = binary
                    .builder
                    .build_int_add(
                        *offset,
                        binary.context.i32_type().const_int(field_offset, false),
                        "field_offset",
                    )
                    .unwrap();

                let val_ty = binary.llvm_type(ty, ns);
                let elem = unsafe {
                    binary
                        .builder
                        .build_gep(
                            val_ty,
                            val.into_pointer_value(),
                            &[
                                binary.context.i32_type().const_zero(),
                                binary.context.i32_type().const_int(i as u64, false),
                            ],
                            field.name_as_str(),
                        )
                        .unwrap()
                };

                // free any existing dynamic storage
                if existing {
                    self.storage_free(binary, &field.ty, data, offset, function, false, ns);
                }

                self.storage_store(
                    binary,
                    &field.ty,
                    existing,
                    &mut offset,
                    if field.ty.is_fixed_reference_type(ns) {
                        elem.into()
                    } else {
                        let load_ty = if field.ty.is_dynamic(ns) {
                            binary
                                .llvm_type(&field.ty, ns)
                                .ptr_type(AddressSpace::default())
                                .as_basic_type_enum()
                        } else {
                            binary.llvm_type(&field.ty, ns)
                        };
                        binary
                            .builder
                            .build_load(load_ty, elem, field.name_as_str())
                            .unwrap()
                    },
                    function,
                    ns,
                    &None,
                );
            }
        } else {
            binary.builder.build_store(member, val).unwrap();
        }
    }

    fn keccak256_hash(
        &self,
        _binary: &Binary,
        _src: PointerValue,
        _length: IntValue,
        _dest: PointerValue,
        _ns: &ast::Namespace,
    ) {
        unreachable!();
    }

    fn return_empty_abi(&self, binary: &Binary) {
        // return 0 for success
        binary
            .builder
            .build_return(Some(&binary.context.i64_type().const_int(0, false)))
            .unwrap();
    }

    fn assert_failure(&self, binary: &Binary, data: PointerValue, length: IntValue) {
        // the reason code should be null (and already printed)
        binary
            .builder
            .build_call(
                binary.module.get_function("sol_set_return_data").unwrap(),
                &[
                    data.into(),
                    binary
                        .builder
                        .build_int_z_extend(length, binary.context.i64_type(), "length")
                        .unwrap()
                        .into(),
                ],
                "",
            )
            .unwrap();

        // return 1 for failure
        binary
            .builder
            .build_return(Some(
                &binary.context.i64_type().const_int(1u64 << 32, false),
            ))
            .unwrap();
    }

    fn print(&self, binary: &Binary, string_ptr: PointerValue, string_len: IntValue) {
        let string_len64 = binary
            .builder
            .build_int_z_extend(string_len, binary.context.i64_type(), "")
            .unwrap();

        binary
            .builder
            .build_call(
                binary.module.get_function("sol_log_").unwrap(),
                &[string_ptr.into(), string_len64.into()],
                "",
            )
            .unwrap();
    }

    /// Create new contract
    fn create_contract<'b>(
        &mut self,
        binary: &Binary<'b>,
        function: FunctionValue<'b>,
        _success: Option<&mut BasicValueEnum<'b>>,
        _contract_no: usize,
        address: PointerValue<'b>,
        encoded_args: BasicValueEnum<'b>,
        encoded_args_len: BasicValueEnum<'b>,
        mut contract_args: ContractArgs<'b>,
        ns: &ast::Namespace,
        _loc: Loc,
    ) {
        contract_args.program_id = Some(address);

        let payload = binary.vector_bytes(encoded_args);
        let payload_len = encoded_args_len.into_int_value();

        assert!(contract_args.accounts.is_some());
        // The AccountMeta array is always present for Solana contracts
        self.build_invoke_signed_c(binary, function, payload, payload_len, contract_args, ns);
    }

    fn builtin_function(
        &self,
        binary: &Binary<'a>,
        function: FunctionValue<'a>,
        builtin_func: &ast::Function,
        args: &[BasicMetadataValueEnum<'a>],
        first_arg_type: Option<BasicTypeEnum>,
        ns: &ast::Namespace,
    ) -> Option<BasicValueEnum<'a>> {
        let first_arg_type =
            first_arg_type.expect("solana does not have builtin without any parameter");

        if builtin_func.id.name == "create_program_address" {
            let func = binary
                .module
                .get_function("sol_create_program_address")
                .unwrap();

            let seed_count = binary
                .context
                .i64_type()
                .const_int(first_arg_type.into_array_type().len() as u64, false);

            // address
            let address = binary.build_alloca(function, binary.address_type(ns), "address");

            binary
                .builder
                .build_store(address, args[1].into_array_value())
                .unwrap();

            let ret = binary
                .builder
                .build_call(
                    func,
                    &[
                        args[0].into_pointer_value().into(),
                        seed_count.into(),
                        address.into(),
                        args[2], // return value
                    ],
                    "",
                )
                .unwrap()
                .try_as_basic_value()
                .left()
                .unwrap();
            Some(ret)
        } else if builtin_func.id.name == "try_find_program_address" {
            let func = binary
                .module
                .get_function("sol_try_find_program_address")
                .unwrap();

            let seed_count = binary
                .context
                .i64_type()
                .const_int(first_arg_type.into_array_type().len() as u64, false);

            // address
            let address = binary.build_alloca(function, binary.address_type(ns), "address");

            binary
                .builder
                .build_store(address, args[1].into_array_value())
                .unwrap();

            let ret = binary
                .builder
                .build_call(
                    func,
                    &[
                        args[0].into_pointer_value().into(),
                        seed_count.into(),
                        address.into(),
                        args[2], // return address/pubkey
                        args[3], // return seed bump
                    ],
                    "",
                )
                .unwrap()
                .try_as_basic_value()
                .left()
                .unwrap();
            Some(ret)
        } else {
            unreachable!();
        }
    }

    /// Call external binary
    fn external_call<'b>(
        &self,
        binary: &Binary<'b>,
        function: FunctionValue<'b>,
        _success: Option<&mut BasicValueEnum<'b>>,
        payload: PointerValue<'b>,
        payload_len: IntValue<'b>,
        address: Option<PointerValue<'b>>,
        mut contract_args: ContractArgs<'b>,
        _ty: ast::CallTy,
        ns: &ast::Namespace,
        _loc: Loc,
    ) {
        let address = address.unwrap();

        if contract_args.accounts.is_none() {
            contract_args.accounts = Some((
                binary
                    .context
                    .i64_type()
                    .ptr_type(AddressSpace::default())
                    .const_zero(),
                binary.context.i32_type().const_zero(),
            ))
        };

        contract_args.program_id = Some(address);
        self.build_invoke_signed_c(binary, function, payload, payload_len, contract_args, ns);
    }

    /// Get return buffer for external call
    fn return_data<'b>(
        &self,
        binary: &Binary<'b>,
        function: FunctionValue<'b>,
    ) -> PointerValue<'b> {
        let null_u8_ptr = binary
            .context
            .i8_type()
            .ptr_type(AddressSpace::default())
            .const_zero();

        let length_as_64 = binary
            .builder
            .build_call(
                binary.module.get_function("sol_get_return_data").unwrap(),
                &[
                    null_u8_ptr.into(),
                    binary.context.i64_type().const_zero().into(),
                    null_u8_ptr.into(),
                ],
                "returndatasize",
            )
            .unwrap()
            .try_as_basic_value()
            .left()
            .unwrap()
            .into_int_value();

        let length = binary
            .builder
            .build_int_truncate(length_as_64, binary.context.i32_type(), "length")
            .unwrap();

        let malloc_length = binary
            .builder
            .build_int_add(
                length,
                binary
                    .module
                    .get_struct_type("struct.vector")
                    .unwrap()
                    .size_of()
                    .unwrap()
                    .const_cast(binary.context.i32_type(), false),
                "size",
            )
            .unwrap();

        let p = binary
            .builder
            .build_call(
                binary.module.get_function("__malloc").unwrap(),
                &[malloc_length.into()],
                "",
            )
            .unwrap()
            .try_as_basic_value()
            .left()
            .unwrap()
            .into_pointer_value();

        let data_len = unsafe {
            binary
                .builder
                .build_gep(
                    binary.module.get_struct_type("struct.vector").unwrap(),
                    p,
                    &[
                        binary.context.i32_type().const_zero(),
                        binary.context.i32_type().const_zero(),
                    ],
                    "data_len",
                )
                .unwrap()
        };

        binary.builder.build_store(data_len, length).unwrap();

        let data_size = unsafe {
            binary
                .builder
                .build_gep(
                    binary.module.get_struct_type("struct.vector").unwrap(),
                    p,
                    &[
                        binary.context.i32_type().const_zero(),
                        binary.context.i32_type().const_int(1, false),
                    ],
                    "data_size",
                )
                .unwrap()
        };

        binary.builder.build_store(data_size, length).unwrap();

        let data = unsafe {
            binary
                .builder
                .build_gep(
                    binary.module.get_struct_type("struct.vector").unwrap(),
                    p,
                    &[
                        binary.context.i32_type().const_zero(),
                        binary.context.i32_type().const_int(2, false),
                    ],
                    "data",
                )
                .unwrap()
        };

        let program_id = binary.build_array_alloca(
            function,
            binary.context.i8_type(),
            binary.context.i32_type().const_int(32, false),
            "program_id",
        );

        binary
            .builder
            .build_call(
                binary.module.get_function("sol_get_return_data").unwrap(),
                &[data.into(), length_as_64.into(), program_id.into()],
                "",
            )
            .unwrap();

        p
    }

    fn return_code<'b>(&self, binary: &'b Binary, ret: IntValue<'b>) {
        binary.builder.build_return(Some(&ret)).unwrap();
    }

    /// Value received is not available on solana
    fn value_transferred<'b>(&self, _binary: &Binary<'b>, _ns: &ast::Namespace) -> IntValue<'b> {
        unreachable!();
    }

    /// Send value to address
    fn value_transfer<'b>(
        &self,
        _binary: &Binary<'b>,
        _function: FunctionValue,
        _success: Option<&mut BasicValueEnum<'b>>,
        _address: PointerValue<'b>,
        _value: IntValue<'b>,
        _ns: &ast::Namespace,
        _loc: Loc,
    ) {
        unreachable!();
    }

    /// Terminate execution, destroy binary and send remaining funds to addr
    fn selfdestruct<'b>(&self, _binary: &Binary<'b>, _addr: ArrayValue<'b>, _ns: &ast::Namespace) {
        unimplemented!();
    }

    /// Emit event
    fn emit_event<'b>(
        &self,
        binary: &Binary<'b>,
        function: FunctionValue<'b>,
        data: BasicValueEnum<'b>,
        _topics: &[BasicValueEnum<'b>],
    ) {
        let fields = binary.build_array_alloca(
            function,
            binary.module.get_struct_type("SolLogDataField").unwrap(),
            binary.context.i32_type().const_int(1, false),
            "fields",
        );

        let field_data = unsafe {
            binary
                .builder
                .build_gep(
                    binary.module.get_struct_type("SolLogDataField").unwrap(),
                    fields,
                    &[
                        binary.context.i32_type().const_zero(),
                        binary.context.i32_type().const_zero(),
                    ],
                    "field_data",
                )
                .unwrap()
        };

        let bytes_pointer = binary.vector_bytes(data);
        binary
            .builder
            .build_store(field_data, bytes_pointer)
            .unwrap();

        let field_len = unsafe {
            binary
                .builder
                .build_gep(
                    binary.module.get_struct_type("SolLogDataField").unwrap(),
                    fields,
                    &[
                        binary.context.i32_type().const_zero(),
                        binary.context.i32_type().const_int(1, false),
                    ],
                    "data_len",
                )
                .unwrap()
        };

        binary
            .builder
            .build_store(
                field_len,
                binary
                    .builder
                    .build_int_z_extend(
                        binary.vector_len(data),
                        binary.context.i64_type(),
                        "data_len64",
                    )
                    .unwrap(),
            )
            .unwrap();

        binary
            .builder
            .build_call(
                binary.module.get_function("sol_log_data").unwrap(),
                &[
                    fields.into(),
                    binary.context.i64_type().const_int(1, false).into(),
                ],
                "",
            )
            .unwrap();
    }

    /// builtin expressions
    fn builtin<'b>(
        &self,
        binary: &Binary<'b>,
        expr: &codegen::Expression,
        vartab: &HashMap<usize, Variable<'b>>,
        function: FunctionValue<'b>,
        ns: &ast::Namespace,
    ) -> BasicValueEnum<'b> {
        match expr {
            codegen::Expression::Builtin {
                kind: codegen::Builtin::Timestamp,
                args,
                ..
            } => {
                assert_eq!(args.len(), 0);

                let parameters = self.sol_parameters(binary);

                let sol_clock = binary.module.get_function("sol_clock").unwrap();

                let clock = binary
                    .builder
                    .build_call(sol_clock, &[parameters.into()], "clock")
                    .unwrap()
                    .try_as_basic_value()
                    .left()
                    .unwrap()
                    .into_pointer_value();

                // This is struct.clock_layout
                let clock_struct = binary
                    .context
                    .struct_type(&[binary.context.i64_type().as_basic_type_enum(); 5], false);
                let timestamp = binary
                    .builder
                    .build_struct_gep(clock_struct, clock, 4, "unix_timestamp")
                    .unwrap();

                binary
                    .builder
                    .build_load(binary.context.i64_type(), timestamp, "timestamp")
                    .unwrap()
            }
            codegen::Expression::Builtin {
                kind: codegen::Builtin::BlockNumber | codegen::Builtin::Slot,
                args,
                ..
            } => {
                assert_eq!(args.len(), 0);

                let parameters = self.sol_parameters(binary);

                let sol_clock = binary.module.get_function("sol_clock").unwrap();

                let clock = binary
                    .builder
                    .build_call(sol_clock, &[parameters.into()], "clock")
                    .unwrap()
                    .try_as_basic_value()
                    .left()
                    .unwrap()
                    .into_pointer_value();

                // This is struct.clock_layout
                let clock_struct = binary
                    .context
                    .struct_type(&[binary.context.i64_type().as_basic_type_enum(); 5], false);
                let slot = binary
                    .builder
                    .build_struct_gep(clock_struct, clock, 0, "slot")
                    .unwrap();

                binary
                    .builder
                    .build_load(binary.context.i64_type(), slot, "timestamp")
                    .unwrap()
            }
            codegen::Expression::Builtin {
                kind: codegen::Builtin::GetAddress,
                args,
                ..
            } => {
                assert_eq!(args.len(), 0);

                let parameters = self.sol_parameters(binary);

                let sol_pubkey_type = binary.module.get_struct_type("struct.SolPubkey").unwrap();
                binary
                    .builder
                    .build_load(
                        sol_pubkey_type.ptr_type(AddressSpace::default()),
                        binary
                            .builder
                            .build_struct_gep(
                                binary
                                    .module
                                    .get_struct_type("struct.SolParameters")
                                    .unwrap(),
                                parameters,
                                4,
                                "program_id",
                            )
                            .unwrap(),
                        "program_id",
                    )
                    .unwrap()
            }
            codegen::Expression::Builtin {
                kind: codegen::Builtin::Calldata,
                args,
                ..
            } => {
                assert_eq!(args.len(), 0);

                let sol_params = self.sol_parameters(binary);

                let input = binary
                    .builder
                    .build_load(
                        binary.context.i8_type().ptr_type(AddressSpace::default()),
                        binary
                            .builder
                            .build_struct_gep(
                                binary
                                    .module
                                    .get_struct_type("struct.SolParameters")
                                    .unwrap(),
                                sol_params,
                                2,
                                "input",
                            )
                            .unwrap(),
                        "data",
                    )
                    .unwrap()
                    .into_pointer_value();

                let input_len = binary
                    .builder
                    .build_load(
                        binary.context.i64_type(),
                        binary
                            .builder
                            .build_struct_gep(
                                binary
                                    .module
                                    .get_struct_type("struct.SolParameters")
                                    .unwrap(),
                                sol_params,
                                3,
                                "input_len",
                            )
                            .unwrap(),
                        "data_len",
                    )
                    .unwrap()
                    .into_int_value();

                let input_len = binary
                    .builder
                    .build_int_truncate(input_len, binary.context.i32_type(), "input_len")
                    .unwrap();

                binary
                    .builder
                    .build_call(
                        binary.module.get_function("vector_new").unwrap(),
                        &[
                            input_len.into(),
                            binary.context.i32_type().const_int(1, false).into(),
                            input.into(),
                        ],
                        "",
                    )
                    .unwrap()
                    .try_as_basic_value()
                    .left()
                    .unwrap()
            }
            codegen::Expression::Builtin {
                kind: codegen::Builtin::Signature,
                args,
                ..
            } => {
                assert_eq!(args.len(), 0);

                let sol_params = self.sol_parameters(binary);

                let input = binary
                    .builder
                    .build_load(
                        binary.context.i8_type().ptr_type(AddressSpace::default()),
                        binary
                            .builder
                            .build_struct_gep(
                                binary
                                    .module
                                    .get_struct_type("struct.SolParameters")
                                    .unwrap(),
                                sol_params,
                                2,
                                "input",
                            )
                            .unwrap(),
                        "data",
                    )
                    .unwrap()
                    .into_pointer_value();

                let selector = binary
                    .builder
                    .build_load(binary.context.i64_type(), input, "selector")
                    .unwrap();

                let bswap = binary.llvm_bswap(64);

                binary
                    .builder
                    .build_call(bswap, &[selector.into()], "")
                    .unwrap()
                    .try_as_basic_value()
                    .left()
                    .unwrap()
            }
            codegen::Expression::Builtin {
                kind: codegen::Builtin::SignatureVerify,
                args,
                ..
            } => {
                assert_eq!(args.len(), 3);

                let address = binary.build_alloca(function, binary.address_type(ns), "address");

                binary
                    .builder
                    .build_store(
                        address,
                        expression(self, binary, &args[0], vartab, function, ns).into_array_value(),
                    )
                    .unwrap();

                let message = expression(self, binary, &args[1], vartab, function, ns);
                let signature = expression(self, binary, &args[2], vartab, function, ns);
                let parameters = self.sol_parameters(binary);
                let signature_verify = binary.module.get_function("signature_verify").unwrap();

                let ret = binary
                    .builder
                    .build_call(
                        signature_verify,
                        &[
                            address.into(),
                            message.into(),
                            signature.into(),
                            parameters.into(),
                        ],
                        "",
                    )
                    .unwrap()
                    .try_as_basic_value()
                    .left()
                    .unwrap()
                    .into_int_value();

                binary
                    .builder
                    .build_int_compare(
                        IntPredicate::EQ,
                        ret,
                        binary.context.i64_type().const_zero(),
                        "success",
                    )
                    .unwrap()
                    .into()
            }
            codegen::Expression::Builtin {
                kind: codegen::Builtin::Accounts,
                args,
                ..
            } => {
                assert_eq!(args.len(), 0);

                let parameters = self.sol_parameters(binary);

                unsafe {
                    binary.builder.build_gep(
                        binary
                            .module
                            .get_struct_type("struct.SolParameters")
                            .unwrap(),
                        parameters,
                        &[
                            binary.context.i32_type().const_int(0, false),
                            binary.context.i32_type().const_int(0, false),
                            binary.context.i32_type().const_int(0, false),
                        ],
                        "accounts",
                    )
                }
                .unwrap()
                .into()
            }
            codegen::Expression::Builtin {
                kind: codegen::Builtin::ArrayLength,
                args,
                ..
            } => {
                assert_eq!(args.len(), 1);

                let parameters = self.sol_parameters(binary);

                let ka_num = binary
                    .builder
                    .build_struct_gep(
                        binary
                            .module
                            .get_struct_type("struct.SolParameters")
                            .unwrap(),
                        parameters,
                        1,
                        "ka_num",
                    )
                    .unwrap();

                let ka_num = binary
                    .builder
                    .build_load(binary.context.i64_type(), ka_num, "ka_num")
                    .unwrap()
                    .into_int_value();

                binary
                    .builder
                    .build_int_truncate(ka_num, binary.context.i32_type(), "ka_num_32bits")
                    .unwrap()
                    .into()
            }
            codegen::Expression::StructMember { expr, member, .. } => {
                let account_info =
                    expression(self, binary, expr, vartab, function, ns).into_pointer_value();

                self.account_info_member(binary, function, account_info, *member, ns)
            }
            _ => unimplemented!(),
        }
    }

    /// Crypto Hash
    fn hash<'b>(
        &self,
        binary: &Binary<'b>,
        function: FunctionValue<'b>,
        hash: HashTy,
        input: PointerValue<'b>,
        input_len: IntValue<'b>,
        ns: &ast::Namespace,
    ) -> IntValue<'b> {
        let (fname, hashlen) = match hash {
            HashTy::Keccak256 => ("sol_keccak256", 32),
            HashTy::Ripemd160 => ("ripemd160", 20),
            HashTy::Sha256 => ("sol_sha256", 32),
            _ => unreachable!(),
        };

        let res = binary.build_array_alloca(
            function,
            binary.context.i8_type(),
            binary.context.i32_type().const_int(hashlen, false),
            "res",
        );

        if hash == HashTy::Ripemd160 {
            binary
                .builder
                .build_call(
                    binary.module.get_function(fname).unwrap(),
                    &[input.into(), input_len.into(), res.into()],
                    "hash",
                )
                .unwrap();
        } else {
            let u64_ty = binary.context.i64_type();

            let sol_keccak256 = binary.module.get_function(fname).unwrap();

            // This is struct.SolBytes
            let sol_bytes = binary.context.struct_type(
                &[
                    binary
                        .context
                        .i8_type()
                        .ptr_type(AddressSpace::default())
                        .as_basic_type_enum(),
                    binary.context.i64_type().as_basic_type_enum(),
                ],
                false,
            );

            let array = binary.build_alloca(function, sol_bytes, "sol_bytes");

            binary
                .builder
                .build_store(
                    binary
                        .builder
                        .build_struct_gep(sol_bytes, array, 0, "input")
                        .unwrap(),
                    input,
                )
                .unwrap();

            binary
                .builder
                .build_store(
                    binary
                        .builder
                        .build_struct_gep(sol_bytes, array, 1, "input_len")
                        .unwrap(),
                    binary
                        .builder
                        .build_int_z_extend(input_len, u64_ty, "input_len")
                        .unwrap(),
                )
                .unwrap();

            binary
                .builder
                .build_call(
                    sol_keccak256,
                    &[
                        array.into(),
                        binary.context.i32_type().const_int(1, false).into(),
                        res.into(),
                    ],
                    "hash",
                )
                .unwrap();
        }

        // bytes32 needs to reverse bytes
        let temp = binary.build_alloca(
            function,
            binary.llvm_type(&ast::Type::Bytes(hashlen as u8), ns),
            "hash",
        );

        binary
            .builder
            .build_call(
                binary.module.get_function("__beNtoleN").unwrap(),
                &[
                    res.into(),
                    temp.into(),
                    binary.context.i32_type().const_int(hashlen, false).into(),
                ],
                "",
            )
            .unwrap();

        binary
            .builder
            .build_load(
                binary.llvm_type(&ast::Type::Bytes(hashlen as u8), ns),
                temp,
                "hash",
            )
            .unwrap()
            .into_int_value()
    }

    fn return_abi_data<'b>(
        &self,
        binary: &Binary<'b>,
        data: PointerValue<'b>,
        data_len: BasicValueEnum<'b>,
    ) {
        binary
            .builder
            .build_call(
                binary.module.get_function("sol_set_return_data").unwrap(),
                &[data.into(), data_len.into()],
                "",
            )
            .unwrap();

        binary
            .builder
            .build_return(Some(&binary.return_values[&ReturnCode::Success]))
            .unwrap();
    }
}
