// SPDX-License-Identifier: Apache-2.0

use crate::codegen::encoding::create_encoder;
use crate::codegen::revert::{error_msg_with_loc, PanicCode, SolidityError};
use crate::codegen::Expression;
use crate::sema::ast::{ArrayLength, Contract, Namespace, StructType, Type};
use std::cell::RefCell;
use std::path::Path;
use std::str;

use num_bigint::BigInt;
use num_traits::ToPrimitive;
use std::collections::HashMap;
#[cfg(feature = "wasm_opt")]
use tempfile::tempdir;
#[cfg(feature = "wasm_opt")]
use wasm_opt::OptimizationOptions;

use crate::codegen::{cfg::ReturnCode, Options};
use crate::emit::{polkadot, TargetRuntime};
use crate::emit::{solana, BinaryOp, Generate};
use crate::linker::link;
use crate::Target;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::debug_info::DICompileUnit;
use inkwell::debug_info::DebugInfoBuilder;
use inkwell::memory_buffer::MemoryBuffer;
use inkwell::module::{Linkage, Module};
use inkwell::passes::PassManager;
use inkwell::targets::{CodeModel, FileType, RelocMode};
use inkwell::types::{
    ArrayType, BasicMetadataTypeEnum, BasicType, BasicTypeEnum, FunctionType, IntType, StringRadix,
};
use inkwell::values::{BasicValueEnum, FunctionValue, GlobalValue, IntValue, PointerValue};
use inkwell::AddressSpace;
use inkwell::IntPredicate;
use inkwell::OptimizationLevel;
use once_cell::sync::OnceCell;
use solang_parser::pt;

#[cfg(feature = "soroban")]
use super::soroban;

static LLVM_INIT: OnceCell<()> = OnceCell::new();

#[macro_export]
macro_rules! emit_context {
    ($binary:expr) => {
        #[allow(unused_macros)]
        macro_rules! byte_ptr {
            () => {
                $binary.context.i8_type().ptr_type(AddressSpace::default())
            };
        }

        #[allow(unused_macros)]
        macro_rules! i32_const {
            ($val:expr) => {
                $binary.context.i32_type().const_int($val, false)
            };
        }

        #[allow(unused_macros)]
        macro_rules! i32_zero {
            () => {
                $binary.context.i32_type().const_zero()
            };
        }

        #[allow(unused_macros)]
        macro_rules! i64_const {
            ($val:expr) => {
                $binary.context.i64_type().const_int($val, false)
            };
        }

        #[allow(unused_macros)]
        macro_rules! i64_zero {
            () => {
                $binary.context.i64_type().const_zero()
            };
        }

        #[allow(unused_macros)]
        macro_rules! call {
            ($name:expr, $args:expr) => {
                $binary
                    .builder
                    .build_call($binary.module.get_function($name).unwrap(), $args, "")
                    .unwrap()
            };
            ($name:expr, $args:expr, $call_name:literal) => {
                $binary
                    .builder
                    .build_call(
                        $binary.module.get_function($name).unwrap(),
                        $args,
                        $call_name,
                    )
                    .unwrap()
            };
        }

        #[allow(unused_macros)]
        macro_rules! seal_get_storage {
            ($key_ptr:expr, $key_len:expr, $value_ptr:expr, $value_len:expr) => {
                call!("get_storage", &[$key_ptr, $key_len, $value_ptr, $value_len])
                    .try_as_basic_value()
                    .left()
                    .unwrap()
                    .into_int_value()
            };
        }

        #[allow(unused_macros)]
        macro_rules! seal_set_storage {
            ($key_ptr:expr, $key_len:expr, $value_ptr:expr, $value_len:expr) => {
                call!("set_storage", &[$key_ptr, $key_len, $value_ptr, $value_len])
                    .try_as_basic_value()
                    .left()
                    .unwrap()
                    .into_int_value()
            };
        }

        #[allow(unused_macros)]
        macro_rules! scratch_buf {
            () => {
                (
                    $binary.scratch.unwrap().as_pointer_value(),
                    $binary.scratch_len.unwrap().as_pointer_value(),
                )
            };
        }

        #[allow(unused_macros)]
        macro_rules! i8_basic_type_enum {
            () => {
                $binary.context.i8_type().as_basic_type_enum()
            };
        }
    };
}

pub struct Binary<'a> {
    pub name: String,
    pub module: Module<'a>,
    pub(crate) options: &'a Options,
    pub runtime: Option<Box<Binary<'a>>>,
    target: Target,
    pub(crate) function_abort_value_transfers: bool,
    pub(crate) constructor_abort_value_transfers: bool,
    pub builder: Builder<'a>,
    pub dibuilder: DebugInfoBuilder<'a>,
    pub compile_unit: DICompileUnit<'a>,
    pub(crate) context: &'a Context,
    pub(crate) functions: HashMap<usize, FunctionValue<'a>>,
    code: RefCell<Vec<u8>>,
    pub(crate) selector: GlobalValue<'a>,
    pub(crate) calldata_len: GlobalValue<'a>,
    pub(crate) scratch_len: Option<GlobalValue<'a>>,
    pub(crate) scratch: Option<GlobalValue<'a>>,
    pub(crate) parameters: Option<PointerValue<'a>>,
    pub(crate) return_values: HashMap<ReturnCode, IntValue<'a>>,
    /// No initializer for vector_new
    pub(crate) vector_init_empty: PointerValue<'a>,
    global_constant_strings: RefCell<HashMap<Vec<u8>, PointerValue<'a>>>,
}

