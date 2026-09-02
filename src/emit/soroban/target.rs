// SPDX-License-Identifier: Apache-2.0

use crate::codegen::cfg::HashTy;
use crate::codegen::error::CodegenError;
use crate::codegen::Expression;
use crate::emit::binary::Binary;
use crate::emit::soroban::{HostFunctions, SorobanTarget};
use crate::emit::ContractArgs;
use crate::emit::{TargetRuntime, Variable};
use crate::emit_context;
use crate::sema::ast;
use crate::sema::ast::CallTy;
use crate::sema::ast::{Function, Type};

use inkwell::types::{BasicTypeEnum, IntType};
use inkwell::values::{
    ArrayValue, BasicMetadataValueEnum, BasicValue, BasicValueEnum, FunctionValue, IntValue,
    PointerValue,
};

use solang_parser::helpers::CodeLocation;
use solang_parser::pt::{Loc, StorageType};

use std::collections::HashMap;

fn unsupported_soroban<T>(loc: Loc, operation: impl Into<String>) -> T {
    panic!(
        "{}",
        CodegenError::unsupported_soroban_operation(loc, operation)
    )
}

fn runtime_helper<'a>(
    bin: &Binary<'a>,
    name: &str,
    operation: impl Into<String>,
) -> FunctionValue<'a> {
    bin.module.get_function(name).unwrap_or_else(|| {
        panic!(
            "{}",
            CodegenError::missing_runtime_helper(name, operation, bin.ns.target)
        )
    })
}

fn expect_llvm_entity<T>(
    value: Option<T>,
    operation: impl Into<String>,
    entity: impl Into<String>,
) -> T {
    value.unwrap_or_else(|| panic!("{}", CodegenError::missing_llvm_entity(operation, entity)))
}

fn expect_return_value<T>(value: Option<T>, operation: impl Into<String>) -> T {
    expect_llvm_entity(value, operation, "expected return value")
}

fn invalid_cfg<T>(operation: impl Into<String>, reason: impl Into<String>) -> T {
    panic!("{}", CodegenError::invalid_cfg_invariant(operation, reason))
}

