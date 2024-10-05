// SPDX-License-Identifier: Apache-2.0

mod array_boundary;
pub mod cfg;
mod constant_folding;
mod constructor;
mod dead_storage;
pub(crate) mod dispatch;
pub(crate) mod encoding;
mod events;
mod expression;
pub(super) mod polkadot;
mod reaching_definitions;
pub mod revert;
mod solana_accounts;
mod solana_deploy;
mod statements;
mod storage;
mod strength_reduce;
pub(crate) mod subexpression_elimination;
mod tests;
mod undefined_variable;
mod unused_variable;
pub(crate) mod vartable;
mod vector_to_slice;
mod yul;

use self::{
    cfg::{optimize_and_check_cfg, ControlFlowGraph, Instr},
    dispatch::function_dispatch,
    expression::expression,
    solana_accounts::account_collection::collect_accounts_from_contract,
    vartable::Vartable,
};
use crate::sema::ast::{
    FormatArg, Function, Layout, Namespace, RetrieveType, StringLocation, Type,
};
use crate::{sema::ast, Target};
use std::cmp::Ordering;

use crate::codegen::cfg::ASTFunction;
use crate::codegen::solana_accounts::account_management::manage_contract_accounts;
use crate::codegen::yul::generate_yul_function_cfg;
use crate::sema::diagnostics::Diagnostics;
use crate::sema::eval::eval_const_number;
use crate::sema::Recurse;
#[cfg(feature = "wasm_opt")]
use contract_build::OptimizationPasses;
use num_bigint::{BigInt, Sign};
use num_rational::BigRational;
use num_traits::{FromPrimitive, Zero};
use solang_parser::diagnostics::Diagnostic;
use solang_parser::{pt, pt::CodeLocation};

// The sizeof(struct account_data_header)
pub const SOLANA_FIRST_OFFSET: u64 = 16;

/// Name of the storage initializer function
pub const STORAGE_INITIALIZER: &str = "storage_initializer";

/// Maximum permitted size of account data (10 MiB).
/// https://github.com/solana-labs/solana/blob/08aba38d3507c8cb66f85074d8f1249d43e64a75/sdk/program/src/system_instruction.rs#L85
pub const MAXIMUM_ACCOUNT_SIZE: u64 = 10 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum OptimizationLevel {
    None = 0,
    Less = 1,
    Default = 2,
    Aggressive = 3,
}

#[cfg(feature = "llvm")]
impl From<OptimizationLevel> for inkwell::OptimizationLevel {
    fn from(level: OptimizationLevel) -> Self {
        match level {
            OptimizationLevel::None => inkwell::OptimizationLevel::None,
            OptimizationLevel::Less => inkwell::OptimizationLevel::Less,
            OptimizationLevel::Default => inkwell::OptimizationLevel::Default,
            OptimizationLevel::Aggressive => inkwell::OptimizationLevel::Aggressive,
        }
    }
}

#[cfg(feature = "llvm")]
impl From<inkwell::OptimizationLevel> for OptimizationLevel {
    fn from(level: inkwell::OptimizationLevel) -> Self {
        match level {
            inkwell::OptimizationLevel::None => OptimizationLevel::None,
            inkwell::OptimizationLevel::Less => OptimizationLevel::Less,
            inkwell::OptimizationLevel::Default => OptimizationLevel::Default,
            inkwell::OptimizationLevel::Aggressive => OptimizationLevel::Aggressive,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub dead_storage: bool,
    pub constant_folding: bool,
    pub strength_reduce: bool,
    pub vector_to_slice: bool,
    pub common_subexpression_elimination: bool,
    pub generate_debug_information: bool,
    pub opt_level: OptimizationLevel,
    pub log_runtime_errors: bool,
    pub log_prints: bool,
    #[cfg(feature = "wasm_opt")]
    pub wasm_opt: Option<OptimizationPasses>,
    pub soroban_version: Option<u64>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            dead_storage: true,
            constant_folding: true,
            strength_reduce: true,
            vector_to_slice: true,
            common_subexpression_elimination: true,
            generate_debug_information: false,
            opt_level: OptimizationLevel::Default,
            log_runtime_errors: false,
            log_prints: true,
            #[cfg(feature = "wasm_opt")]
            wasm_opt: None,
            soroban_version: None,
        }
    }
}

/// The contracts are fully resolved but they do not have any CFGs which is needed for
/// the llvm code emitter. This will also do additional code checks.
pub fn codegen(ns: &mut Namespace, opt: &Options) {
    if ns.diagnostics.any_errors() {
        return;
    }

    let mut contracts_done = Vec::new();

    contracts_done.resize(ns.contracts.len(), false);

    // codegen all the contracts; some additional errors/warnings will be detected here
    while contracts_done.iter().any(|e| !*e) {
        for contract_no in 0..ns.contracts.len() {
            if contracts_done[contract_no] {
                continue;
            }

            if !ns.contracts[contract_no].instantiable {
                contracts_done[contract_no] = true;
                continue;
            }

            // does this contract create any contract which are not done
            if ns.contracts[contract_no]
                .creates
                .iter()
                .any(|c| !contracts_done[*c])
            {
                continue;
            }

            contract(contract_no, ns, opt);

            if ns.diagnostics.any_errors() {
                return;
            }

            contracts_done[contract_no] = true;
        }
    }

    if ns.target == Target::Solana {
        for contract_no in 0..ns.contracts.len() {
            if ns.contracts[contract_no].instantiable {
                let diag = collect_accounts_from_contract(contract_no, ns);
                ns.diagnostics.extend(diag);
            }
        }

        for contract_no in 0..ns.contracts.len() {
            if ns.contracts[contract_no].instantiable {
                manage_contract_accounts(contract_no, ns);
            }
        }
    }
    ns.diagnostics.sort_and_dedup();
}

