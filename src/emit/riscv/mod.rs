// src/emit/riscv/mod.rs
use crate::codegen::targets::riscv::dispatch::DispatchType;
use crate::codegen::Options;
use crate::emit::binary::Binary;
use crate::emit::functions::emit_functions;
use crate::emit::riscv::target::RiscvTargetRuntime;
use crate::sema::ast::{Contract, Namespace};
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::AddressSpace;

pub(crate) mod codegen;
mod target;

pub struct RiscvTarget;

impl RiscvTarget {
    pub fn build<'a>(
        context: &'a Context,
        std_lib: &Module<'a>,
        contract: &'a Contract,
        ns: &'a Namespace,
        opt: &'a Options,
    ) -> Binary<'a> {
        let filename = ns.files[contract.loc.file_no()].file_name();
        let mut bin = Binary::new(
            context,
            ns,
            &contract.id.name,
            filename.as_str(),
            opt,
            std_lib,
            None,
        );

        // declare external syscall functions.
        target::declare_syscalls(&mut bin);

        // emit all contract functions and dispatch.
        emit_functions(&mut RiscvTargetRuntime, &mut bin, contract);

        Self::emit_entry(&mut bin);

        // `solang_dispatch` is the only symbol `_start` looks up.
        bin.internalize(&["solang_dispatch"]);

        bin
    }

    /// Emit `solang_dispatch`, the function the `_start` stub in the RISC-V
    /// stdlib calls with the calldata pointer and length.
    fn emit_entry(bin: &mut Binary) {
        let context = bin.context;
        let ptr_ty = context.ptr_type(AddressSpace::default());
        let i32_ty = context.i32_type();

        let func = bin.module.add_function(
            "solang_dispatch",
            context
                .void_type()
                .fn_type(&[ptr_ty.into(), i32_ty.into()], false),
            None,
        );

        let entry = context.append_basic_block(func, "entry");
        bin.builder.position_at_end(entry);

        // Nothing has initialized the heap at this point, and ABI
        // encoding/decoding allocates.
        let init_heap = bin.module.get_function("__init_heap").unwrap();
        bin.builder.build_call(init_heap, &[], "").unwrap();

        let dispatch = bin
            .module
            .get_function(&DispatchType::Call.to_string())
            .expect("call dispatcher is emitted for every contract");

        bin.builder
            .build_call(
                dispatch,
                &[
                    func.get_nth_param(0).unwrap().into(),
                    func.get_nth_param(1).unwrap().into(),
                ],
                "",
            )
            .unwrap();

        bin.builder.build_return(None).unwrap();
    }
}
