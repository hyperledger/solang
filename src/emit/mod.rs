// SPDX-License-Identifier: Apache-2.0

use crate::codegen::Expression;
use crate::sema::ast::{CallTy, Function, Namespace, Type};
use std::collections::HashMap;
use std::fmt;
use std::str;

use crate::Target;
use inkwell::targets::TargetTriple;
use inkwell::types::{BasicTypeEnum, IntType};
use inkwell::values::{
    ArrayValue, BasicMetadataValueEnum, BasicValueEnum, FunctionValue, IntValue, PointerValue,
};
use solang_parser::pt::{Loc, StorageType};

pub mod binary;
mod cfg;
mod expression;
mod functions;
mod instructions;
mod loop_builder;
mod math;
pub mod polkadot;
pub mod solana;

#[cfg(feature = "soroban")]
pub mod soroban;
mod storage;
mod strings;

use crate::codegen::{cfg::HashTy, Options};
use crate::emit::binary::Binary;
use crate::sema::ast;

#[derive(Clone)]
pub struct Variable<'a> {
    value: BasicValueEnum<'a>,
}

pub struct ContractArgs<'b> {
    program_id: Option<PointerValue<'b>>,
    value: Option<IntValue<'b>>,
    gas: Option<IntValue<'b>>,
    salt: Option<IntValue<'b>>,
    seeds: Option<(PointerValue<'b>, IntValue<'b>)>,
    accounts: Option<(PointerValue<'b>, IntValue<'b>)>,
    flags: Option<IntValue<'b>>,
}

#[derive(Clone, Copy)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
}

impl fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Add => "add",
                Self::Subtract => "sub",
                Self::Multiply => "mul",
            }
        )
    }
}

