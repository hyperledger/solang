use crate::emit::binary::Binary;
use crate::emit::{ContractArgs, HashTy, TargetRuntime, Variable};
use crate::sema::ast::{CallTy, Type};
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Linkage;
use inkwell::types::{BasicTypeEnum, IntType};
use inkwell::values::{
    BasicMetadataValueEnum, BasicValue, BasicValueEnum, FunctionValue, IntValue, PointerValue,
};
use inkwell::AddressSpace;
use solang_parser::pt::Loc;
use solang_parser::pt::StorageType;
use std::collections::HashMap;

pub(crate) struct RiscvTargetRuntime;

impl RiscvTargetRuntime {
    /// Split an i256 into four i64 limbs, least-significant first.
    ///
    /// r55 reassembles the argument registers with `U256::from_limbs([a0, a1,
    /// a2, a3])`, and ruint orders limbs little-endian, so `a0` carries the
    /// low 64 bits. See `Syscall::SStore` in r55/src/exec.rs.
    fn split_256<'a>(
        value: IntValue<'a>,
        ctx: &'a Context,
        builder: &Builder<'a>,
    ) -> Vec<IntValue<'a>> {
        let i64_ty = ctx.i64_type();
        // The shift amount must have the same type as the value being
        // shifted, otherwise LLVM rejects the lshr.
        let value_ty = value.get_type();
        let mut parts = Vec::with_capacity(4);
        for i in 0..4u64 {
            let shifted = builder
                .build_right_shift(
                    value,
                    value_ty.const_int(64 * i, false),
                    false,
                    "split_shift",
                )
                .unwrap();
            let part = builder
                .build_int_truncate(shifted, i64_ty, "split_trunc")
                .unwrap();
            parts.push(part);
        }
        parts
    }

    /// Combine four i64 limbs, least-significant first, into an i256.
    fn combine_256<'a>(
        parts: &[IntValue<'a>],
        ctx: &'a Context,
        builder: &Builder<'a>,
    ) -> IntValue<'a> {
        let i256_ty = ctx.custom_width_int_type(256);
        let mut result = i256_ty.const_zero();
        for (i, part) in parts.iter().enumerate() {
            let extended = builder
                .build_int_z_extend(*part, i256_ty, "extend")
                .unwrap();
            let shifted = builder
                .build_left_shift(extended, i256_ty.const_int(64 * i as u64, false), "shift")
                .unwrap();
            result = builder.build_or(result, shifted, "combine").unwrap();
        }
        result
    }

    /// call a syscall that returns a struct of 4 i64 (like sload).
    fn call_syscall_4_4<'a>(
        bin: &Binary<'a>,
        func: FunctionValue<'a>,
        a0: IntValue<'a>,
        a1: IntValue<'a>,
        a2: IntValue<'a>,
        a3: IntValue<'a>,
    ) -> Vec<IntValue<'a>> {
        let i64_ty = bin.context.i64_type();
        let args = vec![a0, a1, a2, a3]
            .into_iter()
            .map(|v| {
                if v.get_type() != i64_ty {
                    v.const_cast(i64_ty, false)
                } else {
                    v
                }
            })
            .map(|v| v.as_basic_value_enum())
            .map(|v| v.into())
            .collect::<Vec<BasicMetadataValueEnum>>();

        // The callee writes the four result limbs through an out-pointer.
        let out_ty = i64_ty.array_type(4);
        let out = bin.builder.build_alloca(out_ty, "sload_out").unwrap();

        let mut args = args;
        args.push(out.into());
        let _ = bin.builder.build_call(func, &args, "");

        (0..4)
            .map(|i| {
                let slot = unsafe {
                    bin.builder
                        .build_gep(
                            i64_ty,
                            out,
                            &[i64_ty.const_int(i, false)],
                            &format!("sload_out{i}"),
                        )
                        .unwrap()
                };
                bin.builder
                    .build_load(i64_ty, slot, &format!("ret{i}"))
                    .unwrap()
                    .into_int_value()
            })
            .collect()
    }

    /// call a syscall that takes 8 i64 and returns nothing (like sstore).
    fn call_syscall_8_0<'a>(
        bin: &Binary<'a>,
        func: FunctionValue<'a>,
        k0: IntValue<'a>,
        k1: IntValue<'a>,
        k2: IntValue<'a>,
        k3: IntValue<'a>,
        v0: IntValue<'a>,
        v1: IntValue<'a>,
        v2: IntValue<'a>,
        v3: IntValue<'a>,
    ) {
        let i64_ty = bin.context.i64_type();
        let args = vec![k0, k1, k2, k3, v0, v1, v2, v3]
            .into_iter()
            .map(|v| {
                if v.get_type() != i64_ty {
                    v.const_cast(i64_ty, false)
                } else {
                    v
                }
            })
            .map(|v| v.as_basic_value_enum())
            .map(|v| v.into())
            .collect::<Vec<BasicMetadataValueEnum>>();
        let _ = bin.builder.build_call(func, &args, "syscall_sstore");
    }
}