fn contract(contract_no: usize, ns: &mut Namespace, opt: &Options) {
    if !ns.diagnostics.any_errors() && ns.contracts[contract_no].instantiable {
        layout(contract_no, ns);

        let mut cfg_no = 0;
        let mut all_cfg = Vec::new();

        // all the functions should have a cfg_no assigned, so we can generate call instructions to the correct function
        for (_, func_cfg) in ns.contracts[contract_no].all_functions.iter_mut() {
            *func_cfg = cfg_no;
            cfg_no += 1;
        }

        // create a cfg number for yul functions
        for yul_fn_no in &ns.contracts[contract_no].yul_functions {
            ns.yul_functions[*yul_fn_no].cfg_no = cfg_no;
            cfg_no += 1;
        }

        all_cfg.resize(cfg_no, ControlFlowGraph::placeholder());

        // clone all_functions so we can pass a mutable reference to generate_cfg
        for (function_no, cfg_no) in ns.contracts[contract_no]
            .all_functions
            .iter()
            .map(|(function_no, cfg_no)| (*function_no, *cfg_no))
            .collect::<Vec<(usize, usize)>>()
            .into_iter()
        {
            cfg::generate_cfg(
                contract_no,
                Some(function_no),
                cfg_no,
                &mut all_cfg,
                ns,
                opt,
            )
        }

        // generate the cfg for yul functions
        for yul_func_no in ns.contracts[contract_no].yul_functions.clone() {
            generate_yul_function_cfg(contract_no, yul_func_no, &mut all_cfg, ns, opt);
        }

        // Generate cfg for storage initializers
        let cfg = storage_initializer(contract_no, ns, opt);
        let pos = all_cfg.len();
        all_cfg.push(cfg);
        ns.contracts[contract_no].initializer = Some(pos);

        if ns.contracts[contract_no].constructors(ns).is_empty() {
            // generate the default constructor
            let func = ns.default_constructor(contract_no);
            let cfg_no = all_cfg.len();
            all_cfg.push(ControlFlowGraph::placeholder());

            cfg::generate_cfg(contract_no, None, cfg_no, &mut all_cfg, ns, opt);

            ns.contracts[contract_no].default_constructor = Some((func, cfg_no));
        }

        for mut dispatch_cfg in function_dispatch(contract_no, &mut all_cfg, ns, opt) {
            optimize_and_check_cfg(&mut dispatch_cfg, ns, ASTFunction::None, opt);
            all_cfg.push(dispatch_cfg);
        }

        ns.contracts[contract_no].cfg = all_cfg;
    }
}

/// This function will set all contract storage initializers and should be called from the constructor
fn storage_initializer(contract_no: usize, ns: &mut Namespace, opt: &Options) -> ControlFlowGraph {
    // note the single `:` to prevent a name clash with user-declared functions
    let mut cfg = ControlFlowGraph::new(STORAGE_INITIALIZER.to_string(), ASTFunction::None);
    let mut vartab = Vartable::new(ns.next_id);

    for layout in &ns.contracts[contract_no].layout {
        let var = &ns.contracts[layout.contract_no].variables[layout.var_no];

        if let Some(init) = &var.initializer {
            let storage = ns.contracts[contract_no].get_storage_slot(
                pt::Loc::Codegen,
                layout.contract_no,
                layout.var_no,
                ns,
                None,
            );

            let value = expression(init, &mut cfg, contract_no, None, ns, &mut vartab, opt);

            cfg.add(
                &mut vartab,
                Instr::SetStorage {
                    value,
                    ty: var.ty.clone(),
                    storage,
                    storage_type: var.storage_type.clone(),
                },
            );
        }
    }

    cfg.add(&mut vartab, Instr::Return { value: Vec::new() });

    vartab.finalize(ns, &mut cfg);

    optimize_and_check_cfg(&mut cfg, ns, ASTFunction::None, opt);

    cfg
}

/// Layout the contract. We determine the layout of variables and deal with overriding variables
fn layout(contract_no: usize, ns: &mut Namespace) {
    let mut slot = if ns.target == Target::Solana {
        BigInt::from(SOLANA_FIRST_OFFSET)
    } else {
        BigInt::zero()
    };

    for base_contract_no in ns.contract_bases(contract_no) {
        for var_no in 0..ns.contracts[base_contract_no].variables.len() {
            if !ns.contracts[base_contract_no].variables[var_no].constant {
                let ty = ns.contracts[base_contract_no].variables[var_no].ty.clone();

                if ns.target == Target::Solana {
                    // elements need to be aligned on solana
                    let alignment = ty.align_of(ns);

                    let offset = slot.clone() % alignment;

                    if offset > BigInt::zero() {
                        slot += alignment - offset;
                    }
                }

                ns.contracts[contract_no].layout.push(Layout {
                    slot: slot.clone(),
                    contract_no: base_contract_no,
                    var_no,
                    ty: ty.clone(),
                });

                slot += ty.storage_slots(ns);
            }
        }
    }

    let constructors = ns.contracts[contract_no].constructors(ns);
    if !constructors.is_empty() {
        if let Some((_, exp)) = &ns.functions[constructors[0]].annotations.space {
            // This code path is only reachable on Solana
            assert_eq!(ns.target, Target::Solana);
            if let Ok((_, value)) = eval_const_number(exp, ns, &mut Diagnostics::default()) {
                if slot > value {
                    ns.diagnostics.push(Diagnostic::error(
                        exp.loc(),
                        format!("contract requires at least {} bytes of space", slot),
                    ));
                } else if value > BigInt::from(MAXIMUM_ACCOUNT_SIZE) {
                    ns.diagnostics.push(Diagnostic::error(
                        exp.loc(),
                        "Solana's runtime does not permit accounts larger than 10 MB".to_string(),
                    ));
                }
            }
        }
    }

    ns.contracts[contract_no].fixed_layout_size = slot;
}

trait LLVMName {
    fn llvm_symbol(&self, ns: &Namespace) -> String;
}