impl<'a> Binary<'a> {
    /// Build the LLVM IR for a single contract
    pub fn build(
        context: &'a Context,
        contract: &'a Contract,
        ns: &'a Namespace,
        opt: &'a Options,
        _contract_no: usize,
    ) -> Self {
        let std_lib = load_stdlib(context, &ns.target);
        match ns.target {
            Target::Polkadot { .. } => {
                polkadot::PolkadotTarget::build(context, &std_lib, contract, ns, opt)
            }
            Target::Solana => solana::SolanaTarget::build(context, &std_lib, contract, ns, opt),
            #[cfg(feature = "soroban")]
            Target::Soroban => {
                soroban::SorobanTarget::build(context, &std_lib, contract, ns, opt, _contract_no)
            }
            _ => unimplemented!("target not implemented"),
        }
    }

    /// Compile the bin and return the code as bytes. The result is
    /// cached, since this function can be called multiple times (e.g. one for
    /// each time a bin of this type is created).
    /// Pass our module to llvm for optimization and compilation
    pub fn code(&self, generate: Generate) -> Result<Vec<u8>, String> {
        // return cached result if available
        if !self.code.borrow().is_empty() {
            return Ok(self.code.borrow().clone());
        }

        match self.options.opt_level.into() {
            OptimizationLevel::Default | OptimizationLevel::Aggressive => {
                let pass_manager = PassManager::create(());

                pass_manager.add_promote_memory_to_register_pass();
                pass_manager.add_function_inlining_pass();
                pass_manager.add_global_dce_pass();
                pass_manager.add_constant_merge_pass();

                pass_manager.run_on(&self.module);
            }
            _ => {}
        }

        let target = inkwell::targets::Target::from_name(self.target.llvm_target_name()).unwrap();

        let target_machine = target
            .create_target_machine(
                &self.target.llvm_target_triple(),
                "",
                self.target.llvm_features(),
                self.options.opt_level.into(),
                RelocMode::Default,
                CodeModel::Default,
            )
            .unwrap();

        let code = target_machine
            .write_to_memory_buffer(
                &self.module,
                if generate == Generate::Assembly {
                    FileType::Assembly
                } else {
                    FileType::Object
                },
            )
            .map(|out| {
                let slice = out.as_slice();

                if generate == Generate::Linked {
                    link(slice, &self.name, self.target).to_vec()
                } else {
                    slice.to_vec()
                }
            })
            .map_err(|s| s.to_string())?;

        #[cfg(feature = "wasm_opt")]
        if let Some(level) = self.options.wasm_opt.filter(|_| self.target.is_polkadot()) {
            let mut infile = tempdir().map_err(|e| e.to_string())?.into_path();
            infile.push("code.wasm");
            let outfile = infile.with_extension("wasmopt");
            std::fs::write(&infile, &code).map_err(|e| e.to_string())?;

            // Using the same config as cargo contract:
            // https://github.com/paritytech/cargo-contract/blob/71a8a42096e2df36d54a695d099aecfb1e394b78/crates/build/src/wasm_opt.rs#L67
            OptimizationOptions::from(level)
                .mvp_features_only()
                .zero_filled_memory(true)
                .debug_info(self.options.generate_debug_information)
                .run(&infile, &outfile)
                .map_err(|err| format!("wasm-opt for binary {} failed: {}", self.name, err))?;

            return std::fs::read(&outfile).map_err(|e| e.to_string());
        }

        Ok(code)
    }

    /// Mark all functions as internal unless they're in the export_list. This helps the
    /// llvm globaldce pass eliminate unnecessary functions and reduce the wasm output.
    pub(crate) fn internalize(&self, export_list: &[&str]) {
        let mut func = self.module.get_first_function();

        // FIXME: these functions are called from code generated by lowering into wasm,
        // so eliminating them now will cause link errors. Either we should prevent these
        // calls from being done in the first place or do dce at link time
        let mut export_list = export_list.to_vec();
        export_list.push("__ashlti3");
        export_list.push("__lshrti3");
        export_list.push("__ashrti3");

        while let Some(f) = func {
            let name = f.get_name().to_str().unwrap();

            if !name.starts_with("llvm.")
                && export_list.iter().all(|e| {
                    // symbols may get renamed foo.1 or foo.2, also export those
                    if let Some(tail) = name.strip_prefix(e) {
                        if let Some(no) = tail.strip_prefix('.') {
                            no.parse::<u32>().is_ok()
                        } else {
                            tail.is_empty()
                        }
                    } else {
                        false
                    }
                })
            {
                f.set_linkage(Linkage::Internal);
            }

            func = f.get_next_function();
        }
    }

    pub fn bitcode(&self, path: &Path) {
        self.module.write_bitcode_to_path(path);
    }

    pub fn dump_llvm(&self, path: &Path) -> Result<(), String> {
        if let Err(s) = self.module.print_to_file(path) {
            return Err(s.to_string());
        }

        Ok(())
    }