/// declare external syscall functions in the module.
pub(crate) fn declare_syscalls(bin: &mut Binary) {
    let ctx = bin.context;
    let i64_ty = ctx.i64_type();
    let i8_ptr_ty = ctx.ptr_type(AddressSpace::default());
    let void_ty = ctx.void_type();

    // __sys_sload: (i64, i64, i64, i64, i64*) -> void
    let sload_ty = void_ty.fn_type(
        &[
            i64_ty.into(),
            i64_ty.into(),
            i64_ty.into(),
            i64_ty.into(),
            i8_ptr_ty.into(),
        ],
        false,
    );
    let sload = bin.module.add_function("__sys_sload", sload_ty, None);
    sload.set_linkage(Linkage::External);

    // __sys_sstore: (i64, i64, i64, i64, i64, i64, i64, i64) -> void
    let sstore_ty = void_ty.fn_type(
        &[
            i64_ty.into(),
            i64_ty.into(),
            i64_ty.into(),
            i64_ty.into(),
            i64_ty.into(),
            i64_ty.into(),
            i64_ty.into(),
            i64_ty.into(),
        ],
        false,
    );
    let sstore = bin.module.add_function("__sys_sstore", sstore_ty, None);
    sstore.set_linkage(Linkage::External);

    // __sys_return: (i8*, i64) -> void
    let return_ty = void_ty.fn_type(&[i8_ptr_ty.into(), i64_ty.into()], false);
    let return_fn = bin.module.add_function("__sys_return", return_ty, None);
    return_fn.set_linkage(Linkage::External);

    // __sys_caller: (i8*) -> void (writes 20 bytes)
    let caller_ty = void_ty.fn_type(&[i8_ptr_ty.into()], false);
    let caller = bin.module.add_function("__sys_caller", caller_ty, None);
    caller.set_linkage(Linkage::External);

    // __sys_callvalue: (i8*) -> void (writes 32 bytes)
    let callvalue_ty = void_ty.fn_type(&[i8_ptr_ty.into()], false);
    let callvalue = bin
        .module
        .add_function("__sys_callvalue", callvalue_ty, None);
    callvalue.set_linkage(Linkage::External);

    // __sys_revert: (i8*, i64) -> void
    let revert_ty = void_ty.fn_type(&[i8_ptr_ty.into(), i64_ty.into()], false);
    let revert = bin.module.add_function("__sys_revert", revert_ty, None);
    revert.set_linkage(Linkage::External);
}