// TODO: Implement TargetRuntime for SorobanTarget.
#[allow(unused_variables)]
impl<'a> TargetRuntime<'a> for SorobanTarget {
    fn get_storage_int(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue,
        slot: PointerValue<'a>,
        ty: IntType<'a>,
    ) -> IntValue<'a> {
        unsupported_soroban(Loc::Codegen, "raw storage integer loads")
    }

    fn storage_load(
        &self,
        bin: &Binary<'a>,
        ty: &ast::Type,
        slot: &mut IntValue<'a>,
        slot_ty: Option<&ast::Type>,
        function: FunctionValue<'a>,
        storage_type: &Option<StorageType>,
    ) -> BasicValueEnum<'a> {
        // Scalar-element storage-array subscript reads are handled in codegen now
        // (arrays.rs, vec_get); this path only serves other storage loads.
        let storage_type = storage_type_to_int(storage_type);
        emit_context!(bin);

        let slot = if slot.is_const() {
            slot.as_basic_value_enum()
                .into_int_value()
                .const_cast(bin.context.i64_type(), false)
        } else {
            *slot
        };

        // === Call HasContractData ===
        let has_data_val = call!(
            HostFunctions::HasContractData.name(),
            &[
                slot.into(),
                bin.context.i64_type().const_int(storage_type, false).into(),
            ]
        )
        .try_as_basic_value()
        .left()
        .unwrap()
        .into_int_value();

        // === Use helper to check if it's true ===
        let condition = is_val_true(bin, has_data_val);

        // === Prepare blocks ===
        let parent = function;
        let then_bb = bin.context.append_basic_block(parent, "has_data");
        let else_bb = bin.context.append_basic_block(parent, "no_data");
        let merge_bb = bin.context.append_basic_block(parent, "merge");

        bin.builder
            .build_conditional_branch(condition, then_bb, else_bb)
            .unwrap();

        // === THEN block: call GetContractData ===
        bin.builder.position_at_end(then_bb);
        let value_from_contract = call!(
            HostFunctions::GetContractData.name(),
            &[
                slot.into(),
                bin.context.i64_type().const_int(storage_type, false).into(),
            ]
        )
        .try_as_basic_value()
        .left()
        .unwrap();
        bin.builder.build_unconditional_branch(merge_bb).unwrap();
        let then_value = value_from_contract;

        // === ELSE block: return default ===
        bin.builder.position_at_end(else_bb);
        let default_value = type_to_tagged_zero_val(bin, ty);

        bin.builder.build_unconditional_branch(merge_bb).unwrap();

        // === MERGE block with phi node ===
        bin.builder.position_at_end(merge_bb);
        let phi = bin
            .builder
            .build_phi(bin.context.i64_type(), "storage_result")
            .unwrap();
        phi.add_incoming(&[(&then_value, then_bb), (&default_value, else_bb)]);

        phi.as_basic_value()
    }

    /// Recursively store a type to storage
    fn storage_store(
        &self,
        bin: &Binary<'a>,
        ty: &ast::Type,
        existing: bool,
        slot: &mut IntValue<'a>,
        slot_ty: Option<&ast::Type>,
        dest: BasicValueEnum<'a>,
        function: FunctionValue<'a>,
        storage_type: &Option<StorageType>,
    ) {
        // Scalar-element storage-array subscript writes are handled in codegen now
        // (arrays.rs, vec_put); this path only serves other storage stores.
        emit_context!(bin);

        let storage_type = storage_type_to_int(storage_type);

        let function_value = bin
            .module
            .get_function(HostFunctions::PutContractData.name())
            .unwrap();
        let slot = if slot.is_const() {
            slot.as_basic_value_enum()
                .into_int_value()
                .const_cast(bin.context.i64_type(), false)
        } else {
            *slot
        };

        let value = bin
            .builder
            .build_call(
                function_value,
                &[
                    slot.into(),
                    dest.into(),
                    bin.context.i64_type().const_int(storage_type, false).into(),
                ],
                HostFunctions::PutContractData.name(),
            )
            .unwrap()
            .try_as_basic_value()
            .left()
            .unwrap()
            .into_int_value();
    }

    /// Recursively clear storage. The default implementation is for slot-based storage
    fn storage_delete(
        &self,
        bin: &Binary<'a>,
        ty: &Type,
        slot: &mut IntValue<'a>,
        function: FunctionValue<'a>,
    ) {
        let storage_type = storage_type_to_int(&None);

        let type_int = bin.context.i64_type().const_int(storage_type, false);

        let function_value = bin
            .module
            .get_function(HostFunctions::DeleteContractData.name())
            .unwrap();

        let call = bin
            .builder
            .build_call(
                function_value,
                &[slot.as_basic_value_enum().into(), type_int.into()],
                "del_contract_data",
            )
            .unwrap();
    }

    // Bytes and string have special storage layout
    fn set_storage_string(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue<'a>,
        slot: PointerValue<'a>,
        dest: BasicValueEnum<'a>,
    ) {
        unsupported_soroban(Loc::Codegen, "storage string and bytes stores")
    }

    fn get_storage_string(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue,
        slot: PointerValue<'a>,
    ) -> PointerValue<'a> {
        unsupported_soroban(Loc::Codegen, "storage string and bytes loads")
    }

    fn set_storage_extfunc(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue,
        slot: PointerValue,
        dest: PointerValue,
        dest_ty: BasicTypeEnum,
    ) {
        unsupported_soroban(Loc::Codegen, "storage external function stores")
    }

    fn get_storage_extfunc(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue,
        slot: PointerValue<'a>,
    ) -> PointerValue<'a> {
        unsupported_soroban(Loc::Codegen, "storage external function loads")
    }

    fn get_storage_bytes_subscript(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue,
        slot: IntValue<'a>,
        index: IntValue<'a>,
        _loc: Loc,
    ) -> IntValue<'a> {
        emit_context!(bin);

        // Load BytesObject handle from persistent storage.
        let bytes_obj = call!(
            HostFunctions::GetContractData.name(),
            &[slot.into(), i64_const!(1).into()],
            "bytes_load"
        )
        .try_as_basic_value()
        .left()
        .unwrap()
        .into_int_value();

        let idx_encoded = encode_value(index, 32, 4, bin);

        // bytes_get(handle, U32Val(i)) → U32Val(byte)
        let raw = call!(
            HostFunctions::BytesGet.name(),
            &[bytes_obj.into(), idx_encoded.into()],
            "bytes_get"
        )
        .try_as_basic_value()
        .left()
        .unwrap()
        .into_int_value();

        // Decode U32Val: shift right 32 and truncate to i8 (bytes1).
        let shifted = bin
            .builder
            .build_right_shift(raw, i64_const!(32), false, "byte_raw")
            .unwrap();
        bin.builder
            .build_int_truncate(shifted, bin.context.i8_type(), "byte_val")
            .unwrap()
    }

    fn set_storage_bytes_subscript(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue,
        slot: IntValue<'a>,
        index: IntValue<'a>,
        value: IntValue<'a>,
        _loc: Loc,
    ) {
        emit_context!(bin);

        // Load existing BytesObject handle from persistent storage.
        let bytes_obj = call!(
            HostFunctions::GetContractData.name(),
            &[slot.into(), i64_const!(1).into()],
            "bytes_load"
        )
        .try_as_basic_value()
        .left()
        .unwrap()
        .into_int_value();

        let idx_encoded = encode_value(index, 32, 4, bin);

        // bytes1 value is i8; zero-extend to i32 for U32Val encoding.
        let value = if value.get_type().get_bit_width() != 32 {
            bin.builder
                .build_int_z_extend(value, bin.context.i32_type(), "byte32")
                .unwrap()
        } else {
            value
        };
        let val_encoded = encode_value(value, 32, 4, bin);

        // bytes_put(handle, U32Val(i), U32Val(byte)) → new BytesObject
        let new_obj = call!(
            HostFunctions::BytesPut.name(),
            &[bytes_obj.into(), idx_encoded.into(), val_encoded.into()],
            "bytes_put"
        )
        .try_as_basic_value()
        .left()
        .unwrap()
        .into_int_value();

        // Write the new handle back to persistent storage.
        call!(
            HostFunctions::PutContractData.name(),
            &[slot.into(), new_obj.into(), i64_const!(1).into()],
            "bytes_store"
        );
    }

    fn storage_subscript(
        &self,
        _bin: &Binary<'a>,
        _function: FunctionValue<'a>,
        _ty: &Type,
        _slot: IntValue<'a>,
        _index: BasicValueEnum<'a>,
    ) -> IntValue<'a> {
        // All storage subscripts (arrays and mappings) are lowered in codegen now
        // (storage_path.rs: vec_get/vec_put for arrays, map_get/map_put for
        // mappings), so no `Subscript` storage expression reaches emit.
        unsupported_soroban(Loc::Codegen, "storage subscript")
    }

    fn storage_push(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue<'a>,
        ty: &Type,
        slot: IntValue<'a>,
        val: Option<BasicValueEnum<'a>>,
    ) -> BasicValueEnum<'a> {
        unsupported_soroban(Loc::Codegen, "storage array push")
    }

    fn storage_pop(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue<'a>,
        ty: &Type,
        slot: IntValue<'a>,
        load: bool,
        loc: Loc,
    ) -> Option<BasicValueEnum<'a>> {
        unsupported_soroban(loc, "storage array pop")
    }

    fn storage_array_length(
        &self,
        _bin: &Binary<'a>,
        _function: FunctionValue,
        _slot: IntValue<'a>,
        _elem_ty: &Type,
    ) -> IntValue<'a> {
        // Every storage array length is computed in codegen now (arrays.rs, vec_len).
        unsupported_soroban(Loc::Codegen, "storage array length")
    }

    /// keccak256 hash
    fn keccak256_hash(
        &self,
        bin: &Binary<'a>,
        src: PointerValue,
        length: IntValue,
        dest: PointerValue,
    ) {
        unsupported_soroban(Loc::Codegen, "keccak256 hashing")
    }

    /// Prints a string
    /// TODO: Implement this function, with a call to the `log` function in the Soroban runtime.
    fn print<'b>(&self, bin: &Binary<'b>, string: PointerValue<'b>, length: IntValue<'b>) {
        let msg_pos = bin
            .builder
            .build_ptr_to_int(string, bin.context.i64_type(), "msg_pos")
            .unwrap();

        let msg_pos_encoded = encode_value(msg_pos, 32, 4, bin);
        let length_encoded = encode_value(length, 32, 4, bin);

        bin.builder
            .build_call(
                bin.module
                    .get_function(HostFunctions::LogFromLinearMemory.name())
                    .unwrap(),
                &[
                    msg_pos_encoded.into(),
                    length_encoded.into(),
                    msg_pos_encoded.into(),
                    encode_value(bin.context.i64_type().const_zero(), 32, 4, bin).into(),
                ],
                "log",
            )
            .unwrap();
    }

    /// Return success without any result
    fn return_empty_abi(&self, bin: &Binary) {
        unsupported_soroban(Loc::Codegen, "empty ABI returns")
    }

    /// Return failure code
    fn return_code<'b>(&self, bin: &'b Binary, ret: IntValue<'b>) {
        unsupported_soroban(Loc::Codegen, "return codes")
    }

    /// Return failure without any result
    fn assert_failure(&self, bin: &Binary, data: PointerValue, length: IntValue) {
        bin.builder.build_unreachable().unwrap();
    }

    fn builtin_function(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue<'a>,
        builtin_func: &Function,
        args: &[BasicMetadataValueEnum<'a>],
        first_arg_type: Option<BasicTypeEnum>,
    ) -> Option<BasicValueEnum<'a>> {
        unsupported_soroban(Loc::Codegen, "target-specific builtin functions")
    }

    /// Calls constructor
    fn create_contract<'b>(
        &mut self,
        bin: &Binary<'b>,
        function: FunctionValue<'b>,
        success: Option<&mut BasicValueEnum<'b>>,
        contract_no: usize,
        address: PointerValue<'b>,
        encoded_args: BasicValueEnum<'b>,
        encoded_args_len: BasicValueEnum<'b>,
        contract_args: ContractArgs<'b>,
        loc: Loc,
    ) {
        unsupported_soroban(loc, "contract construction")
    }

    /// call external function
    fn external_call<'b>(
        &self,
        bin: &Binary<'b>,
        function: FunctionValue<'b>,
        success: Option<&mut BasicValueEnum<'b>>,
        payload: PointerValue<'b>,
        payload_len: IntValue<'b>,
        address: Option<BasicValueEnum<'b>>,
        contract_args: ContractArgs<'b>,
        ty: CallTy,
        loc: Loc,
    ) {
        let offset = bin.context.i64_type().const_int(0, false);

        let start = unsafe {
            bin.builder
                .build_gep(
                    bin.context.i64_type().array_type(1),
                    payload,
                    &[bin.context.i64_type().const_zero(), offset],
                    "start",
                )
                .unwrap()
        };

        let symbol = bin
            .builder
            .build_load(bin.context.i64_type(), start, "symbol")
            .unwrap()
            .into_int_value();

        let args_len = bin
            .builder
            .build_int_unsigned_div(
                payload_len,
                payload_len.get_type().const_int(8, false),
                "args_len",
            )
            .unwrap();

        let args_len = bin
            .builder
            .build_int_sub(
                args_len,
                args_len.get_type().const_int(1, false),
                "args_len",
            )
            .unwrap();

        let args_len_encoded = encode_value(args_len, 32, 4, bin);

        let offset = bin.context.i64_type().const_int(1, false);
        let args_ptr = unsafe {
            bin.builder
                .build_gep(
                    bin.context.i64_type().array_type(1),
                    payload,
                    &[bin.context.i64_type().const_zero(), offset],
                    "start",
                )
                .unwrap()
        };

        let args_ptr_to_int = bin
            .builder
            .build_ptr_to_int(args_ptr, bin.context.i64_type(), "args_ptr")
            .unwrap();

        let args_ptr_encoded = encode_value(args_ptr_to_int, 32, 4, bin);

        let vec_object = bin
            .builder
            .build_call(
                runtime_helper(
                    bin,
                    HostFunctions::VectorNewFromLinearMemory.name(),
                    "building Soroban external call argument vector",
                ),
                &[args_ptr_encoded.into(), args_len_encoded.into()],
                "vec_object",
            )
            .unwrap()
            .try_as_basic_value()
            .left()
            .unwrap_or_else(|| {
                expect_return_value(None, "building Soroban external call argument vector")
            })
            .into_int_value();

        let call_res = bin
            .builder
            .build_call(
                runtime_helper(
                    bin,
                    HostFunctions::Call.name(),
                    "emitting Soroban external call",
                ),
                &[
                    expect_llvm_entity(
                        address,
                        "emitting Soroban external call",
                        "target contract address",
                    )
                    .into(),
                    symbol.into(),
                    vec_object.into(),
                ],
                "call",
            )
            .unwrap()
            .try_as_basic_value()
            .left()
            .unwrap_or_else(|| expect_return_value(None, "emitting Soroban external call"))
            .into_int_value();

        let ret_vector = bin.vector_new(
            bin.context.i32_type().const_int(8, false),
            bin.context.i32_type().const_int(1, false),
            None,
        );
        let ret_data = bin.vector_bytes(ret_vector);
        bin.builder.build_store(ret_data, call_res).unwrap();
        *bin.return_data.borrow_mut() = Some(ret_vector.into_pointer_value());

        if let Some(success) = success {
            *success = bin.context.i32_type().const_int(1, false).into();
        }
    }

    /// send value to address
    fn value_transfer<'b>(
        &self,
        _bin: &Binary<'b>,
        _function: FunctionValue,
        _success: Option<&mut BasicValueEnum<'b>>,
        _address: PointerValue<'b>,
        _value: IntValue<'b>,
        loc: Loc,
    ) {
        unsupported_soroban(loc, "value transfer")
    }

    /// builtin expressions
    fn builtin<'b>(
        &self,
        _bin: &Binary<'b>,
        expr: &Expression,
        _vartab: &HashMap<usize, Variable<'b>>,
        _function: FunctionValue<'b>,
    ) -> BasicValueEnum<'b> {
        unsupported_soroban(expr.loc(), "this Soroban builtin")
    }

    /// Return the return data from an external call (either revert error or return values)
    fn return_data<'b>(&self, bin: &Binary<'b>, function: FunctionValue<'b>) -> PointerValue<'b> {
        bin.return_data.borrow().unwrap()
    }

    /// Return the value we received
    fn value_transferred<'b>(&self, bin: &Binary<'b>) -> IntValue<'b> {
        unsupported_soroban(Loc::Codegen, "transferred value reads")
    }

    /// Terminate execution, destroy bin and send remaining funds to addr
    fn selfdestruct<'b>(&self, bin: &Binary<'b>, addr: ArrayValue<'b>) {
        unsupported_soroban(Loc::Codegen, "selfdestruct")
    }

    /// Crypto Hash
    fn hash<'b>(
        &self,
        bin: &Binary<'b>,
        function: FunctionValue<'b>,
        hash: HashTy,
        string: PointerValue<'b>,
        length: IntValue<'b>,
    ) -> IntValue<'b> {
        unsupported_soroban(Loc::Codegen, "hash builtins")
    }

    /// Emit event
    fn emit_event<'b>(
        &self,
        bin: &Binary<'b>,
        _function: FunctionValue<'b>,
        data: BasicValueEnum<'b>,
        topics: &[BasicValueEnum<'b>],
    ) {
        emit_context!(bin);

        // Build a Soroban VecObject from the topics slice.
        // VectorNew() -> VecObject
        let mut topics_vec = call!(HostFunctions::VectorNew.name(), &[])
            .try_as_basic_value()
            .left()
            .unwrap();

        // VecPushBack(vec: VecObject, val: Val) -> VecObject
        for topic in topics.iter() {
            topics_vec = call!(
                HostFunctions::VecPushBack.name(),
                &[topics_vec.into(), (*topic).into()]
            )
            .try_as_basic_value()
            .left()
            .unwrap();
        }

        // contract_event(topics: VecObject, data: Val) -> Void
        call!(
            HostFunctions::ContractEvent.name(),
            &[topics_vec.into(), data.into()]
        );
    }

    /// Return ABI encoded data
    fn return_abi_data<'b>(
        &self,
        bin: &Binary<'b>,
        data: PointerValue<'b>,
        data_len: BasicValueEnum<'b>,
    ) {
        unsupported_soroban(Loc::Codegen, "ABI data returns")
    }
}

