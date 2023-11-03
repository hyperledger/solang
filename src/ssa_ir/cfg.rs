// SPDX-License-Identifier: Apache-2.0

use crate::codegen::cfg::ASTFunction;
use crate::pt::FunctionTy;
use crate::sema::ast::Parameter;
use crate::ssa_ir::instructions::Instruction;
use crate::ssa_ir::vartable::Vartable;

use super::ssa_type::Type;

#[derive(Debug)]
pub struct Cfg {
    pub name: String,
    pub function_no: ASTFunction,
    pub params: Vec<Parameter<Type>>,
    pub returns: Vec<Parameter<Type>>,
    pub vartable: Vartable,
    pub blocks: Vec<Block>,
    pub nonpayable: bool,
    pub public: bool,
    pub ty: FunctionTy,
    /// used to match the function in the contract
    pub selector: Vec<u8>,
}

#[derive(Debug)]
pub struct Block {
    pub name: String,
    pub instructions: Vec<Instruction>,
}