impl<'a> TargetRuntime<'a> for RiscvTargetRuntime {
    fn get_storage_int(
        &self,
        bin: &Binary<'a>,
        _function: FunctionValue,
        slot: PointerValue<'a>,
        ty: IntType<'a>,
    ) -> IntValue<'a> {
        let i256_ty = bin.context.custom_width_int_type(256);
        let slot_val = bin
            .builder
            .build_load(i256_ty, slot, "slot_load")
            .unwrap()
            .into_int_value();
        let parts = RiscvTargetRuntime::split_256(slot_val, bin.context, &bin.builder);
        let sload_fn = bin.module.get_function("__sys_sload").unwrap();
        let results = RiscvTargetRuntime::call_syscall_4_4(
            bin, sload_fn, parts[0], parts[1], parts[2], parts[3],
        );
        let value_256 = RiscvTargetRuntime::combine_256(&results, bin.context, &bin.builder);
        bin.builder
            .build_int_truncate(value_256, ty, "storage_int")
            .unwrap()
    }

    fn storage_load(
        &self,
        bin: &Binary<'a>,
        ty: &Type,
        slot: &mut IntValue<'a>,
        _slot_ty: Option<&Type>,
        function: FunctionValue<'a>,
        _storage_type: &Option<StorageType>,
    ) -> BasicValueEnum<'a> {
        let i256_ty = bin.context.custom_width_int_type(256);
        let slot_ptr = bin.builder.build_alloca(i256_ty, "slot_ptr").unwrap();
        bin.builder.build_store(slot_ptr, *slot).unwrap();

        if let Type::Int(bits) | Type::Uint(bits) = ty {
            let int_ty = bin.context.custom_width_int_type((*bits) as u32);
            let val = self.get_storage_int(bin, function, slot_ptr, int_ty);
            return val.as_basic_value_enum();
        }
        // otherwise return the full 256-bit value.
        let val = self.get_storage_int(bin, function, slot_ptr, i256_ty);
        val.as_basic_value_enum()
    }

    fn storage_store(
        &self,
        bin: &Binary<'a>,
        _elem_ty: &Type,
        _existing: bool,
        slot: &mut IntValue<'a>,
        _slot_ty: Option<&Type>,
        dest: BasicValueEnum<'a>,
        _function: FunctionValue<'a>,
        _storage_type: &Option<StorageType>,
    ) {
        // extend dest to 256 bits.
        let i256_ty = bin.context.custom_width_int_type(256);
        let value = dest.into_int_value();
        let value_256 = if value.get_type() == i256_ty {
            value
        } else {
            bin.builder
                .build_int_z_extend(value, i256_ty, "extend_value")
                .unwrap()
        };

        let slot_ptr = bin.builder.build_alloca(i256_ty, "slot_ptr_store").unwrap();
        bin.builder.build_store(slot_ptr, *slot).unwrap();
        let slot_val = bin
            .builder
            .build_load(i256_ty, slot_ptr, "slot_load_store")
            .unwrap()
            .into_int_value();
        let slot_parts = RiscvTargetRuntime::split_256(slot_val, bin.context, &bin.builder);
        let value_parts = RiscvTargetRuntime::split_256(value_256, bin.context, &bin.builder);

        let sstore_fn = bin.module.get_function("__sys_sstore").unwrap();
        RiscvTargetRuntime::call_syscall_8_0(
            bin,
            sstore_fn,
            slot_parts[0],
            slot_parts[1],
            slot_parts[2],
            slot_parts[3],
            value_parts[0],
            value_parts[1],
            value_parts[2],
            value_parts[3],
        );
    }

    fn return_abi_data<'b>(
        &self,
        bin: &Binary<'b>,
        data: PointerValue<'b>,
        data_len: BasicValueEnum<'b>,
    ) {
        let len = data_len.into_int_value();
        let len_i64 = bin
            .builder
            .build_int_cast(len, bin.context.i64_type(), "len_cast")
            .unwrap();
        let return_fn = bin.module.get_function("__sys_return").unwrap();
        let args: Vec<BasicMetadataValueEnum> = vec![data.into(), len_i64.into()];
        let _ = bin.builder.build_call(return_fn, &args, "");
        // The Return syscall does not come back, but LLVM still needs the
        // block to be terminated.
        bin.builder.build_unreachable().unwrap();
    }

    fn value_transferred<'b>(&self, contract: &Binary<'b>) -> IntValue<'b> {
        let i256_ty = contract.context.custom_width_int_type(256);
        i256_ty.const_zero()
    }

    fn storage_delete(
        &self,
        _bin: &Binary<'a>,
        _ty: &Type,
        _slot: &mut IntValue<'a>,
        _function: FunctionValue<'a>,
    ) {
        unimplemented!("storage_delete")
    }

    fn set_storage_string(
        &self,
        _bin: &Binary<'a>,
        _function: FunctionValue<'a>,
        _slot: PointerValue<'a>,
        _dest: BasicValueEnum<'a>,
    ) {
        unimplemented!("set_storage_string")
    }

    fn get_storage_string(
        &self,
        _bin: &Binary<'a>,
        _function: FunctionValue,
        _slot: PointerValue<'a>,
    ) -> PointerValue<'a> {
        unimplemented!("get_storage_string")
    }

    fn set_storage_extfunc(
        &self,
        _bin: &Binary<'a>,
        _function: FunctionValue,
        _slot: PointerValue,
        _dest: PointerValue,
        _dest_ty: BasicTypeEnum,
    ) {
        unimplemented!("set_storage_extfunc")
    }

    fn get_storage_extfunc(
        &self,
        _bin: &Binary<'a>,
        _function: FunctionValue,
        _slot: PointerValue<'a>,
    ) -> PointerValue<'a> {
        unimplemented!("get_storage_extfunc")
    }

    fn get_storage_bytes_subscript(
        &self,
        _bin: &Binary<'a>,
        _function: FunctionValue,
        _slot: IntValue<'a>,
        _index: IntValue<'a>,
        _loc: Loc,
    ) -> IntValue<'a> {
        unimplemented!("get_storage_bytes_subscript")
    }

    fn set_storage_bytes_subscript(
        &self,
        _bin: &Binary<'a>,
        _function: FunctionValue,
        _slot: IntValue<'a>,
        _index: IntValue<'a>,
        _value: IntValue<'a>,
        _loc: Loc,
    ) {
        unimplemented!("set_storage_bytes_subscript")
    }

    fn storage_subscript(
        &self,
        _bin: &Binary<'a>,
        _function: FunctionValue<'a>,
        _ty: &Type,
        _slot: IntValue<'a>,
        _index: BasicValueEnum<'a>,
    ) -> IntValue<'a> {
        unimplemented!("storage_subscript")
    }

    fn storage_push(
        &self,
        _bin: &Binary<'a>,
        _function: FunctionValue<'a>,
        _ty: &Type,
        _slot: IntValue<'a>,
        _val: Option<BasicValueEnum<'a>>,
    ) -> BasicValueEnum<'a> {
        unimplemented!("storage_push")
    }

    fn storage_pop(
        &self,
        _bin: &Binary<'a>,
        _function: FunctionValue<'a>,
        _ty: &Type,
        _slot: IntValue<'a>,
        _load: bool,
        _loc: Loc,
    ) -> Option<BasicValueEnum<'a>> {
        unimplemented!("storage_pop")
    }

    fn storage_array_length(
        &self,
        _bin: &Binary<'a>,
        _function: FunctionValue,
        _slot: IntValue<'a>,
        _elem_ty: &Type,
    ) -> IntValue<'a> {
        unimplemented!("storage_array_length")
    }

    fn keccak256_hash(
        &self,
        _bin: &Binary<'a>,
        _src: PointerValue,
        _length: IntValue,
        _dest: PointerValue,
    ) {
        unimplemented!("keccak256_hash")
    }

    /// r55 has no debug output syscall, so runtime error messages are dropped.
    fn print<'b>(&self, _bin: &Binary<'b>, _string: PointerValue<'b>, _length: IntValue<'b>) {}

    fn return_empty_abi(&self, bin: &Binary) {
        let null = bin.context.ptr_type(AddressSpace::default()).const_null();
        let return_fn = bin.module.get_function("__sys_return").unwrap();
        let args: Vec<BasicMetadataValueEnum> =
            vec![null.into(), bin.context.i64_type().const_zero().into()];
        let _ = bin.builder.build_call(return_fn, &args, "");
        bin.builder.build_unreachable().unwrap();
    }

    fn return_code<'b>(&self, bin: &'b Binary, _ret: IntValue<'b>) {
        // r55 signals success/failure through the Return/Revert syscalls
        // rather than an exit code, so there is nothing to encode here.
        self.return_empty_abi(bin);
    }

    fn assert_failure(&self, bin: &Binary, data: PointerValue, length: IntValue) {
        let len_i64 = bin
            .builder
            .build_int_cast(length, bin.context.i64_type(), "revert_len")
            .unwrap();
        let revert_fn = bin.module.get_function("__sys_revert").unwrap();
        let args: Vec<BasicMetadataValueEnum> = vec![data.into(), len_i64.into()];
        let _ = bin.builder.build_call(revert_fn, &args, "");
        bin.builder.build_unreachable().unwrap();
    }

    fn builtin_function(
        &self,
        _bin: &Binary<'a>,
        _function: FunctionValue<'a>,
        _builtin_func: &crate::sema::ast::Function,
        _args: &[BasicMetadataValueEnum<'a>],
        _first_arg_type: Option<BasicTypeEnum>,
    ) -> Option<BasicValueEnum<'a>> {
        unimplemented!("builtin_function")
    }

    fn builtin<'b>(
        &self,
        _bin: &Binary<'b>,
        _expr: &crate::codegen::Expression,
        _vartab: &HashMap<usize, Variable<'b>>,
        _function: FunctionValue<'b>,
    ) -> BasicValueEnum<'b> {
        unimplemented!("builtin")
    }

    fn emit_event<'b>(
        &self,
        _bin: &Binary<'b>,
        _function: FunctionValue<'b>,
        _data: BasicValueEnum<'b>,
        _topics: &[BasicValueEnum<'b>],
    ) {
        unimplemented!("emit_event")
    }

    fn external_call<'b>(
        &self,
        _bin: &Binary<'b>,
        _function: FunctionValue<'b>,
        _success: Option<&mut BasicValueEnum<'b>>,
        _payload: PointerValue<'b>,
        _payload_len: IntValue<'b>,
        _address: Option<BasicValueEnum<'b>>,
        _contract_args: ContractArgs<'b>,
        _ty: CallTy,
        _loc: Loc,
    ) {
        unimplemented!("external_call")
    }

    fn create_contract<'b>(
        &mut self,
        _bin: &Binary<'b>,
        _function: FunctionValue<'b>,
        _success: Option<&mut BasicValueEnum<'b>>,
        _contract_no: usize,
        _address: PointerValue<'b>,
        _encoded_args: BasicValueEnum<'b>,
        _encoded_args_len: BasicValueEnum<'b>,
        _contract_args: ContractArgs<'b>,
        _loc: Loc,
    ) {
        unimplemented!("create_contract")
    }

    fn value_transfer<'b>(
        &self,
        _bin: &Binary<'b>,
        _function: FunctionValue,
        _success: Option<&mut BasicValueEnum<'b>>,
        _address: PointerValue<'b>,
        _value: IntValue<'b>,
        _loc: Loc,
    ) {
        unimplemented!("value_transfer")
    }

    fn return_data<'b>(&self, _bin: &Binary<'b>, _function: FunctionValue<'b>) -> PointerValue<'b> {
        unimplemented!("return_data")
    }

    fn selfdestruct<'b>(&self, _binary: &Binary<'b>, _addr: inkwell::values::ArrayValue<'b>) {
        unimplemented!("selfdestruct")
    }

    fn hash<'b>(
        &self,
        _bin: &Binary<'b>,
        _function: FunctionValue<'b>,
        _hash: HashTy,
        _string: PointerValue<'b>,
        _length: IntValue<'b>,
    ) -> IntValue<'b> {
        unimplemented!("hash")
    }
}
