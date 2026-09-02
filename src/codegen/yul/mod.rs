// SPDX-License-Identifier: Apache-2.0

use crate::codegen::cfg::{
    optimize_and_check_cfg, populate_arguments, populate_named_returns, ASTFunction,
    ControlFlowGraph, Instr,
};
use crate::codegen::interface::TargetCodegen;
use crate::codegen::statements::LoopScopes;
use crate::codegen::vartable::Vartable;
use crate::codegen::yul::statements::statement;
use crate::codegen::{Expression, Options};
use crate::sema::ast::Namespace;
use crate::sema::yul::ast::InlineAssembly;
use solang_parser::pt;
use solang_parser::pt::FunctionTy;

mod builtin;
mod expression;
mod statements;
mod tests;

/// Create the CFG instructions for inline assembly statements
pub fn inline_assembly_cfg(
    inline_assembly: &InlineAssembly,
    contract_no: usize,
    ns: &Namespace,
    cfg: &mut ControlFlowGraph,
    vartab: &mut Vartable,
    opt: &Options,
    target: &dyn TargetCodegen,
) {
    let mut loops = LoopScopes::new();
    for stmt in &inline_assembly.body {
        statement(
            stmt,
            contract_no,
            &mut loops,
            ns,
            cfg,
            vartab,
            &None,
            opt,
            target,
        );
    }
}

/// Create the CFG for yul functions
pub(crate) fn generate_yul_function_cfg(
    contract_no: usize,
    function_no: usize,
    all_cfgs: &mut [ControlFlowGraph],
    ns: &mut Namespace,
    opt: &Options,
    target: &dyn TargetCodegen,
) {
    let mut cfg = yul_function_cfg(contract_no, function_no, ns, opt, target);

    optimize_and_check_cfg(&mut cfg, ns, ASTFunction::YulFunction(function_no), opt);
    all_cfgs[ns.yul_functions[function_no].cfg_no] = cfg;
}

/// Generate the CFG containing all the instructions from a YUL function
fn yul_function_cfg(
    contract_no: usize,
    function_no: usize,
    ns: &mut Namespace,
    opt: &Options,
    target: &dyn TargetCodegen,
) -> ControlFlowGraph {
    let mut vartab =
        Vartable::from_symbol_table(&ns.yul_functions[function_no].symtable, ns.next_id);

    let mut loops = LoopScopes::new();
    let yul_func = &ns.yul_functions[function_no];

    let func_name = format!(
        "{}::yul_function_{}::{}",
        ns.contracts[contract_no].id, function_no, yul_func.name
    );
    let mut cfg = ControlFlowGraph::new(func_name, ASTFunction::YulFunction(function_no));

    cfg.params = yul_func.params.clone();
    cfg.returns = yul_func.returns.clone();
    cfg.selector = Vec::new();
    cfg.public = false;
    cfg.ty = FunctionTy::Function;
    cfg.nonpayable = true;

    // populate the arguments
    populate_arguments(yul_func, &mut cfg, &mut vartab);
    // populate the returns, if any
    populate_named_returns(yul_func, ns, &mut cfg, &mut vartab);

    let returns = if yul_func.returns.is_empty() {
        Instr::Return { value: vec![] }
    } else {
        Instr::Return {
            value: yul_func
                .symtable
                .returns
                .iter()
                .map(|pos| Expression::Variable {
                    loc: pt::Loc::Codegen,
                    ty: yul_func.symtable.vars[pos].ty.clone(),
                    var_no: *pos,
                })
                .collect::<Vec<Expression>>(),
        }
    };

    for stmt in &yul_func.body.statements {
        statement(
            stmt,
            contract_no,
            &mut loops,
            ns,
            &mut cfg,
            &mut vartab,
            &Some(returns.clone()),
            opt,
            target,
        );
    }

    if yul_func.body.is_next_reachable() {
        cfg.add(&mut vartab, returns);
    }

    vartab.finalize(ns, &mut cfg);
    cfg
}