    pub fn new(
        context: &'a Context,
        target: Target,
        name: &str,
        filename: &str,
        opt: &'a Options,
        std_lib: &Module<'a>,
        runtime: Option<Box<Binary<'a>>>,
    ) -> Self {
        LLVM_INIT.get_or_init(|| {
            inkwell::targets::Target::initialize_webassembly(&Default::default());

            extern "C" {
                fn LLVMInitializeSBFTarget();
                fn LLVMInitializeSBFTargetInfo();
                fn LLVMInitializeSBFAsmPrinter();
                fn LLVMInitializeSBFDisassembler();
                fn LLVMInitializeSBFTargetMC();
            }

            unsafe {
                LLVMInitializeSBFTarget();
                LLVMInitializeSBFTargetInfo();
                LLVMInitializeSBFAsmPrinter();
                LLVMInitializeSBFDisassembler();
                LLVMInitializeSBFTargetMC();
            }
        });

        let triple = target.llvm_target_triple();
        let module = context.create_module(name);

        let debug_metadata_version = context.i32_type().const_int(3, false);
        module.add_basic_value_flag(
            "Debug Info Version",
            inkwell::module::FlagBehavior::Warning,
            debug_metadata_version,
        );

        let builder = context.create_builder();
        let (dibuilder, compile_unit) = module.create_debug_info_builder(
            true,
            inkwell::debug_info::DWARFSourceLanguage::C,
            filename,
            ".",
            "Solang",
            false,
            "",
            0,
            "",
            inkwell::debug_info::DWARFEmissionKind::Full,
            0,
            false,
            false,
            "",
            "",
        );

        module.set_triple(&triple);
        module.set_source_file_name(filename);

        module.link_in_module(std_lib.clone()).unwrap();

        let selector = module.add_global(
            context.i32_type(),
            Some(AddressSpace::default()),
            "selector",
        );
        selector.set_linkage(Linkage::Internal);
        selector.set_initializer(&context.i32_type().const_zero());

        let calldata_len = module.add_global(
            context.i32_type(),
            Some(AddressSpace::default()),
            "calldata_len",
        );
        calldata_len.set_linkage(Linkage::Internal);
        calldata_len.set_initializer(&context.i32_type().const_zero());

        let mut return_values = HashMap::new();

        return_values.insert(ReturnCode::Success, context.i32_type().const_zero());
        return_values.insert(
            ReturnCode::FunctionSelectorInvalid,
            context.i32_type().const_int(3, false),
        );
        return_values.insert(
            ReturnCode::AbiEncodingInvalid,
            context.i32_type().const_int(2, false),
        );

        Binary {
            name: name.to_owned(),
            module,
            runtime,
            function_abort_value_transfers: false,
            constructor_abort_value_transfers: false,
            builder,
            dibuilder,
            compile_unit,
            context,
            target,
            functions: HashMap::new(),
            code: RefCell::new(Vec::new()),
            options: opt,
            selector,
            calldata_len,
            scratch: None,
            scratch_len: None,
            parameters: None,
            return_values,
            vector_init_empty: context
                .i8_type()
                .ptr_type(AddressSpace::default())
                .const_null(),
            global_constant_strings: RefCell::new(HashMap::new()),
        }
    }

    /// Set flags for early aborts if a value transfer is done and no function/constructor can handle it
    pub fn set_early_value_aborts(&mut self, contract: &Contract, ns: &Namespace) {
        // if there is no payable function, fallback or receive then abort all value transfers at the top
        // note that receive() is always payable so this just checkes for presence.
        self.function_abort_value_transfers = !contract.functions.iter().any(|function_no| {
            let f = &ns.functions[*function_no];
            !f.is_constructor() && f.is_payable()
        });

        self.constructor_abort_value_transfers = !contract.functions.iter().any(|function_no| {
            let f = &ns.functions[*function_no];
            f.is_constructor() && f.is_payable()
        });
    }

