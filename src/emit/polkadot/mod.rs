// SPDX-License-Identifier: Apache-2.0

use std::ffi::CString;

use crate::codegen::{Options, STORAGE_INITIALIZER};
use crate::sema::ast::{Contract, Namespace};
use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::values::{BasicMetadataValueEnum, FunctionValue, IntValue, PointerValue};
use inkwell::AddressSpace;

use crate::codegen::dispatch::polkadot::DispatchType;
use crate::emit::functions::emit_functions;
use crate::emit::{Binary, TargetRuntime};

mod storage;
pub(super) mod target;

// When using the seal api, we use our own scratch buffer.
pub const SCRATCH_SIZE: u32 = 32 * 1024;

pub struct PolkadotTarget;

impl PolkadotTarget {
    pub fn build<'a>(
        context: &'a Context,
        std_lib: &Module<'a>,
        contract: &'a Contract,
        ns: &'a Namespace,
        opt: &'a Options,
    ) -> Binary<'a> {
        let filename = ns.files[contract.loc.file_no()].file_name();
        let mut binary = Binary::new(
            context,
            ns.target,
            &contract.id.name,
            filename.as_str(),
            opt,
            std_lib,
            None,
        );

        let ptr = binary.context.i8_type().ptr_type(AddressSpace::default());

        binary.vector_init_empty = binary
            .context
            .i32_type()
            .const_all_ones()
            .const_to_pointer(ptr);
        binary.set_early_value_aborts(contract, ns);

        let scratch_len = binary.module.add_global(
            context.i32_type(),
            Some(AddressSpace::default()),
            "scratch_len",
        );
        scratch_len.set_linkage(Linkage::Internal);
        scratch_len.set_initializer(&context.i32_type().get_undef());

        binary.scratch_len = Some(scratch_len);

        let scratch = binary.module.add_global(
            context.i8_type().array_type(SCRATCH_SIZE),
            Some(AddressSpace::default()),
            "scratch",
        );
        scratch.set_linkage(Linkage::Internal);
        scratch.set_initializer(&context.i8_type().array_type(SCRATCH_SIZE).get_undef());
        binary.scratch = Some(scratch);

        let mut target = PolkadotTarget;

        target.declare_externals(&binary);

        emit_functions(&mut target, &mut binary, contract, ns);

        let function_name = CString::new(STORAGE_INITIALIZER).unwrap();
        let mut storage_initializers = binary
            .functions
            .values()
            .filter(|f| f.get_name() == function_name.as_c_str());
        let storage_initializer = *storage_initializers
            .next()
            .expect("storage initializer is always present");
        assert!(storage_initializers.next().is_none());

        target.emit_dispatch(Some(storage_initializer), &mut binary, ns);
        target.emit_dispatch(None, &mut binary, ns);

        binary.internalize(&[
            "deploy",
            "call",
            "call_chain_extension",
            "input",
            "set_storage",
            "get_storage",
            "clear_storage",
            "hash_keccak_256",
            "hash_sha2_256",
            "hash_blake2_128",
            "hash_blake2_256",
            "seal_return",
            "debug_message",
            "instantiate",
            "seal_call",
            "delegate_call",
            "code_hash",
            "value_transferred",
            "minimum_balance",
            "weight_to_fee",
            "instantiation_nonce",
            "address",
            "balance",
            "block_number",
            "now",
            "gas_left",
            "caller",
            "terminate",
            "deposit_event",
            "transfer",
            "is_contract",
            "set_code_hash",
            "caller_is_root",
        ]);

        binary
    }

    fn public_function_prelude<'a>(
        &self,
        binary: &Binary<'a>,
        function: FunctionValue<'a>,
        storage_initializer: Option<FunctionValue>,
    ) -> (PointerValue<'a>, IntValue<'a>) {
        let entry = binary.context.append_basic_block(function, "entry");

        binary.builder.position_at_end(entry);

        // init our heap
        binary
            .builder
            .build_call(binary.module.get_function("__init_heap").unwrap(), &[], "")
            .unwrap();

        // Call the storage initializers on deploy
        if let Some(initializer) = storage_initializer {
            binary.builder.build_call(initializer, &[], "").unwrap();
        }

        let scratch_buf = binary.scratch.unwrap().as_pointer_value();
        let scratch_len = binary.scratch_len.unwrap().as_pointer_value();

        // copy arguments from input buffer
        binary
            .builder
            .build_store(
                scratch_len,
                binary
                    .context
                    .i32_type()
                    .const_int(SCRATCH_SIZE as u64, false),
            )
            .unwrap();

        binary
            .builder
            .build_call(
                binary.module.get_function("input").unwrap(),
                &[scratch_buf.into(), scratch_len.into()],
                "",
            )
            .unwrap();

        let args_length = binary
            .builder
            .build_load(binary.context.i32_type(), scratch_len, "input_len")
            .unwrap();

        // store the length in case someone wants it via msg.data
        binary
            .builder
            .build_store(
                binary.calldata_len.as_pointer_value(),
                args_length.into_int_value(),
            )
            .unwrap();

        (scratch_buf, args_length.into_int_value())
    }

    fn declare_externals(&self, binary: &Binary) {
        let ctx = binary.context;
        let u8_ptr = ctx.i8_type().ptr_type(AddressSpace::default()).into();
        let u32_val = ctx.i32_type().into();
        let u32_ptr = ctx.i32_type().ptr_type(AddressSpace::default()).into();
        let u64_val = ctx.i64_type().into();

        macro_rules! external {
            ($name:literal, $fn_type:ident, $( $args:expr ),*) => {
                binary.module.add_function(
                    $name,
                    ctx.$fn_type().fn_type(&[$($args),*], false),
                    Some(Linkage::External),
                );
            };
        }

        external!(
            "call_chain_extension",
            i32_type,
            u32_val,
            u8_ptr,
            u32_val,
            u8_ptr,
            u32_ptr
        );
        external!("input", void_type, u8_ptr, u32_ptr);
        external!("hash_keccak_256", void_type, u8_ptr, u32_val, u8_ptr);
        external!("hash_sha2_256", void_type, u8_ptr, u32_val, u8_ptr);
        external!("hash_blake2_128", void_type, u8_ptr, u32_val, u8_ptr);
        external!("hash_blake2_256", void_type, u8_ptr, u32_val, u8_ptr);
        external!("instantiation_nonce", i64_type,);
        external!("set_storage", i32_type, u8_ptr, u32_val, u8_ptr, u32_val);
        external!("debug_message", i32_type, u8_ptr, u32_val);
        external!("clear_storage", i32_type, u8_ptr, u32_val);
        external!("get_storage", i32_type, u8_ptr, u32_val, u8_ptr, u32_ptr);
        external!("seal_return", void_type, u32_val, u8_ptr, u32_val);
        external!(
            "instantiate",
            i32_type,
            u8_ptr,
            u64_val,
            u8_ptr,
            u8_ptr,
            u32_val,
            u8_ptr,
            u32_ptr,
            u8_ptr,
            u32_ptr,
            u8_ptr,
            u32_val
        );
        // We still use prefixed seal_call because it would collide with the exported call function.
        // TODO: Refactor emit to use a dedicated module for the externals to avoid any collisions.
        external!(
            "seal_call",
            i32_type,
            u32_val,
            u8_ptr,
            u64_val,
            u8_ptr,
            u8_ptr,
            u32_val,
            u8_ptr,
            u32_ptr
        );
        external!(
            "delegate_call",
            i32_type,
            u32_val,
            u8_ptr,
            u8_ptr,
            u32_val,
            u8_ptr,
            u32_ptr
        );
        external!("code_hash", i32_type, u8_ptr, u8_ptr, u32_ptr);
        external!("transfer", i32_type, u8_ptr, u32_val, u8_ptr, u32_val);
        external!("value_transferred", void_type, u8_ptr, u32_ptr);
        external!("address", void_type, u8_ptr, u32_ptr);
        external!("balance", void_type, u8_ptr, u32_ptr);
        external!("minimum_balance", void_type, u8_ptr, u32_ptr);
        external!("block_number", void_type, u8_ptr, u32_ptr);
        external!("now", void_type, u8_ptr, u32_ptr);
        external!("weight_to_fee", void_type, u64_val, u8_ptr, u32_ptr);
        external!("gas_left", void_type, u8_ptr, u32_ptr);
        external!("caller", void_type, u8_ptr, u32_ptr);
        external!("terminate", void_type, u8_ptr);
        external!("deposit_event", void_type, u8_ptr, u32_val, u8_ptr, u32_val);
        external!("is_contract", i32_type, u8_ptr);
        external!("set_code_hash", i32_type, u8_ptr);
        external!("caller_is_root", i32_type,);
    }

    /// Emits the "deploy" function if `storage_initializer` is `Some`, otherwise emits the "call" function.
    fn emit_dispatch(
        &mut self,
        storage_initializer: Option<FunctionValue>,
        bin: &mut Binary,
        ns: &Namespace,
    ) {
        let ty = bin.context.void_type().fn_type(&[], false);
        let export_name = if storage_initializer.is_some() {
            "deploy"
        } else {
            "call"
        };
        let func = bin.module.add_function(export_name, ty, None);
        let (input, input_length) = self.public_function_prelude(bin, func, storage_initializer);
        let args = vec![
            BasicMetadataValueEnum::PointerValue(input),
            BasicMetadataValueEnum::IntValue(input_length),
            BasicMetadataValueEnum::IntValue(self.value_transferred(bin, ns)),
            BasicMetadataValueEnum::PointerValue(bin.selector.as_pointer_value()),
        ];
        let dispatch_cfg_name = &storage_initializer
            .map(|_| DispatchType::Deploy)
            .unwrap_or(DispatchType::Call)
            .to_string();
        let cfg = bin.module.get_function(dispatch_cfg_name).unwrap();
        bin.builder
            .build_call(cfg, &args, dispatch_cfg_name)
            .unwrap();

        bin.builder.build_unreachable().unwrap();
    }
}
