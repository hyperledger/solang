// SPDX-License-Identifier: Apache-2.0

use crate::codegen::cfg::HashTy;
use crate::codegen::Expression;
use crate::emit::binary::Binary;
use crate::emit::soroban::{
    SorobanTarget, GET_CONTRACT_DATA, LOG_FROM_LINEAR_MEMORY, PUT_CONTRACT_DATA,
};
use crate::emit::ContractArgs;
use crate::emit::{TargetRuntime, Variable};
use crate::emit_context;
use crate::sema::ast;
use crate::sema::ast::CallTy;
use crate::sema::ast::{Function, Namespace, Type};

use inkwell::types::{BasicTypeEnum, IntType};
use inkwell::values::{
    ArrayValue, BasicMetadataValueEnum, BasicValue, BasicValueEnum, FunctionValue, IntValue,
    PointerValue,
};

use solang_parser::pt::{Loc, StorageType};

use std::collections::HashMap;

// TODO: Implement TargetRuntime for SorobanTarget.
#[allow(unused_variables)]
impl<'a> TargetRuntime<'a> for SorobanTarget {
    fn get_storage_int(
        &self,
        binary: &Binary<'a>,
        function: FunctionValue,
        slot: PointerValue<'a>,
        ty: IntType<'a>,
    ) -> IntValue<'a> {
        todo!()
    }

    fn storage_load(
        &self,
        binary: &Binary<'a>,
        ty: &ast::Type,
        slot: &mut IntValue<'a>,
        function: FunctionValue<'a>,
        ns: &ast::Namespace,
        storage_type: &Option<StorageType>,
    ) -> BasicValueEnum<'a> {
        let storage_type = storage_type_to_int(storage_type);
        emit_context!(binary);
        let ret = call!(
            GET_CONTRACT_DATA,
            &[
                slot.as_basic_value_enum()
                    .into_int_value()
                    .const_cast(binary.context.i64_type(), false)
                    .into(),
                binary
                    .context
                    .i64_type()
                    .const_int(storage_type, false)
                    .into(),
            ]
        )
        .try_as_basic_value()
        .left()
        .unwrap()
        .into_int_value();

        ret.into()
    }

    /// Recursively store a type to storage
    fn storage_store(
        &self,
        binary: &Binary<'a>,
        ty: &ast::Type,
        existing: bool,
        slot: &mut IntValue<'a>,
        dest: BasicValueEnum<'a>,
        function: FunctionValue<'a>,
        ns: &ast::Namespace,
        storage_type: &Option<StorageType>,
    ) {
        emit_context!(binary);

        let storage_type = storage_type_to_int(storage_type);

        let function_value = binary.module.get_function(PUT_CONTRACT_DATA).unwrap();

        let value = binary
            .builder
            .build_call(
                function_value,
                &[
                    slot.as_basic_value_enum()
                        .into_int_value()
                        .const_cast(binary.context.i64_type(), false)
                        .into(),
                    dest.into(),
                    binary
                        .context
                        .i64_type()
                        .const_int(storage_type, false)
                        .into(),
                ],
                PUT_CONTRACT_DATA,
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
        ns: &Namespace,
    ) {
        unimplemented!()
    }

    // Bytes and string have special storage layout
    fn set_storage_string(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue<'a>,
        slot: PointerValue<'a>,
        dest: BasicValueEnum<'a>,
    ) {
        unimplemented!()
    }

    fn get_storage_string(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue,
        slot: PointerValue<'a>,
    ) -> PointerValue<'a> {
        unimplemented!()
    }

    fn set_storage_extfunc(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue,
        slot: PointerValue,
        dest: PointerValue,
        dest_ty: BasicTypeEnum,
    ) {
        unimplemented!()
    }

    fn get_storage_extfunc(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue,
        slot: PointerValue<'a>,
        ns: &Namespace,
    ) -> PointerValue<'a> {
        unimplemented!()
    }

    fn get_storage_bytes_subscript(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue,
        slot: IntValue<'a>,
        index: IntValue<'a>,
        loc: Loc,
        ns: &Namespace,
    ) -> IntValue<'a> {
        unimplemented!()
    }

    fn set_storage_bytes_subscript(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue,
        slot: IntValue<'a>,
        index: IntValue<'a>,
        value: IntValue<'a>,
        ns: &Namespace,
        loc: Loc,
    ) {
        unimplemented!()
    }

    fn storage_subscript(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue<'a>,
        ty: &Type,
        slot: IntValue<'a>,
        index: BasicValueEnum<'a>,
        ns: &Namespace,
    ) -> IntValue<'a> {
        unimplemented!()
    }

    fn storage_push(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue<'a>,
        ty: &Type,
        slot: IntValue<'a>,
        val: Option<BasicValueEnum<'a>>,
        ns: &Namespace,
    ) -> BasicValueEnum<'a> {
        unimplemented!()
    }

    fn storage_pop(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue<'a>,
        ty: &Type,
        slot: IntValue<'a>,
        load: bool,
        ns: &Namespace,
        loc: Loc,
    ) -> Option<BasicValueEnum<'a>> {
        unimplemented!()
    }

    fn storage_array_length(
        &self,
        _bin: &Binary<'a>,
        _function: FunctionValue,
        _slot: IntValue<'a>,
        _elem_ty: &Type,
        _ns: &Namespace,
    ) -> IntValue<'a> {
        unimplemented!()
    }

    /// keccak256 hash
    fn keccak256_hash(
        &self,
        bin: &Binary<'a>,
        src: PointerValue,
        length: IntValue,
        dest: PointerValue,
        ns: &Namespace,
    ) {
        unimplemented!()
    }

    /// Prints a string
    /// TODO: Implement this function, with a call to the `log` function in the Soroban runtime.
    fn print(&self, bin: &Binary, string: PointerValue, length: IntValue) {
        if string.is_const() && length.is_const() {
            let msg_pos = bin
                .builder
                .build_ptr_to_int(string, bin.context.i64_type(), "msg_pos")
                .unwrap();
            let msg_pos = msg_pos.const_cast(bin.context.i64_type(), false);

            let length = length.const_cast(bin.context.i64_type(), false);

            let eight = bin.context.i64_type().const_int(8, false);
            let four = bin.context.i64_type().const_int(4, false);
            let zero = bin.context.i64_type().const_int(0, false);
            let thirty_two = bin.context.i64_type().const_int(32, false);

            // XDR encode msg_pos and length
            let msg_pos_encoded = bin
                .builder
                .build_left_shift(msg_pos, thirty_two, "temp")
                .unwrap();
            let msg_pos_encoded = bin
                .builder
                .build_int_add(msg_pos_encoded, four, "msg_pos_encoded")
                .unwrap();

            let length_encoded = bin
                .builder
                .build_left_shift(length, thirty_two, "temp")
                .unwrap();
            let length_encoded = bin
                .builder
                .build_int_add(length_encoded, four, "length_encoded")
                .unwrap();

            let zero_encoded = bin.builder.build_left_shift(zero, eight, "temp").unwrap();

            let eight_encoded = bin.builder.build_left_shift(eight, eight, "temp").unwrap();
            let eight_encoded = bin
                .builder
                .build_int_add(eight_encoded, four, "eight_encoded")
                .unwrap();

            let call_res = bin
                .builder
                .build_call(
                    bin.module.get_function(LOG_FROM_LINEAR_MEMORY).unwrap(),
                    &[
                        msg_pos_encoded.into(),
                        length_encoded.into(),
                        msg_pos_encoded.into(),
                        four.into(),
                    ],
                    "log",
                )
                .unwrap();
        } else {
            todo!("Dynamic String printing is not yet supported")
        }
    }

    /// Return success without any result
    fn return_empty_abi(&self, bin: &Binary) {
        unimplemented!()
    }

    /// Return failure code
    fn return_code<'b>(&self, bin: &'b Binary, ret: IntValue<'b>) {
        unimplemented!()
    }

    /// Return failure without any result
    fn assert_failure(&self, bin: &Binary, data: PointerValue, length: IntValue) {
        bin.builder.build_unreachable().unwrap();
    }

    fn builtin_function(
        &self,
        binary: &Binary<'a>,
        function: FunctionValue<'a>,
        builtin_func: &Function,
        args: &[BasicMetadataValueEnum<'a>],
        first_arg_type: Option<BasicTypeEnum>,
        ns: &Namespace,
    ) -> Option<BasicValueEnum<'a>> {
        unimplemented!()
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
        ns: &Namespace,
        loc: Loc,
    ) {
        unimplemented!()
    }

    /// call external function
    fn external_call<'b>(
        &self,
        bin: &Binary<'b>,
        function: FunctionValue<'b>,
        success: Option<&mut BasicValueEnum<'b>>,
        payload: PointerValue<'b>,
        payload_len: IntValue<'b>,
        address: Option<PointerValue<'b>>,
        contract_args: ContractArgs<'b>,
        ty: CallTy,
        ns: &Namespace,
        loc: Loc,
    ) {
        unimplemented!()
    }

    /// send value to address
    fn value_transfer<'b>(
        &self,
        _bin: &Binary<'b>,
        _function: FunctionValue,
        _success: Option<&mut BasicValueEnum<'b>>,
        _address: PointerValue<'b>,
        _value: IntValue<'b>,
        _ns: &Namespace,
        loc: Loc,
    ) {
        unimplemented!()
    }

    /// builtin expressions
    fn builtin<'b>(
        &self,
        bin: &Binary<'b>,
        expr: &Expression,
        vartab: &HashMap<usize, Variable<'b>>,
        function: FunctionValue<'b>,
        ns: &Namespace,
    ) -> BasicValueEnum<'b> {
        unimplemented!()
    }

    /// Return the return data from an external call (either revert error or return values)
    fn return_data<'b>(&self, bin: &Binary<'b>, function: FunctionValue<'b>) -> PointerValue<'b> {
        unimplemented!()
    }

    /// Return the value we received
    fn value_transferred<'b>(&self, binary: &Binary<'b>, ns: &Namespace) -> IntValue<'b> {
        unimplemented!()
    }

    /// Terminate execution, destroy bin and send remaining funds to addr
    fn selfdestruct<'b>(&self, binary: &Binary<'b>, addr: ArrayValue<'b>, ns: &Namespace) {
        unimplemented!()
    }

    /// Crypto Hash
    fn hash<'b>(
        &self,
        bin: &Binary<'b>,
        function: FunctionValue<'b>,
        hash: HashTy,
        string: PointerValue<'b>,
        length: IntValue<'b>,
        ns: &Namespace,
    ) -> IntValue<'b> {
        unimplemented!()
    }

    /// Emit event
    fn emit_event<'b>(
        &self,
        bin: &Binary<'b>,
        function: FunctionValue<'b>,
        data: BasicValueEnum<'b>,
        topics: &[BasicValueEnum<'b>],
    ) {
        unimplemented!()
    }

    /// Return ABI encoded data
    fn return_abi_data<'b>(
        &self,
        binary: &Binary<'b>,
        data: PointerValue<'b>,
        data_len: BasicValueEnum<'b>,
    ) {
        unimplemented!()
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