pub trait TargetRuntime<'a> {
    fn get_storage_int(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue,
        slot: PointerValue<'a>,
        ty: IntType<'a>,
    ) -> IntValue<'a>;

    fn storage_load(
        &self,
        binary: &Binary<'a>,
        ty: &ast::Type,
        slot: &mut IntValue<'a>,
        function: FunctionValue<'a>,
        ns: &ast::Namespace,
        storage_type: &Option<StorageType>,
    ) -> BasicValueEnum<'a>;

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
    );

    /// Recursively clear storage. The default implementation is for slot-based storage
    fn storage_delete(
        &self,
        bin: &Binary<'a>,
        ty: &Type,
        slot: &mut IntValue<'a>,
        function: FunctionValue<'a>,
        ns: &Namespace,
    );

    // Bytes and string have special storage layout
    fn set_storage_string(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue<'a>,
        slot: PointerValue<'a>,
        dest: BasicValueEnum<'a>,
    );

    fn get_storage_string(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue,
        slot: PointerValue<'a>,
    ) -> PointerValue<'a>;

    fn set_storage_extfunc(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue,
        slot: PointerValue,
        dest: PointerValue,
        dest_ty: BasicTypeEnum,
    );

    fn get_storage_extfunc(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue,
        slot: PointerValue<'a>,
        ns: &Namespace,
    ) -> PointerValue<'a>;

    fn get_storage_bytes_subscript(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue,
        slot: IntValue<'a>,
        index: IntValue<'a>,
        loc: Loc,
        ns: &Namespace,
    ) -> IntValue<'a>;

    fn set_storage_bytes_subscript(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue,
        slot: IntValue<'a>,
        index: IntValue<'a>,
        value: IntValue<'a>,
        ns: &Namespace,
        loc: Loc,
    );

    fn storage_subscript(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue<'a>,
        ty: &Type,
        slot: IntValue<'a>,
        index: BasicValueEnum<'a>,
        ns: &Namespace,
    ) -> IntValue<'a>;

    fn storage_push(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue<'a>,
        ty: &Type,
        slot: IntValue<'a>,
        val: Option<BasicValueEnum<'a>>,
        ns: &Namespace,
    ) -> BasicValueEnum<'a>;

    fn storage_pop(
        &self,
        bin: &Binary<'a>,
        function: FunctionValue<'a>,
        ty: &Type,
        slot: IntValue<'a>,
        load: bool,
        ns: &Namespace,
        loc: Loc,
    ) -> Option<BasicValueEnum<'a>>;

    fn storage_array_length(
        &self,
        _bin: &Binary<'a>,
        _function: FunctionValue,
        _slot: IntValue<'a>,
        _elem_ty: &Type,
        _ns: &Namespace,
    ) -> IntValue<'a>;

    /// keccak256 hash
    fn keccak256_hash(
        &self,
        bin: &Binary<'a>,
        src: PointerValue,
        length: IntValue,
        dest: PointerValue,
        ns: &Namespace,
    );

    /// Prints a string
    fn print(&self, bin: &Binary, string: PointerValue, length: IntValue);

    /// Return success without any result
    fn return_empty_abi(&self, bin: &Binary);

    /// Return failure code
    fn return_code<'b>(&self, bin: &'b Binary, ret: IntValue<'b>);

    /// Return failure without any result
    fn assert_failure(&self, bin: &Binary, data: PointerValue, length: IntValue);

    fn builtin_function(
        &self,
        binary: &Binary<'a>,
        function: FunctionValue<'a>,
        builtin_func: &Function,
        args: &[BasicMetadataValueEnum<'a>],
        first_arg_type: Option<BasicTypeEnum>,
        ns: &Namespace,
    ) -> Option<BasicValueEnum<'a>>;

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
    );

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
    );

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
    );

    /// builtin expressions
    fn builtin<'b>(
        &self,
        bin: &Binary<'b>,
        expr: &Expression,
        vartab: &HashMap<usize, Variable<'b>>,
        function: FunctionValue<'b>,
        ns: &Namespace,
    ) -> BasicValueEnum<'b>;

    /// Return the return data from an external call (either revert error or return values)
    fn return_data<'b>(&self, bin: &Binary<'b>, function: FunctionValue<'b>) -> PointerValue<'b>;

    /// Return the value we received
    fn value_transferred<'b>(&self, binary: &Binary<'b>, ns: &Namespace) -> IntValue<'b>;

    /// Terminate execution, destroy bin and send remaining funds to addr
    fn selfdestruct<'b>(&self, binary: &Binary<'b>, addr: ArrayValue<'b>, ns: &Namespace);

    /// Crypto Hash
    fn hash<'b>(
        &self,
        bin: &Binary<'b>,
        function: FunctionValue<'b>,
        hash: HashTy,
        string: PointerValue<'b>,
        length: IntValue<'b>,
        ns: &Namespace,
    ) -> IntValue<'b>;

    /// Emit event
    fn emit_event<'b>(
        &self,
        bin: &Binary<'b>,
        function: FunctionValue<'b>,
        data: BasicValueEnum<'b>,
        topics: &[BasicValueEnum<'b>],
    );

    /// Return ABI encoded data
    fn return_abi_data<'b>(
        &self,
        binary: &Binary<'b>,
        data: PointerValue<'b>,
        data_len: BasicValueEnum<'b>,
    );
}

#[derive(PartialEq, Eq)]
pub enum Generate {
    Object,
    Assembly,
    Linked,
}

impl Target {
    /// LLVM Target name
    fn llvm_target_name(&self) -> &'static str {
        if *self == Target::Solana {
            "sbf"
        } else {
            "wasm32"
        }
    }

    /// LLVM Target triple
    fn llvm_target_triple(&self) -> TargetTriple {
        TargetTriple::create(if *self == Target::Solana {
            "sbf-unknown-unknown"
        } else {
            "wasm32-unknown-unknown-wasm"
        })
    }

    /// LLVM Target triple
    fn llvm_features(&self) -> &'static str {
        if *self == Target::Solana {
            "+solana"
        } else {
            ""
        }
    }
}

impl ast::Contract {
    /// Generate the binary. This can be used to generate llvm text, object file
    /// or final linked binary.
    pub fn binary<'a>(
        &'a self,
        ns: &'a ast::Namespace,
        context: &'a inkwell::context::Context,
        opt: &'a Options,
        contract_no: usize,
    ) -> binary::Binary {
        binary::Binary::build(context, self, ns, opt, contract_no)
    }

    /// Generate the final program code for the contract
    pub fn emit(&self, ns: &ast::Namespace, opt: &Options, contract_no: usize) -> Vec<u8> {
        if ns.target == Target::EVM {
            return vec![];
        }

        self.code
            .get_or_init(move || {
                let context = inkwell::context::Context::create();
                let binary = self.binary(ns, &context, opt, contract_no);
                binary.code(Generate::Linked).expect("llvm build")
            })
            .to_vec()
    }
}