fn storage_type_to_int(storage_type: &Option<StorageType>) -> u64 {
    if let Some(storage_type) = storage_type {
        match storage_type {
            StorageType::Temporary(_) => 0,
            StorageType::Persistent(_) => 1,
            StorageType::Instance(_) => 2,
        }
    } else {
        1
    }
}

fn encode_value<'a>(
    mut value: IntValue<'a>,
    shift: u64,
    add: u64,
    bin: &Binary<'a>,
) -> IntValue<'a> {
    match value.get_type().get_bit_width() {
        32 =>
        // extend to 64 bits
        {
            value = bin
                .builder
                .build_int_z_extend(value, bin.context.i64_type(), "temp")
                .unwrap();
        }
        64 => (),
        _ => invalid_cfg(
            "encoding Soroban host value",
            "expected a 32-bit or 64-bit integer value",
        ),
    }

    let shifted = bin
        .builder
        .build_left_shift(
            value,
            bin.context.i64_type().const_int(shift, false),
            "temp",
        )
        .unwrap();

    bin.builder
        .build_int_add(
            shifted,
            bin.context.i64_type().const_int(add, false),
            "encoded",
        )
        .unwrap()
}

fn is_val_true<'ctx>(bin: &Binary<'ctx>, val: IntValue<'ctx>) -> IntValue<'ctx> {
    let tag_mask = bin.context.i64_type().const_int(0xff, false);
    let tag_true = bin.context.i64_type().const_int(1, false);

    let tag = bin
        .builder
        .build_and(val, tag_mask, "val_tag")
        .expect("build_and failed");

    bin.builder
        .build_int_compare(inkwell::IntPredicate::EQ, tag, tag_true, "is_val_true")
        .expect("build_int_compare failed")
}