impl LLVMName for Function {
    /// Return a unique string for this function which is a valid llvm symbol
    fn llvm_symbol(&self, ns: &Namespace) -> String {
        let mut sig = self.id.name.to_owned();

        if !self.params.is_empty() {
            sig.push_str("__");

            for (i, p) in self.params.iter().enumerate() {
                if i > 0 {
                    sig.push('_');
                }

                sig.push_str(&p.ty.to_llvm_string(ns));
            }
        }

        sig
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expression {
    Add {
        loc: pt::Loc,
        ty: Type,
        overflowing: bool,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    AllocDynamicBytes {
        loc: pt::Loc,
        ty: Type,
        size: Box<Expression>,
        initializer: Option<Vec<u8>>,
    },
    ArrayLiteral {
        loc: pt::Loc,
        ty: Type,
        dimensions: Vec<u32>,
        values: Vec<Expression>,
    },
    BitwiseAnd {
        loc: pt::Loc,
        ty: Type,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    BitwiseOr {
        loc: pt::Loc,
        ty: Type,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    BitwiseXor {
        loc: pt::Loc,
        ty: Type,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    BoolLiteral {
        loc: pt::Loc,
        value: bool,
    },
    Builtin {
        loc: pt::Loc,
        tys: Vec<Type>,
        kind: Builtin,
        args: Vec<Expression>,
    },
    BytesCast {
        loc: pt::Loc,
        ty: Type,
        from: Type,
        expr: Box<Expression>,
    },
    BytesLiteral {
        loc: pt::Loc,
        ty: Type,
        value: Vec<u8>,
    },
    Cast {
        loc: pt::Loc,
        ty: Type,
        expr: Box<Expression>,
    },
    BitwiseNot {
        loc: pt::Loc,
        ty: Type,
        expr: Box<Expression>,
    },
    ConstArrayLiteral {
        loc: pt::Loc,
        ty: Type,
        dimensions: Vec<u32>,
        values: Vec<Expression>,
    },
    UnsignedDivide {
        loc: pt::Loc,
        ty: Type,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    SignedDivide {
        loc: pt::Loc,
        ty: Type,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Equal {
        loc: pt::Loc,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    FormatString {
        loc: pt::Loc,
        args: Vec<(FormatArg, Expression)>,
    },
    FunctionArg {
        loc: pt::Loc,
        ty: Type,
        arg_no: usize,
    },
    GetRef {
        loc: pt::Loc,
        ty: Type,
        expr: Box<Expression>,
    },
    InternalFunctionCfg {
        ty: Type,
        cfg_no: usize,
    },
    Keccak256 {
        loc: pt::Loc,
        ty: Type,
        exprs: Vec<Expression>,
    },
    Less {
        loc: pt::Loc,
        signed: bool,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    LessEqual {
        loc: pt::Loc,
        signed: bool,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Load {
        loc: pt::Loc,
        ty: Type,
        expr: Box<Expression>,
    },
    UnsignedModulo {
        loc: pt::Loc,
        ty: Type,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    SignedModulo {
        loc: pt::Loc,
        ty: Type,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    More {
        loc: pt::Loc,
        signed: bool,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    MoreEqual {
        loc: pt::Loc,
        signed: bool,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Multiply {
        loc: pt::Loc,
        ty: Type,
        overflowing: bool,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Not {
        loc: pt::Loc,
        expr: Box<Expression>,
    },
    NotEqual {
        loc: pt::Loc,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    NumberLiteral {
        loc: pt::Loc,
        ty: Type,
        value: BigInt,
    },
    Poison,
    Power {
        loc: pt::Loc,
        ty: Type,
        overflowing: bool,
        base: Box<Expression>,
        exp: Box<Expression>,
    },
    RationalNumberLiteral {
        loc: pt::Loc,
        ty: Type,
        rational: BigRational,
    },
    ReturnData {
        loc: pt::Loc,
    },
    SignExt {
        loc: pt::Loc,
        ty: Type,
        expr: Box<Expression>,
    },
    ShiftLeft {
        loc: pt::Loc,
        ty: Type,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    ShiftRight {
        loc: pt::Loc,
        ty: Type,
        left: Box<Expression>,
        right: Box<Expression>,
        signed: bool,
    },
    StorageArrayLength {
        loc: pt::Loc,
        ty: Type,
        array: Box<Expression>,
        elem_ty: Type,
    },
    StringCompare {
        loc: pt::Loc,
        left: StringLocation<Expression>,
        right: StringLocation<Expression>,
    },
    StructLiteral {
        loc: pt::Loc,
        ty: Type,
        values: Vec<Expression>,
    },
    StructMember {
        loc: pt::Loc,
        ty: Type,
        expr: Box<Expression>,
        member: usize,
    },
    Subscript {
        loc: pt::Loc,
        ty: Type,
        array_ty: Type,
        expr: Box<Expression>,
        index: Box<Expression>,
    },
    Subtract {
        loc: pt::Loc,
        ty: Type,
        overflowing: bool,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Trunc {
        loc: pt::Loc,
        ty: Type,
        expr: Box<Expression>,
    },
    Negate {
        loc: pt::Loc,
        ty: Type,
        overflowing: bool,
        expr: Box<Expression>,
    },
    Undefined {
        ty: Type,
    },
    Variable {
        loc: pt::Loc,
        ty: Type,
        var_no: usize,
    },
    ZeroExt {
        loc: pt::Loc,
        ty: Type,
        expr: Box<Expression>,
    },
    AdvancePointer {
        pointer: Box<Expression>,
        bytes_offset: Box<Expression>,
    },
}

impl CodeLocation for Expression {
    fn loc(&self) -> pt::Loc {
        match self {
            Expression::StorageArrayLength { loc, .. }
            | Expression::Builtin { loc, .. }
            | Expression::Cast { loc, .. }
            | Expression::NumberLiteral { loc, .. }
            | Expression::Keccak256 { loc, .. }
            | Expression::MoreEqual { loc, .. }
            | Expression::ReturnData { loc }
            | Expression::Subscript { loc, .. }
            | Expression::Trunc { loc, .. }
            | Expression::Variable { loc, .. }
            | Expression::SignExt { loc, .. }
            | Expression::GetRef { loc, .. }
            | Expression::Load { loc, .. }
            | Expression::BytesLiteral { loc, .. }
            | Expression::Add { loc, .. }
            | Expression::Multiply { loc, .. }
            | Expression::Subtract { loc, .. }
            | Expression::FormatString { loc, .. }
            | Expression::LessEqual { loc, .. }
            | Expression::BoolLiteral { loc, .. }
            | Expression::UnsignedDivide { loc, .. }
            | Expression::SignedDivide { loc, .. }
            | Expression::UnsignedModulo { loc, .. }
            | Expression::SignedModulo { loc, .. }
            | Expression::Power { loc, .. }
            | Expression::BitwiseOr { loc, .. }
            | Expression::BitwiseAnd { loc, .. }
            | Expression::BitwiseXor { loc, .. }
            | Expression::Equal { loc, .. }
            | Expression::NotEqual { loc, .. }
            | Expression::BitwiseNot { loc, .. }
            | Expression::Negate { loc, .. }
            | Expression::Less { loc, .. }
            | Expression::Not { loc, .. }
            | Expression::StructLiteral { loc, .. }
            | Expression::ArrayLiteral { loc, .. }
            | Expression::ConstArrayLiteral { loc, .. }
            | Expression::StructMember { loc, .. }
            | Expression::StringCompare { loc, .. }
            | Expression::FunctionArg { loc, .. }
            | Expression::ShiftRight { loc, .. }
            | Expression::ShiftLeft { loc, .. }
            | Expression::RationalNumberLiteral { loc, .. }
            | Expression::AllocDynamicBytes { loc, .. }
            | Expression::BytesCast { loc, .. }
            | Expression::More { loc, .. }
            | Expression::ZeroExt { loc, .. } => *loc,

            Expression::InternalFunctionCfg { .. }
            | Expression::Poison
            | Expression::Undefined { .. }
            | Expression::AdvancePointer { .. } => pt::Loc::Codegen,
        }
    }
}

impl Recurse for Expression {
    type ArgType = Expression;
    fn recurse<T>(&self, cx: &mut T, f: fn(expr: &Expression, ctx: &mut T) -> bool) {
        if !f(self, cx) {
            return;
        }
        match self {
            Expression::BitwiseAnd { left, right, .. }
            | Expression::BitwiseOr { left, right, .. }
            | Expression::UnsignedDivide { left, right, .. }
            | Expression::SignedDivide { left, right, .. }
            | Expression::Equal { left, right, .. }
            | Expression::Less { left, right, .. }
            | Expression::LessEqual { left, right, .. }
            | Expression::BitwiseXor { left, right, .. }
            | Expression::More { left, right, .. }
            | Expression::MoreEqual { left, right, .. }
            | Expression::Multiply { left, right, .. }
            | Expression::NotEqual { left, right, .. }
            | Expression::ShiftLeft { left, right, .. }
            | Expression::ShiftRight { left, right, .. }
            | Expression::Power {
                base: left,
                exp: right,
                ..
            }
            | Expression::Subscript {
                expr: left,
                index: right,
                ..
            }
            | Expression::Subtract { left, right, .. }
            | Expression::AdvancePointer {
                pointer: left,
                bytes_offset: right,
                ..
            }
            | Expression::Add { left, right, .. } => {
                left.recurse(cx, f);
                right.recurse(cx, f);
            }

            Expression::BytesCast { expr, .. }
            | Expression::Cast { expr, .. }
            | Expression::GetRef { expr, .. }
            | Expression::Not { expr, .. }
            | Expression::Trunc { expr, .. }
            | Expression::Negate { expr, .. }
            | Expression::ZeroExt { expr, .. }
            | Expression::SignExt { expr, .. }
            | Expression::BitwiseNot { expr, .. }
            | Expression::Load { expr, .. }
            | Expression::StorageArrayLength { array: expr, .. }
            | Expression::StructMember { expr, .. }
            | Expression::AllocDynamicBytes { size: expr, .. } => {
                expr.recurse(cx, f);
            }

            Expression::Builtin { args, .. }
            | Expression::ConstArrayLiteral { values: args, .. }
            | Expression::Keccak256 { exprs: args, .. }
            | Expression::StructLiteral { values: args, .. }
            | Expression::ArrayLiteral { values: args, .. } => {
                for item in args {
                    item.recurse(cx, f);
                }
            }

            Expression::FormatString { args, .. } => {
                for item in args {
                    item.1.recurse(cx, f);
                }
            }

            Expression::StringCompare { left, right, .. } => {
                if let StringLocation::RunTime(exp) = left {
                    exp.recurse(cx, f);
                }

                if let StringLocation::RunTime(exp) = right {
                    exp.recurse(cx, f);
                }
            }

            _ => (),
        }
    }
}

impl RetrieveType for Expression {
    fn ty(&self) -> Type {
        match self {
            Expression::ReturnData { loc: _ } => Type::DynamicBytes,
            Expression::Builtin { tys, .. } => {
                assert_eq!(tys.len(), 1);
                tys[0].clone()
            }
            Expression::Keccak256 { ty, .. }
            | Expression::Undefined { ty }
            | Expression::Variable { ty, .. }
            | Expression::Trunc { ty, .. }
            | Expression::ZeroExt { ty, .. }
            | Expression::Cast { ty, .. }
            | Expression::SignExt { ty, .. }
            | Expression::GetRef { ty, .. }
            | Expression::Load { ty, .. }
            | Expression::BytesLiteral { ty, .. }
            | Expression::Add { ty, .. }
            | Expression::NumberLiteral { ty, .. }
            | Expression::Multiply { ty, .. }
            | Expression::Subtract { ty, .. }
            | Expression::SignedDivide { ty, .. }
            | Expression::UnsignedDivide { ty, .. }
            | Expression::SignedModulo { ty, .. }
            | Expression::UnsignedModulo { ty, .. }
            | Expression::Power { ty, .. }
            | Expression::BitwiseOr { ty, .. }
            | Expression::BitwiseAnd { ty, .. }
            | Expression::BitwiseXor { ty, .. }
            | Expression::ShiftLeft { ty, .. }
            | Expression::ShiftRight { ty, .. }
            | Expression::BitwiseNot { ty, .. }
            | Expression::StorageArrayLength { ty, .. }
            | Expression::Negate { ty, .. }
            | Expression::StructLiteral { ty, .. }
            | Expression::ArrayLiteral { ty, .. }
            | Expression::ConstArrayLiteral { ty, .. }
            | Expression::StructMember { ty, .. }
            | Expression::FunctionArg { ty, .. }
            | Expression::AllocDynamicBytes { ty, .. }
            | Expression::BytesCast { ty, .. }
            | Expression::RationalNumberLiteral { ty, .. }
            | Expression::Subscript { ty, .. }
            | Expression::InternalFunctionCfg { ty, .. } => ty.clone(),

            Expression::BoolLiteral { .. }
            | Expression::MoreEqual { .. }
            | Expression::More { .. }
            | Expression::Not { .. }
            | Expression::NotEqual { .. }
            | Expression::Less { .. }
            | Expression::Equal { .. }
            | Expression::StringCompare { .. }
            | Expression::LessEqual { .. } => Type::Bool,

            Expression::AdvancePointer { .. } => Type::BufferPointer,
            Expression::FormatString { .. } => Type::String,
            Expression::Poison => unreachable!("Expression does not have a type"),
        }
    }
}

impl Expression {
    /// Increment an expression by some value.
    pub(crate) fn add_u32(self, other: Expression) -> Self {
        Expression::Add {
            loc: pt::Loc::Codegen,
            ty: Type::Uint(32),
            overflowing: false,
            left: self.into(),
            right: other.into(),
        }
    }

    pub(crate) fn cast(&self, to: &Type, ns: &Namespace) -> Expression {
        let from = self.ty();

        if &from == to {
            return self.clone();
        }

        let address_bits = ns.address_length as u16 * 8;

        // When converting from literals, there is not need to trunc or extend.
        match (self, &from, to) {
            (Expression::NumberLiteral { value, .. }, p, &Type::Uint(to_len))
                if p.is_primitive() =>
            {
                return if value.sign() == Sign::Minus {
                    let mut bs = value.to_signed_bytes_le();
                    bs.resize(to_len as usize / 8, 0xff);
                    Expression::NumberLiteral {
                        loc: self.loc(),
                        ty: Type::Uint(to_len),
                        value: BigInt::from_bytes_le(Sign::Plus, &bs),
                    }
                } else {
                    Expression::NumberLiteral {
                        loc: self.loc(),
                        ty: Type::Uint(to_len),
                        value: value.clone(),
                    }
                }
            }
            (Expression::NumberLiteral { value, .. }, p, &Type::Int(to_len))
                if p.is_primitive() =>
            {
                return Expression::NumberLiteral {
                    loc: self.loc(),
                    ty: Type::Int(to_len),
                    value: value.clone(),
                };
            }
            (Expression::NumberLiteral { value, .. }, p, &Type::Bytes(to_len))
                if p.is_primitive() =>
            {
                return Expression::NumberLiteral {
                    loc: self.loc(),
                    ty: Type::Bytes(to_len),
                    value: value.clone(),
                };
            }
            (Expression::NumberLiteral { value, .. }, p, &Type::Address(payable))
                if p.is_primitive() =>
            {
                return Expression::NumberLiteral {
                    loc: self.loc(),
                    ty: Type::Address(payable),
                    value: value.clone(),
                };
            }

            (Expression::BytesLiteral { value: bs, .. }, p, &Type::Bytes(to_len))
                if p.is_primitive() =>
            {
                let mut bs = bs.to_owned();
                bs.resize(to_len as usize, 0);
                return Expression::BytesLiteral {
                    loc: self.loc(),
                    ty: Type::Bytes(to_len),
                    value: bs,
                };
            }
            (
                Expression::BytesLiteral {
                    loc, value: init, ..
                },
                _,
                &Type::DynamicBytes,
            )
            | (
                Expression::BytesLiteral {
                    loc, value: init, ..
                },
                _,
                &Type::String,
            ) => {
                return Expression::AllocDynamicBytes {
                    loc: *loc,
                    ty: to.clone(),
                    size: Box::new(Expression::NumberLiteral {
                        loc: *loc,
                        ty: Type::Uint(32),
                        value: BigInt::from(init.len()),
                    }),
                    initializer: Some(init.clone()),
                };
            }
            (Expression::NumberLiteral { value, .. }, _, &Type::Rational) => {
                return Expression::RationalNumberLiteral {
                    loc: self.loc(),
                    ty: Type::Rational,
                    rational: BigRational::from(value.clone()),
                };
            }

            _ => (),
        }

        let from = match (&from, to) {
            (Type::Enum(enum_no), Type::Uint(_)) | (Type::Enum(enum_no), Type::Int(_)) => {
                let enum_ty = &ns.enums[*enum_no];
                let from_width = enum_ty.ty.bits(ns);
                Type::Uint(from_width)
            }

            (Type::Value, Type::Uint(_)) | (Type::Value, Type::Int(_)) => {
                let from_len = (ns.value_length as u16) * 8;
                Type::Int(from_len)
            }

            _ => from,
        };

        match (&from, to) {
            (Type::Uint(from_width), Type::Enum(enum_no))
            | (Type::Int(from_width), Type::Enum(enum_no)) => {
                let enum_ty = &ns.enums[*enum_no];
                // Not checking eval const number
                let to_width = enum_ty.ty.bits(ns);
                match from_width.cmp(&to_width) {
                    Ordering::Greater => Expression::Trunc {
                        loc: self.loc(),
                        ty: to.clone(),
                        expr: Box::new(self.clone()),
                    },
                    Ordering::Less => Expression::ZeroExt {
                        loc: self.loc(),
                        ty: to.clone(),
                        expr: Box::new(self.clone()),
                    },
                    Ordering::Equal => Expression::Cast {
                        loc: self.loc(),
                        ty: to.clone(),
                        expr: Box::new(self.clone()),
                    },
                }
            }

            (Type::Bytes(1), Type::Uint(8)) | (Type::Uint(8), Type::Bytes(1)) => self.clone(),

            (Type::Uint(from_len), Type::Uint(to_len))
            | (Type::Uint(from_len), Type::Int(to_len)) => match from_len.cmp(to_len) {
                Ordering::Greater => Expression::Trunc {
                    loc: self.loc(),
                    ty: to.clone(),
                    expr: Box::new(self.clone()),
                },
                Ordering::Less => Expression::ZeroExt {
                    loc: self.loc(),
                    ty: to.clone(),
                    expr: Box::new(self.clone()),
                },
                Ordering::Equal => Expression::Cast {
                    loc: self.loc(),
                    ty: to.clone(),
                    expr: Box::new(self.clone()),
                },
            },

            (Type::Int(from_len), Type::Uint(to_len))
            | (Type::Int(from_len), Type::Int(to_len)) => match from_len.cmp(to_len) {
                Ordering::Greater => Expression::Trunc {
                    loc: self.loc(),
                    ty: to.clone(),
                    expr: Box::new(self.clone()),
                },
                Ordering::Less => Expression::SignExt {
                    loc: self.loc(),
                    ty: to.clone(),
                    expr: Box::new(self.clone()),
                },
                Ordering::Equal => Expression::Cast {
                    loc: self.loc(),
                    ty: to.clone(),
                    expr: Box::new(self.clone()),
                },
            },

            (Type::Uint(from_len), Type::Address(_)) | (Type::Int(from_len), Type::Address(_)) => {
                let address_to_int = if from.is_signed_int(ns) {
                    Type::Int(address_bits)
                } else {
                    Type::Uint(address_bits)
                };

                let expr = match from_len.cmp(&address_bits) {
                    Ordering::Greater => Expression::Trunc {
                        loc: self.loc(),
                        ty: address_to_int,
                        expr: Box::new(self.clone()),
                    },
                    Ordering::Less if from.is_signed_int(ns) => Expression::ZeroExt {
                        loc: self.loc(),
                        ty: to.clone(),
                        expr: Box::new(self.clone()),
                    },
                    Ordering::Less => Expression::SignExt {
                        loc: self.loc(),
                        ty: to.clone(),
                        expr: Box::new(self.clone()),
                    },
                    Ordering::Equal => self.clone(),
                };

                Expression::Cast {
                    loc: self.loc(),
                    ty: to.clone(),
                    expr: Box::new(expr),
                }
            }
            (Type::Address(_), Type::Uint(to_len)) | (Type::Address(_), Type::Int(to_len)) => {
                let address_to_int = if to.is_signed_int(ns) {
                    Type::Int(address_bits)
                } else {
                    Type::Uint(address_bits)
                };

                let expr = Expression::Cast {
                    loc: self.loc(),
                    ty: address_to_int,
                    expr: Box::new(self.clone()),
                };

                // now resize int to request size with sign extension etc
                match to_len.cmp(&address_bits) {
                    Ordering::Less => Expression::Trunc {
                        loc: self.loc(),
                        ty: to.clone(),
                        expr: Box::new(expr),
                    },
                    Ordering::Greater if to.is_signed_int(ns) => Expression::ZeroExt {
                        loc: self.loc(),
                        ty: to.clone(),
                        expr: Box::new(expr),
                    },
                    Ordering::Greater => Expression::SignExt {
                        loc: self.loc(),
                        ty: to.clone(),
                        expr: Box::new(expr),
                    },
                    Ordering::Equal => expr,
                }
            }
            (Type::Bytes(from_len), Type::Bytes(to_len)) => {
                if to_len > from_len {
                    let shift = (to_len - from_len) * 8;

                    Expression::ShiftLeft {
                        loc: self.loc(),
                        ty: to.clone(),
                        left: Box::new(Expression::ZeroExt {
                            loc: self.loc(),
                            ty: to.clone(),
                            expr: Box::new(self.clone()),
                        }),
                        right: Box::new(Expression::NumberLiteral {
                            loc: self.loc(),
                            ty: Type::Uint(*to_len as u16 * 8),
                            value: BigInt::from_u8(shift).unwrap(),
                        }),
                    }
                } else {
                    let shift = (from_len - to_len) * 8;

                    Expression::Trunc {
                        loc: self.loc(),
                        ty: to.clone(),
                        expr: Box::new(Expression::ShiftRight {
                            loc: self.loc(),
                            ty: from.clone(),
                            left: Box::new(self.clone()),
                            right: Box::new(Expression::NumberLiteral {
                                loc: self.loc(),
                                ty: Type::Uint(*from_len as u16 * 8),
                                value: BigInt::from_u8(shift).unwrap(),
                            }),
                            signed: false,
                        }),
                    }
                }
            }
            // Conversion from rational will never happen in codegen
            (Type::Uint(_) | Type::Int(_) | Type::Value, Type::Rational) => Expression::Cast {
                loc: self.loc(),
                ty: to.clone(),
                expr: Box::new(self.clone()),
            },

            (Type::Bytes(_), Type::DynamicBytes) | (Type::DynamicBytes, Type::Bytes(_)) => {
                Expression::BytesCast {
                    loc: self.loc(),
                    ty: from.clone(),
                    from: to.clone(),
                    expr: Box::new(self.clone()),
                }
            }

            (Type::Bool, Type::Int(_) | Type::Uint(_)) => Expression::Cast {
                loc: self.loc(),
                ty: to.clone(),
                expr: Box::new(self.clone()),
            },

            (Type::Int(_) | Type::Uint(_), Type::Bool) => Expression::NotEqual {
                loc: self.loc(),
                left: Box::new(Expression::NumberLiteral {
                    loc: self.loc(),
                    ty: self.ty(),
                    value: BigInt::zero(),
                }),
                right: Box::new(self.clone()),
            },

            (Type::Bytes(n), Type::Uint(bits) | Type::Int(bits)) => {
                let num_bytes = (bits / 8) as u8;
                match n.cmp(&num_bytes) {
                    Ordering::Greater => Expression::Trunc {
                        loc: self.loc(),
                        ty: to.clone(),
                        expr: Box::new(self.clone()),
                    },
                    Ordering::Less => Expression::ZeroExt {
                        loc: self.loc(),
                        ty: to.clone(),
                        expr: Box::new(self.clone()),
                    },
                    Ordering::Equal => Expression::Cast {
                        loc: self.loc(),
                        ty: to.clone(),
                        expr: Box::new(self.clone()),
                    },
                }
            }

            (Type::FunctionSelector, _) => Expression::Cast {
                loc: self.loc(),
                ty: Type::Bytes(ns.target.selector_length()),
                expr: self.clone().into(),
            }
            .cast(to, ns),

            (_, Type::FunctionSelector) => self.cast(&Type::Bytes(ns.target.selector_length()), ns),

            (Type::Uint(_), Type::Bytes(_))
            | (Type::Int(_), Type::Bytes(_))
            | (Type::Bytes(_), Type::Address(_))
            | (Type::Address(false), Type::Address(true))
            | (Type::Address(_), Type::Contract(_))
            | (Type::Contract(_), Type::Address(_))
            | (Type::Contract(_), Type::Contract(_))
            | (Type::Address(true), Type::Address(false))
            | (Type::Address(_), Type::Bytes(_))
            | (Type::String, Type::DynamicBytes)
            | (Type::DynamicBytes, Type::String)
            | (Type::InternalFunction { .. }, Type::InternalFunction { .. })
            | (Type::ExternalFunction { .. }, Type::ExternalFunction { .. }) => Expression::Cast {
                loc: self.loc(),
                ty: to.clone(),
                expr: Box::new(self.clone()),
            },

            _ if !from.is_contract_storage()
                && !to.is_contract_storage()
                && from.is_reference_type(ns)
                && !to.is_reference_type(ns) =>
            {
                let expr = Expression::Cast {
                    loc: self.loc(),
                    ty: Type::Uint(ns.target.ptr_size()),
                    expr: self.clone().into(),
                };

                expr.cast(to, ns)
            }

            _ if !from.is_contract_storage()
                && !to.is_contract_storage()
                && !from.is_reference_type(ns)
                && to.is_reference_type(ns) =>
            {
                // cast non-pointer to pointer
                let ptr_ty = Type::Uint(ns.target.ptr_size());

                Expression::Cast {
                    loc: self.loc(),
                    ty: to.clone(),
                    expr: self.cast(&ptr_ty, ns).into(),
                }
            }

            _ if !from.is_contract_storage()
                && !to.is_contract_storage()
                && !from.is_reference_type(ns)
                && !to.is_reference_type(ns) =>
            {
                // cast pointer to different pointer
                Expression::Cast {
                    loc: self.loc(),
                    ty: to.clone(),
                    expr: self.clone().into(),
                }
            }

            _ => self.clone(),
        }
    }

    /// Recurse over expression and copy each element through a filter. This allows the optimizer passes to create
    /// copies of expressions while modifying the results slightly
    #[must_use]
    pub fn copy_filter<T, F>(&self, ctx: &mut T, filter: F) -> Expression
    where
        F: Fn(&Expression, &mut T) -> Expression,
    {
        filter(
            &match self {
                Expression::StructLiteral { loc, ty, values } => Expression::StructLiteral {
                    loc: *loc,
                    ty: ty.clone(),
                    values: values.iter().map(|e| filter(e, ctx)).collect(),
                },
                Expression::ArrayLiteral {
                    loc,
                    ty,
                    dimensions,
                    values,
                } => Expression::ArrayLiteral {
                    loc: *loc,
                    ty: ty.clone(),
                    dimensions: dimensions.clone(),
                    values: values.iter().map(|e| filter(e, ctx)).collect(),
                },
                Expression::ConstArrayLiteral {
                    loc,
                    ty,
                    dimensions,
                    values,
                } => Expression::ConstArrayLiteral {
                    loc: *loc,
                    ty: ty.clone(),
                    dimensions: dimensions.clone(),
                    values: values.iter().map(|e| filter(e, ctx)).collect(),
                },
                Expression::Add {
                    loc,
                    ty,
                    overflowing,
                    left,
                    right,
                } => Expression::Add {
                    loc: *loc,
                    ty: ty.clone(),
                    overflowing: *overflowing,
                    left: Box::new(filter(left, ctx)),
                    right: Box::new(filter(right, ctx)),
                },
                Expression::Subtract {
                    loc,
                    ty,
                    overflowing,
                    left,
                    right,
                } => Expression::Subtract {
                    loc: *loc,
                    ty: ty.clone(),
                    overflowing: *overflowing,
                    left: Box::new(filter(left, ctx)),
                    right: Box::new(filter(right, ctx)),
                },
                Expression::Multiply {
                    loc,
                    ty,
                    overflowing,
                    left,
                    right,
                } => Expression::Multiply {
                    loc: *loc,
                    ty: ty.clone(),
                    overflowing: *overflowing,
                    left: Box::new(filter(left, ctx)),
                    right: Box::new(filter(right, ctx)),
                },
                Expression::UnsignedDivide {
                    loc,
                    ty,
                    left,
                    right,
                } => Expression::UnsignedDivide {
                    loc: *loc,
                    ty: ty.clone(),
                    left: Box::new(filter(left, ctx)),
                    right: Box::new(filter(right, ctx)),
                },
                Expression::SignedDivide {
                    loc,
                    ty,
                    left,
                    right,
                } => Expression::SignedDivide {
                    loc: *loc,
                    ty: ty.clone(),
                    left: Box::new(filter(left, ctx)),
                    right: Box::new(filter(right, ctx)),
                },
                Expression::Power {
                    loc,
                    ty,
                    overflowing,
                    base,
                    exp,
                } => Expression::Power {
                    loc: *loc,
                    ty: ty.clone(),
                    overflowing: *overflowing,
                    base: Box::new(filter(base, ctx)),
                    exp: Box::new(filter(exp, ctx)),
                },
                Expression::BitwiseOr {
                    loc,
                    ty,
                    left,
                    right,
                } => Expression::BitwiseOr {
                    loc: *loc,
                    ty: ty.clone(),
                    left: Box::new(filter(left, ctx)),
                    right: Box::new(filter(right, ctx)),
                },
                Expression::BitwiseAnd {
                    loc,
                    ty,
                    left,
                    right,
                } => Expression::BitwiseAnd {
                    loc: *loc,
                    ty: ty.clone(),
                    left: Box::new(filter(left, ctx)),
                    right: Box::new(filter(right, ctx)),
                },
                Expression::BitwiseXor {
                    loc,
                    ty,
                    left,
                    right,
                } => Expression::BitwiseXor {
                    loc: *loc,
                    ty: ty.clone(),
                    left: Box::new(filter(left, ctx)),
                    right: Box::new(filter(right, ctx)),
                },
                Expression::ShiftLeft {
                    loc,
                    ty,
                    left,
                    right,
                } => Expression::ShiftLeft {
                    loc: *loc,
                    ty: ty.clone(),
                    left: Box::new(filter(left, ctx)),
                    right: Box::new(filter(right, ctx)),
                },
                Expression::ShiftRight {
                    loc,
                    ty,
                    left,
                    right,
                    signed: sign_extend,
                } => Expression::ShiftRight {
                    loc: *loc,
                    ty: ty.clone(),
                    left: Box::new(filter(left, ctx)),
                    right: Box::new(filter(right, ctx)),
                    signed: *sign_extend,
                },
                Expression::Load { loc, ty, expr } => Expression::Load {
                    loc: *loc,
                    ty: ty.clone(),
                    expr: Box::new(filter(expr, ctx)),
                },
                Expression::ZeroExt { loc, ty, expr } => Expression::ZeroExt {
                    loc: *loc,
                    ty: ty.clone(),
                    expr: Box::new(filter(expr, ctx)),
                },
                Expression::SignExt { loc, ty, expr } => Expression::SignExt {
                    loc: *loc,
                    ty: ty.clone(),
                    expr: Box::new(filter(expr, ctx)),
                },
                Expression::Trunc { loc, ty, expr } => Expression::Trunc {
                    loc: *loc,
                    ty: ty.clone(),
                    expr: Box::new(filter(expr, ctx)),
                },
                Expression::Cast { loc, ty, expr } => Expression::Cast {
                    loc: *loc,
                    ty: ty.clone(),
                    expr: Box::new(filter(expr, ctx)),
                },
                Expression::BytesCast {
                    loc,
                    ty,
                    from,
                    expr,
                } => Expression::BytesCast {
                    loc: *loc,
                    ty: ty.clone(),
                    from: from.clone(),
                    expr: Box::new(filter(expr, ctx)),
                },
                Expression::More {
                    loc,
                    signed,
                    left,
                    right,
                } => Expression::More {
                    loc: *loc,
                    signed: *signed,
                    left: Box::new(filter(left, ctx)),
                    right: Box::new(filter(right, ctx)),
                },
                Expression::Less {
                    loc,
                    signed,
                    left,
                    right,
                } => Expression::Less {
                    loc: *loc,
                    signed: *signed,
                    left: Box::new(filter(left, ctx)),
                    right: Box::new(filter(right, ctx)),
                },
                Expression::MoreEqual {
                    loc,
                    signed,
                    left,
                    right,
                } => Expression::MoreEqual {
                    loc: *loc,
                    signed: *signed,
                    left: Box::new(filter(left, ctx)),
                    right: Box::new(filter(right, ctx)),
                },
                Expression::LessEqual {
                    loc,
                    signed,
                    left,
                    right,
                } => Expression::LessEqual {
                    loc: *loc,
                    signed: *signed,
                    left: Box::new(filter(left, ctx)),
                    right: Box::new(filter(right, ctx)),
                },
                Expression::Equal { loc, left, right } => Expression::Equal {
                    loc: *loc,
                    left: Box::new(filter(left, ctx)),
                    right: Box::new(filter(right, ctx)),
                },
                Expression::NotEqual { loc, left, right } => Expression::NotEqual {
                    loc: *loc,
                    left: Box::new(filter(left, ctx)),
                    right: Box::new(filter(right, ctx)),
                },
                Expression::AdvancePointer {
                    pointer,
                    bytes_offset: offset,
                } => Expression::AdvancePointer {
                    pointer: Box::new(filter(pointer, ctx)),
                    bytes_offset: Box::new(filter(offset, ctx)),
                },
                Expression::Not { loc, expr } => Expression::Not {
                    loc: *loc,
                    expr: Box::new(filter(expr, ctx)),
                },
                Expression::BitwiseNot { loc, ty, expr } => Expression::BitwiseNot {
                    loc: *loc,
                    ty: ty.clone(),
                    expr: Box::new(filter(expr, ctx)),
                },
                Expression::Negate {
                    loc,
                    ty,
                    overflowing,
                    expr,
                } => Expression::Negate {
                    loc: *loc,
                    ty: ty.clone(),
                    overflowing: *overflowing,
                    expr: Box::new(filter(expr, ctx)),
                },
                Expression::Subscript {
                    loc,
                    ty: elem_ty,
                    array_ty,
                    expr,
                    index,
                } => Expression::Subscript {
                    loc: *loc,
                    ty: elem_ty.clone(),
                    array_ty: array_ty.clone(),
                    expr: Box::new(filter(expr, ctx)),
                    index: Box::new(filter(index, ctx)),
                },
                Expression::StructMember {
                    loc,
                    ty,
                    expr,
                    member,
                } => Expression::StructMember {
                    loc: *loc,
                    ty: ty.clone(),
                    expr: Box::new(filter(expr, ctx)),
                    member: *member,
                },
                Expression::AllocDynamicBytes {
                    loc,
                    ty,
                    size,
                    initializer,
                } => Expression::AllocDynamicBytes {
                    loc: *loc,
                    ty: ty.clone(),
                    size: Box::new(filter(size, ctx)),
                    initializer: initializer.clone(),
                },
                Expression::StorageArrayLength {
                    loc,
                    ty,
                    array,
                    elem_ty,
                } => Expression::StorageArrayLength {
                    loc: *loc,
                    ty: ty.clone(),
                    array: Box::new(filter(array, ctx)),
                    elem_ty: elem_ty.clone(),
                },
                Expression::StringCompare { loc, left, right } => Expression::StringCompare {
                    loc: *loc,
                    left: match left {
                        StringLocation::CompileTime(_) => left.clone(),
                        StringLocation::RunTime(expr) => {
                            StringLocation::RunTime(Box::new(filter(expr, ctx)))
                        }
                    },
                    right: match right {
                        StringLocation::CompileTime(_) => right.clone(),
                        StringLocation::RunTime(expr) => {
                            StringLocation::RunTime(Box::new(filter(expr, ctx)))
                        }
                    },
                },
                Expression::FormatString { loc, args } => {
                    let args = args.iter().map(|(f, e)| (*f, filter(e, ctx))).collect();

                    Expression::FormatString { loc: *loc, args }
                }
                Expression::Builtin {
                    loc,
                    tys,
                    kind: builtin,
                    args,
                } => {
                    let args = args.iter().map(|e| filter(e, ctx)).collect();

                    Expression::Builtin {
                        loc: *loc,
                        tys: tys.clone(),
                        kind: *builtin,
                        args,
                    }
                }
                _ => self.clone(),
            },
            ctx,
        )
    }

    fn external_function_selector(&self) -> Expression {
        debug_assert!(
            matches!(self.ty().deref_any(), Type::ExternalFunction { .. }),
            "This is not an external function"
        );
        let loc = self.loc();
        let struct_member = Expression::StructMember {
            loc,
            ty: Type::Ref(Box::new(Type::FunctionSelector)),
            expr: Box::new(self.clone()),
            member: 0,
        };
        Expression::Load {
            loc,
            ty: Type::FunctionSelector,
            expr: Box::new(struct_member),
        }
    }

    fn external_function_address(&self) -> Expression {
        debug_assert!(
            matches!(self.ty(), Type::ExternalFunction { .. }),
            "This is not an external function"
        );
        let loc = self.loc();
        let struct_member = Expression::StructMember {
            loc,
            ty: Type::Ref(Box::new(Type::Address(false))),
            expr: Box::new(self.clone()),
            member: 1,
        };
        Expression::Load {
            loc,
            ty: Type::Address(false),
            expr: Box::new(struct_member),
        }
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum Builtin {
    Accounts,
    AddMod,
    ArrayLength,
    Balance,
    Blake2_128,
    Blake2_256,
    BlockCoinbase,
    BlockDifficulty,
    BlockHash,
    BlockNumber,
    Calldata,
    ChainId,
    ContractCode,
    Gasleft,
    GasLimit,
    Gasprice,
    BaseFee,
    PrevRandao,
    /// GetAddress returns a pointer to the address. On Polkadot, this pointer points to the
    /// scratch buffer, to which many syscall write. We strongly recommend loading the pointer
    /// before using on Polkadot. This is not the case for Solana, though.
    GetAddress,
    ExtCodeSize,
    MinimumBalance,
    MulMod,
    Keccak256,
    Origin,
    ReadFromBuffer,
    Ripemd160,
    Sender,
    Slot,
    Sha256,
    Signature,
    SignatureVerify,
    Timestamp,
    Value,
    WriteAddress,
    WriteInt8,
    WriteInt16LE,
    WriteInt32LE,
    WriteInt64LE,
    WriteInt128LE,
    WriteInt256LE,
    WriteUint16LE,
    WriteUint32LE,
    WriteUint64LE,
    WriteUint128LE,
    WriteUint256LE,
    WriteBytes,
    Concat,
}

impl From<&ast::Builtin> for Builtin {
    fn from(ast_builtin: &ast::Builtin) -> Self {
        match ast_builtin {
            ast::Builtin::Accounts => Builtin::Accounts,
            ast::Builtin::AddMod => Builtin::AddMod,
            ast::Builtin::ArrayLength => Builtin::ArrayLength,
            ast::Builtin::Balance => Builtin::Balance,
            ast::Builtin::Blake2_128 => Builtin::Blake2_128,
            ast::Builtin::Blake2_256 => Builtin::Blake2_256,
            ast::Builtin::BlockCoinbase => Builtin::BlockCoinbase,
            ast::Builtin::BlockDifficulty => Builtin::BlockDifficulty,
            ast::Builtin::BlockHash => Builtin::BlockHash,
            ast::Builtin::BlockNumber => Builtin::BlockNumber,
            ast::Builtin::Calldata => Builtin::Calldata,
            ast::Builtin::Gasleft => Builtin::Gasleft,
            ast::Builtin::GasLimit => Builtin::GasLimit,
            ast::Builtin::Gasprice => Builtin::Gasprice,
            ast::Builtin::GetAddress => Builtin::GetAddress,
            ast::Builtin::MinimumBalance => Builtin::MinimumBalance,
            ast::Builtin::MulMod => Builtin::MulMod,
            ast::Builtin::Keccak256 => Builtin::Keccak256,
            ast::Builtin::Origin => Builtin::Origin,
            ast::Builtin::ReadAddress
            | ast::Builtin::ReadInt8
            | ast::Builtin::ReadInt16LE
            | ast::Builtin::ReadInt32LE
            | ast::Builtin::ReadInt64LE
            | ast::Builtin::ReadInt128LE
            | ast::Builtin::ReadInt256LE
            | ast::Builtin::ReadUint16LE
            | ast::Builtin::ReadUint32LE
            | ast::Builtin::ReadUint64LE
            | ast::Builtin::ReadUint128LE
            | ast::Builtin::ReadUint256LE => Builtin::ReadFromBuffer,
            ast::Builtin::Ripemd160 => Builtin::Ripemd160,
            ast::Builtin::Sender => Builtin::Sender,
            ast::Builtin::Slot => Builtin::Slot,
            ast::Builtin::Sha256 => Builtin::Sha256,
            ast::Builtin::Signature => Builtin::Signature,
            ast::Builtin::SignatureVerify => Builtin::SignatureVerify,
            ast::Builtin::Timestamp => Builtin::Timestamp,
            ast::Builtin::Value => Builtin::Value,
            ast::Builtin::WriteAddress => Builtin::WriteAddress,
            ast::Builtin::WriteInt8 => Builtin::WriteInt8,
            ast::Builtin::WriteInt16LE => Builtin::WriteInt16LE,
            ast::Builtin::WriteInt32LE => Builtin::WriteInt32LE,
            ast::Builtin::WriteInt64LE => Builtin::WriteInt64LE,
            ast::Builtin::WriteInt128LE => Builtin::WriteInt128LE,
            ast::Builtin::WriteInt256LE => Builtin::WriteInt256LE,
            ast::Builtin::WriteUint16LE => Builtin::WriteUint16LE,
            ast::Builtin::WriteUint32LE => Builtin::WriteUint32LE,
            ast::Builtin::WriteUint64LE => Builtin::WriteUint64LE,
            ast::Builtin::WriteUint128LE => Builtin::WriteUint128LE,
            ast::Builtin::WriteUint256LE => Builtin::WriteUint256LE,
            ast::Builtin::WriteBytes | ast::Builtin::WriteString => Builtin::WriteBytes,
            ast::Builtin::ChainId => Builtin::ChainId,
            ast::Builtin::BaseFee => Builtin::BaseFee,
            ast::Builtin::PrevRandao => Builtin::PrevRandao,
            ast::Builtin::ContractCode => Builtin::ContractCode,
            ast::Builtin::StringConcat | ast::Builtin::BytesConcat => Builtin::Concat,
            _ => panic!("Builtin should not be in the cfg"),
        }
    }
}