    /// llvm value type, as in chain currency (usually 128 bits int)
    pub(crate) fn value_type(&self, ns: &Namespace) -> IntType<'a> {
        self.context
            .custom_width_int_type(ns.value_length as u32 * 8)
    }

    /// llvm address type
    pub(crate) fn address_type(&self, ns: &Namespace) -> ArrayType<'a> {
        self.context.i8_type().array_type(ns.address_length as u32)
    }

    /// Creates global string in the llvm module with initializer
    ///
    pub(crate) fn emit_global_string(
        &self,
        name: &str,
        data: &[u8],
        constant: bool,
    ) -> PointerValue<'a> {
        if let Some(emitted_string) = self.global_constant_strings.borrow().get(data) {
            if constant {
                return *emitted_string;
            }
        }
        let ty = self.context.i8_type().array_type(data.len() as u32);

        let gv = self
            .module
            .add_global(ty, Some(AddressSpace::default()), name);

        gv.set_linkage(Linkage::Internal);

        gv.set_initializer(&self.context.const_string(data, false));

        if constant {
            gv.set_constant(true);
            gv.set_unnamed_addr(true);
            let ptr_val = gv.as_pointer_value();
            self.global_constant_strings
                .borrow_mut()
                .insert(data.to_vec(), ptr_val);
            ptr_val
        } else {
            gv.as_pointer_value()
        }
    }

    /// Wrapper for alloca. Ensures that the alloca is done on the first basic block.
    /// If alloca is not on the first basic block, llvm will get to llvm_unreachable
    /// for the BPF target.
    pub(crate) fn build_alloca<T: BasicType<'a>>(
        &self,
        function: inkwell::values::FunctionValue<'a>,
        ty: T,
        name: &str,
    ) -> PointerValue<'a> {
        let entry = function
            .get_first_basic_block()
            .expect("function missing entry block");
        let current = self.builder.get_insert_block().unwrap();

        if let Some(instr) = &entry.get_first_instruction() {
            self.builder.position_before(instr);
        } else {
            // if there is no instruction yet, then nothing was built
            self.builder.position_at_end(entry);
        }

        let res = self.builder.build_alloca(ty, name).unwrap();

        self.builder.position_at_end(current);

        res
    }

    pub(crate) fn build_array_alloca<T: BasicType<'a>>(
        &self,
        function: inkwell::values::FunctionValue<'a>,
        ty: T,
        length: IntValue<'a>,
        name: &str,
    ) -> PointerValue<'a> {
        let entry = function
            .get_first_basic_block()
            .expect("function missing entry block");
        let current = self.builder.get_insert_block().unwrap();

        if let Some(instr) = entry.get_first_instruction() {
            self.builder.position_before(&instr);
        } else {
            self.builder.position_at_end(entry);
        }

        let res = self.builder.build_array_alloca(ty, length, name).unwrap();

        self.builder.position_at_end(current);

        res
    }

    /// Emit a loop from `from` to `to`. The closure exists to insert the body of the loop; the closure
    /// gets the loop variable passed to it as an IntValue, and a userdata PointerValue
    pub fn emit_static_loop_with_pointer<F>(
        &self,
        function: FunctionValue,
        from: IntValue<'a>,
        to: IntValue<'a>,
        data_ref: &mut PointerValue<'a>,
        mut insert_body: F,
    ) where
        F: FnMut(IntValue<'a>, &mut PointerValue<'a>),
    {
        let body = self.context.append_basic_block(function, "body");
        let done = self.context.append_basic_block(function, "done");
        let entry = self.builder.get_insert_block().unwrap();

        self.builder.build_unconditional_branch(body).unwrap();
        self.builder.position_at_end(body);

        let loop_ty = from.get_type();
        let loop_phi = self.builder.build_phi(loop_ty, "index").unwrap();
        let data_phi = self.builder.build_phi(data_ref.get_type(), "data").unwrap();
        let mut data = data_phi.as_basic_value().into_pointer_value();

        let loop_var = loop_phi.as_basic_value().into_int_value();

        // add loop body
        insert_body(loop_var, &mut data);

        let next = self
            .builder
            .build_int_add(loop_var, loop_ty.const_int(1, false), "next_index")
            .unwrap();

        let comp = self
            .builder
            .build_int_compare(IntPredicate::ULT, next, to, "loop_cond")
            .unwrap();
        self.builder
            .build_conditional_branch(comp, body, done)
            .unwrap();

        let body = self.builder.get_insert_block().unwrap();
        loop_phi.add_incoming(&[(&from, entry), (&next, body)]);
        data_phi.add_incoming(&[(&*data_ref, entry), (&data, body)]);

        self.builder.position_at_end(done);

        *data_ref = data;
    }

    /// Emit a loop from `from` to `to`. The closure exists to insert the body of the loop; the closure
    /// gets the loop variable passed to it as an IntValue, and a userdata IntValue
    pub fn emit_static_loop_with_int<F>(
        &self,
        function: FunctionValue,
        from: IntValue<'a>,
        to: IntValue<'a>,
        data_ref: &mut IntValue<'a>,
        mut insert_body: F,
    ) where
        F: FnMut(IntValue<'a>, &mut IntValue<'a>),
    {
        let body = self.context.append_basic_block(function, "body");
        let done = self.context.append_basic_block(function, "done");
        let entry = self.builder.get_insert_block().unwrap();

        self.builder.build_unconditional_branch(body).unwrap();
        self.builder.position_at_end(body);

        let loop_ty = from.get_type();
        let loop_phi = self.builder.build_phi(loop_ty, "index").unwrap();
        let data_phi = self.builder.build_phi(data_ref.get_type(), "data").unwrap();
        let mut data = data_phi.as_basic_value().into_int_value();

        let loop_var = loop_phi.as_basic_value().into_int_value();

        // add loop body
        insert_body(loop_var, &mut data);

        let next = self
            .builder
            .build_int_add(loop_var, loop_ty.const_int(1, false), "next_index")
            .unwrap();

        let comp = self
            .builder
            .build_int_compare(IntPredicate::ULT, next, to, "loop_cond")
            .unwrap();
        self.builder
            .build_conditional_branch(comp, body, done)
            .unwrap();

        let body = self.builder.get_insert_block().unwrap();
        loop_phi.add_incoming(&[(&from, entry), (&next, body)]);
        data_phi.add_incoming(&[(&*data_ref, entry), (&data, body)]);

        self.builder.position_at_end(done);

        *data_ref = data;
    }

    /// Emit a loop from `from` to `to`, checking the condition _before_ the body.
    pub fn emit_loop_cond_first_with_int<F>(
        &self,
        function: FunctionValue,
        from: IntValue<'a>,
        to: IntValue<'a>,
        data_ref: &mut IntValue<'a>,
        mut insert_body: F,
    ) where
        F: FnMut(IntValue<'a>, &mut IntValue<'a>),
    {
        let cond = self.context.append_basic_block(function, "cond");
        let body = self.context.append_basic_block(function, "body");
        let done = self.context.append_basic_block(function, "done");
        let entry = self.builder.get_insert_block().unwrap();

        self.builder.build_unconditional_branch(cond).unwrap();
        self.builder.position_at_end(cond);

        let loop_ty = from.get_type();
        let loop_phi = self.builder.build_phi(loop_ty, "index").unwrap();
        let data_phi = self.builder.build_phi(data_ref.get_type(), "data").unwrap();
        let mut data = data_phi.as_basic_value().into_int_value();

        let loop_var = loop_phi.as_basic_value().into_int_value();

        let next = self
            .builder
            .build_int_add(loop_var, loop_ty.const_int(1, false), "next_index")
            .unwrap();

        let comp = self
            .builder
            .build_int_compare(IntPredicate::ULT, loop_var, to, "loop_cond")
            .unwrap();
        self.builder
            .build_conditional_branch(comp, body, done)
            .unwrap();

        self.builder.position_at_end(body);
        // add loop body
        insert_body(loop_var, &mut data);

        let body = self.builder.get_insert_block().unwrap();

        loop_phi.add_incoming(&[(&from, entry), (&next, body)]);
        data_phi.add_incoming(&[(&*data_ref, entry), (&data, body)]);

        self.builder.build_unconditional_branch(cond).unwrap();

        self.builder.position_at_end(done);

        *data_ref = data_phi.as_basic_value().into_int_value();
    }

    /// Emit a loop from `from` to `to`, checking the condition _before_ the body.
    pub fn emit_loop_cond_first_with_pointer<F>(
        &self,
        function: FunctionValue,
        from: IntValue<'a>,
        to: IntValue<'a>,
        data_ref: &mut PointerValue<'a>,
        mut insert_body: F,
    ) where
        F: FnMut(IntValue<'a>, &mut PointerValue<'a>),
    {
        let cond = self.context.append_basic_block(function, "cond");
        let body = self.context.append_basic_block(function, "body");
        let done = self.context.append_basic_block(function, "done");
        let entry = self.builder.get_insert_block().unwrap();

        self.builder.build_unconditional_branch(cond).unwrap();
        self.builder.position_at_end(cond);

        let loop_ty = from.get_type();
        let loop_phi = self.builder.build_phi(loop_ty, "index").unwrap();
        let data_phi = self.builder.build_phi(data_ref.get_type(), "data").unwrap();
        let mut data = data_phi.as_basic_value().into_pointer_value();

        let loop_var = loop_phi.as_basic_value().into_int_value();

        let next = self
            .builder
            .build_int_add(loop_var, loop_ty.const_int(1, false), "next_index")
            .unwrap();

        let comp = self
            .builder
            .build_int_compare(IntPredicate::ULT, loop_var, to, "loop_cond")
            .unwrap();
        self.builder
            .build_conditional_branch(comp, body, done)
            .unwrap();

        self.builder.position_at_end(body);
        // add loop body
        insert_body(loop_var, &mut data);

        let body = self.builder.get_insert_block().unwrap();

        loop_phi.add_incoming(&[(&from, entry), (&next, body)]);
        data_phi.add_incoming(&[(&*data_ref, entry), (&data, body)]);

        self.builder.build_unconditional_branch(cond).unwrap();

        self.builder.position_at_end(done);

        *data_ref = data_phi.as_basic_value().into_pointer_value();
    }

    /// Convert a BigInt number to llvm const value
    pub(crate) fn number_literal(&self, bits: u32, n: &BigInt, _ns: &Namespace) -> IntValue<'a> {
        let ty = self.context.custom_width_int_type(bits);
        let s = n.to_string();

        ty.const_int_from_string(&s, StringRadix::Decimal).unwrap()
    }

    /// Emit function prototype
    pub(crate) fn function_type(
        &self,
        params: &[Type],
        returns: &[Type],
        ns: &Namespace,
    ) -> FunctionType<'a> {
        // function parameters
        let mut args = params
            .iter()
            .map(|ty| self.llvm_var_ty(ty, ns).into())
            .collect::<Vec<BasicMetadataTypeEnum>>();

        if ns.target == Target::Soroban {
            match returns.iter().next() {
                Some(ret) => return self.llvm_type(ret, ns).fn_type(&args, false),
                None => return self.context.void_type().fn_type(&args, false),
            }
        }
        // add return values
        for ty in returns {
            args.push(if ty.is_reference_type(ns) && !ty.is_contract_storage() {
                self.llvm_type(ty, ns)
                    .ptr_type(AddressSpace::default())
                    .ptr_type(AddressSpace::default())
                    .into()
            } else {
                self.llvm_type(ty, ns)
                    .ptr_type(AddressSpace::default())
                    .into()
            });
        }

        // On Solana, we need to pass around the accounts
        if ns.target == Target::Solana {
            args.push(
                self.module
                    .get_struct_type("struct.SolParameters")
                    .unwrap()
                    .ptr_type(AddressSpace::default())
                    .into(),
            );
        }

        // Solana return type should be 64 bit, 32 bit on wasm
        self.return_values[&ReturnCode::Success]
            .get_type()
            .fn_type(&args, false)
    }

    // Create the llvm intrinsic for counting leading zeros
    pub fn llvm_ctlz(&self, bit: u32) -> FunctionValue<'a> {
        let name = format!("llvm.ctlz.i{bit}");
        let ty = self.context.custom_width_int_type(bit);

        if let Some(f) = self.module.get_function(&name) {
            return f;
        }

        self.module.add_function(
            &name,
            ty.fn_type(&[ty.into(), self.context.bool_type().into()], false),
            None,
        )
    }

    // Create the llvm intrinsic for bswap
    pub fn llvm_bswap(&self, bit: u32) -> FunctionValue<'a> {
        let name = format!("llvm.bswap.i{bit}");
        let ty = self.context.custom_width_int_type(bit);

        if let Some(f) = self.module.get_function(&name) {
            return f;
        }

        self.module
            .add_function(&name, ty.fn_type(&[ty.into()], false), None)
    }

    // Create the llvm intrinsic for overflows
    pub fn llvm_overflow(
        &self,
        ret_ty: BasicTypeEnum<'a>,
        ty: IntType<'a>,
        signed: bool,
        op: BinaryOp,
    ) -> FunctionValue<'a> {
        let bit = ty.get_bit_width();
        let name = format!(
            "llvm.{}{}.with.overflow.i{}",
            if signed { "s" } else { "u" },
            op,
            bit,
        );

        if let Some(f) = self.module.get_function(&name) {
            return f;
        }

        self.module
            .add_function(&name, ret_ty.fn_type(&[ty.into(), ty.into()], false), None)
    }

    /// Return the llvm type for a variable holding the type, not the type itself
    pub(crate) fn llvm_var_ty(&self, ty: &Type, ns: &Namespace) -> BasicTypeEnum<'a> {
        let llvm_ty = self.llvm_type(ty, ns);
        match ty.deref_memory() {
            Type::Struct(_)
            | Type::Array(..)
            | Type::DynamicBytes
            | Type::String
            | Type::ExternalFunction { .. } => llvm_ty
                .ptr_type(AddressSpace::default())
                .as_basic_type_enum(),
            _ => llvm_ty,
        }
    }

    /// Return the llvm type for field in struct or array
    pub(crate) fn llvm_field_ty(&self, ty: &Type, ns: &Namespace) -> BasicTypeEnum<'a> {
        let llvm_ty = self.llvm_type(ty, ns);
        match ty.deref_memory() {
            Type::Array(_, dim) if dim.last() == Some(&ArrayLength::Dynamic) => llvm_ty
                .ptr_type(AddressSpace::default())
                .as_basic_type_enum(),
            Type::DynamicBytes | Type::String => llvm_ty
                .ptr_type(AddressSpace::default())
                .as_basic_type_enum(),
            _ => llvm_ty,
        }
    }

    /// Return the llvm type for the resolved type.
    pub(crate) fn llvm_type(&self, ty: &Type, ns: &Namespace) -> BasicTypeEnum<'a> {
        emit_context!(self);
        if ty.is_builtin_struct() == Some(StructType::AccountInfo) {
            return self
                .context
                .struct_type(
                    &[
                        byte_ptr!().as_basic_type_enum(),             // SolPubkey *
                        byte_ptr!().as_basic_type_enum(),             // uint64_t *
                        self.context.i64_type().as_basic_type_enum(), // uint64_t
                        byte_ptr!().as_basic_type_enum(),             // uint8_t *
                        byte_ptr!().as_basic_type_enum(),             // SolPubkey *
                        self.context.i64_type().as_basic_type_enum(), // uint64_t
                        i8_basic_type_enum!(),                        // bool
                        i8_basic_type_enum!(),                        // bool
                        i8_basic_type_enum!(),                        // bool
                    ],
                    false,
                )
                .as_basic_type_enum();
        } else {
            match ty {
                Type::Bool => BasicTypeEnum::IntType(self.context.bool_type()),
                Type::Int(n) | Type::Uint(n) => {
                    BasicTypeEnum::IntType(self.context.custom_width_int_type(*n as u32))
                }
                Type::Value => BasicTypeEnum::IntType(
                    self.context
                        .custom_width_int_type(ns.value_length as u32 * 8),
                ),
                Type::Contract(_) | Type::Address(_) => {
                    BasicTypeEnum::ArrayType(self.address_type(ns))
                }
                Type::Bytes(n) => {
                    BasicTypeEnum::IntType(self.context.custom_width_int_type(*n as u32 * 8))
                }
                Type::Enum(n) => self.llvm_type(&ns.enums[*n].ty, ns),
                Type::String | Type::DynamicBytes => {
                    self.module.get_struct_type("struct.vector").unwrap().into()
                }
                Type::Array(base_ty, dims) => {
                    dims.iter()
                        .fold(self.llvm_field_ty(base_ty, ns), |aty, dim| match dim {
                            ArrayLength::Fixed(d) => aty.array_type(d.to_u32().unwrap()).into(),
                            ArrayLength::Dynamic => {
                                self.module.get_struct_type("struct.vector").unwrap().into()
                            }
                            ArrayLength::AnyFixed => unreachable!(),
                        })
                }
                Type::Struct(StructType::SolParameters) => self
                    .module
                    .get_struct_type("struct.SolParameters")
                    .unwrap()
                    .as_basic_type_enum(),
                Type::Struct(str_ty) => self
                    .context
                    .struct_type(
                        &str_ty
                            .definition(ns)
                            .fields
                            .iter()
                            .map(|f| self.llvm_field_ty(&f.ty, ns))
                            .collect::<Vec<BasicTypeEnum>>(),
                        false,
                    )
                    .as_basic_type_enum(),
                Type::Mapping(..) => self.llvm_type(&ns.storage_type(), ns),
                Type::Ref(r) => self
                    .llvm_type(r, ns)
                    .ptr_type(AddressSpace::default())
                    .as_basic_type_enum(),
                Type::StorageRef(..) => self.llvm_type(&ns.storage_type(), ns),
                Type::InternalFunction {
                    params, returns, ..
                } => {
                    let ftype = self.function_type(params, returns, ns);

                    BasicTypeEnum::PointerType(ftype.ptr_type(AddressSpace::default()))
                }
                Type::ExternalFunction { .. } => {
                    let address = self.llvm_type(&Type::Address(false), ns);
                    let selector = self.llvm_type(&Type::FunctionSelector, ns);
                    self.context
                        .struct_type(&[selector, address], false)
                        .as_basic_type_enum()
                }
                Type::Slice(ty) => BasicTypeEnum::StructType(
                    self.context.struct_type(
                        &[
                            self.llvm_type(ty, ns)
                                .ptr_type(AddressSpace::default())
                                .into(),
                            self.context
                                .custom_width_int_type(ns.target.ptr_size().into())
                                .into(),
                        ],
                        false,
                    ),
                ),
                Type::UserType(no) => self.llvm_type(&ns.user_types[*no].ty, ns),
                Type::BufferPointer => self
                    .context
                    .i8_type()
                    .ptr_type(AddressSpace::default())
                    .as_basic_type_enum(),
                Type::FunctionSelector => {
                    self.llvm_type(&Type::Bytes(ns.target.selector_length()), ns)
                }
                // Soroban functions always return a 64 bit value.
                Type::Void => BasicTypeEnum::IntType(self.context.i64_type()),
                _ => unreachable!(),
            }
        }
    }

    /// Allocate vector
    pub(crate) fn vector_new(
        &self,
        size: IntValue<'a>,
        elem_size: IntValue<'a>,
        init: Option<&Vec<u8>>,
    ) -> PointerValue<'a> {
        if let Some(init) = init {
            if init.is_empty() {
                return self
                    .module
                    .get_struct_type("struct.vector")
                    .unwrap()
                    .ptr_type(AddressSpace::default())
                    .const_null();
            }
        }

        let init = match init {
            None => self.vector_init_empty,
            Some(s) => self.emit_global_string("const_string", s, true),
        };

        self.builder
            .build_call(
                self.module.get_function("vector_new").unwrap(),
                &[size.into(), elem_size.into(), init.into()],
                "",
            )
            .unwrap()
            .try_as_basic_value()
            .left()
            .unwrap()
            .into_pointer_value()
    }

    /// Number of element in a vector
    pub(crate) fn vector_len(&self, vector: BasicValueEnum<'a>) -> IntValue<'a> {
        if vector.is_struct_value() {
            // slice
            let slice = vector.into_struct_value();

            self.builder
                .build_int_truncate(
                    self.builder
                        .build_extract_value(slice, 1, "slice_len")
                        .unwrap()
                        .into_int_value(),
                    self.context.i32_type(),
                    "len",
                )
                .unwrap()
        } else {
            // field 0 is the length
            let vector = vector.into_pointer_value();
            let vector_type = self.module.get_struct_type("struct.vector").unwrap();

            let len = unsafe {
                self.builder
                    .build_gep(
                        vector_type,
                        vector,
                        &[
                            self.context.i32_type().const_zero(),
                            self.context.i32_type().const_zero(),
                        ],
                        "vector_len",
                    )
                    .unwrap()
            };

            self.builder
                .build_select(
                    self.builder
                        .build_is_null(vector, "vector_is_null")
                        .unwrap(),
                    self.context.i32_type().const_zero(),
                    self.builder
                        .build_load(self.context.i32_type(), len, "vector_len")
                        .unwrap()
                        .into_int_value(),
                    "length",
                )
                .unwrap()
                .into_int_value()
        }
    }

    /// Return the pointer to the actual bytes in the vector
    pub(crate) fn vector_bytes(&self, vector: BasicValueEnum<'a>) -> PointerValue<'a> {
        if vector.is_struct_value() {
            // slice
            let slice = vector.into_struct_value();
            self.builder
                .build_extract_value(slice, 0, "slice_data")
                .unwrap()
                .into_pointer_value()
        } else {
            let vector_type = self.module.get_struct_type("struct.vector").unwrap();
            unsafe {
                self.builder
                    .build_gep(
                        vector_type,
                        vector.into_pointer_value(),
                        &[
                            self.context.i32_type().const_zero(),
                            self.context.i32_type().const_int(2, false),
                        ],
                        "data",
                    )
                    .unwrap()
            }
        }
    }

    /// Dereference an array
    pub(crate) fn array_subscript(
        &self,
        array_ty: &Type,
        array: PointerValue<'a>,
        index: IntValue<'a>,
        ns: &Namespace,
    ) -> PointerValue<'a> {
        match array_ty {
            Type::Array(_, dim) => {
                if matches!(dim.last(), Some(ArrayLength::Fixed(_))) {
                    // fixed size array
                    let llvm_ty = self.llvm_type(array_ty, ns);
                    unsafe {
                        self.builder
                            .build_gep(
                                llvm_ty,
                                array,
                                &[self.context.i32_type().const_zero(), index],
                                "index_access",
                            )
                            .unwrap()
                    }
                } else {
                    let elem_ty = array_ty.array_deref();
                    let llvm_elem_ty = self.llvm_type(elem_ty.deref_memory(), ns);

                    // dynamic length array or vector
                    let index = self
                        .builder
                        .build_int_mul(
                            index,
                            llvm_elem_ty
                                .size_of()
                                .unwrap()
                                .const_cast(self.context.i32_type(), false),
                            "",
                        )
                        .unwrap();

                    let vector_type = self.module.get_struct_type("struct.vector").unwrap();

                    unsafe {
                        self.builder
                            .build_gep(
                                vector_type,
                                array,
                                &[
                                    self.context.i32_type().const_zero(),
                                    self.context.i32_type().const_int(2, false),
                                    index,
                                ],
                                "index_access",
                            )
                            .unwrap()
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    pub(super) fn log_runtime_error<T: TargetRuntime<'a> + ?Sized>(
        &self,
        target: &T,
        reason_string: String,
        reason_loc: Option<pt::Loc>,
        ns: &Namespace,
    ) {
        if !self.options.log_runtime_errors {
            return;
        }
        let error_with_loc = error_msg_with_loc(ns, reason_string, reason_loc);
        let global_string =
            self.emit_global_string("runtime_error", error_with_loc.as_bytes(), true);
        target.print(
            self,
            global_string,
            self.context
                .i32_type()
                .const_int(error_with_loc.len() as u64, false),
        );
    }

    /// Emit encoded error data of "Panic(uint256)" as interned global string.
    ///
    /// On Solana, because reverts do not return data, a nil ptr is returned.
    pub(super) fn panic_data_const(
        &self,
        ns: &Namespace,
        code: PanicCode,
    ) -> (PointerValue<'a>, IntValue<'a>) {
        if ns.target == Target::Solana || ns.target == Target::Soroban {
            return (
                self.context
                    .i8_type()
                    .ptr_type(AddressSpace::default())
                    .const_null(),
                self.context.i32_type().const_zero(),
            );
        }

        let expr = Expression::NumberLiteral {
            loc: pt::Loc::Codegen,
            ty: Type::Uint(256),
            value: (code as u8).into(),
        };
        let bytes = create_encoder(ns, false)
            .const_encode(&[SolidityError::Panic(code).selector_expression(ns), expr])
            .unwrap();
        (
            self.emit_global_string(&code.to_string(), &bytes, true),
            self.context.i32_type().const_int(bytes.len() as u64, false),
        )
    }
}

/// Return the stdlib as parsed llvm module. The solidity standard library is hardcoded into
/// the solang library
fn load_stdlib<'a>(context: &'a Context, target: &Target) -> Module<'a> {
    if *target == Target::Solana {
        let memory = MemoryBuffer::create_from_memory_range(BPF_IR[0], "bpf_bc");

        let module = Module::parse_bitcode_from_buffer(&memory, context).unwrap();

        for bc in BPF_IR.iter().skip(1) {
            let memory = MemoryBuffer::create_from_memory_range(bc, "bpf_bc");

            module
                .link_in_module(Module::parse_bitcode_from_buffer(&memory, context).unwrap())
                .unwrap();
        }

        return module;
    }

    let memory = MemoryBuffer::create_from_memory_range(WASM_IR[0], "wasm_bc");

    let module = Module::parse_bitcode_from_buffer(&memory, context).unwrap();

    for bc in WASM_IR.iter().skip(1) {
        let memory = MemoryBuffer::create_from_memory_range(bc, "wasm_bc");

        module
            .link_in_module(Module::parse_bitcode_from_buffer(&memory, context).unwrap())
            .unwrap();
    }

    if let Target::Polkadot { .. } = *target {
        // contracts pallet does not provide ripemd160
        let memory = MemoryBuffer::create_from_memory_range(RIPEMD160_IR, "ripemd160");

        module
            .link_in_module(Module::parse_bitcode_from_buffer(&memory, context).unwrap())
            .unwrap();
    }

    module
}

static BPF_IR: [&[u8]; 6] = [
    include_bytes!("../../target/bpf/stdlib.bc"),
    include_bytes!("../../target/bpf/bigint.bc"),
    include_bytes!("../../target/bpf/format.bc"),
    include_bytes!("../../target/bpf/solana.bc"),
    include_bytes!("../../target/bpf/ripemd160.bc"),
    include_bytes!("../../target/bpf/heap.bc"),
];

static WASM_IR: [&[u8]; 4] = [
    include_bytes!("../../target/wasm/stdlib.bc"),
    include_bytes!("../../target/wasm/heap.bc"),
    include_bytes!("../../target/wasm/bigint.bc"),
    include_bytes!("../../target/wasm/format.bc"),
];

static RIPEMD160_IR: &[u8] = include_bytes!("../../target/wasm/ripemd160.bc");