/// Returns a Val representing a default zero value with the correct Soroban Tag.
pub fn type_to_tagged_zero_val<'ctx>(bin: &Binary<'ctx>, ty: &Type) -> IntValue<'ctx> {
    let context = &bin.context;
    let i64_type = context.i64_type();

    // Tag definitions from CAP-0046
    let tag = match ty {
        Type::Bool => 0,          // Tag::False
        Type::Uint(32) => 4,      // Tag::U32Val
        Type::Int(32) => 5,       // Tag::I32Val
        Type::Enum(_) => 4,       // Tag::U32Val
        Type::Uint(64) => 6,      // Tag::U64Small
        Type::Int(64) => 7,       // Tag::I64Small
        Type::Uint(128) => 10,    // Tag::U128Small
        Type::Int(128) => 11,     // Tag::I128Small
        Type::Uint(256) => 12,    // Tag::U256Small
        Type::Int(256) => 13,     // Tag::I256Small
        Type::Bytes(_) => 72,     // Tag::BytesObject
        Type::String => 73,       // Tag::StringObject
        Type::DynamicBytes => 72, // Tag::BytesObject
        Type::Address(_) => 77,   // Tag::AddressObject
        Type::Void => 2,          // Tag::Void
        _ => {
            // Fallback to Void for unsupported types
            2 // Tag::Void
        }
    };

    // All zero body + tag in lower 8 bits
    let tag_val: u64 = tag;
    i64_type.const_int(tag_val, false)
}
