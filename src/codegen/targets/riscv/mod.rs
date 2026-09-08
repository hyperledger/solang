// SPDX-License-Identifier: Apache-2.0

pub(crate) mod dispatch;
pub(crate) mod encoding;

use crate::codegen::cfg::ControlFlowGraph;
use crate::codegen::interface::{EventEmitter, TargetCodegen};
use crate::codegen::storage::{array_pop, array_push};
use crate::codegen::vartable::Vartable;
use crate::codegen::{Expression, Options};
use crate::sema::ast::{self, Function, Namespace, StructType, Type};
use num_bigint::BigInt;
use solang_parser::pt::{self, Loc};

pub(crate) struct RiscvEventEmitter;

impl EventEmitter for RiscvEventEmitter {
    fn emit(
        &self,
        _contract_no: usize,
        _func: &crate::sema::ast::Function,
        _cfg: &mut ControlFlowGraph,
        _vartab: &mut Vartable,
        _opt: &Options,
        _target: &dyn TargetCodegen,
    ) {
        // TODO: implement real RISC-V / r55 event emission semantics
    }

    fn selector(&self, _emitting_contract_no: usize) -> Vec<u8> {
        // TODO: implement real RISC-V / r55 event selector semantics
        vec![]
    }
}

pub(crate) struct RiscvTarget;

impl TargetCodegen for RiscvTarget {
    fn function_dispatch(
        &self,
        contract_no: usize,
        all_cfg: &mut [ControlFlowGraph],
        ns: &mut Namespace,
        opt: &Options,
    ) -> Vec<ControlFlowGraph> {
        dispatch::function_dispatch(contract_no, all_cfg, ns, opt)
    }

    fn post_process_program(&self, _ns: &mut Namespace, _opt: &Options) {
        // println!("=== RISC-V AST ===");
        // for contract in &ns.contracts {
        //     println!("{:#?}", contract);
        // }
        //
        // println!("=== RISC-V CFG ===");
        // for contract in &ns.contracts {
        //     print!("{}", contract.print_cfg(ns));
        // }
    }

    fn lower_storage_array_length(
        &self,
        loc: &Loc,
        ty: &Type,
        array: Expression,
        elem_ty: &Type,
        _cfg: &mut ControlFlowGraph,
        _vartab: &mut Vartable,
        _ns: &Namespace,
    ) -> Expression {
        // TODO: implement real RISC-V / r55 storage array length semantics
        Expression::StorageArrayLength {
            loc: *loc,
            ty: ty.clone(),
            array: Box::new(array),
            elem_ty: elem_ty.clone(),
        }
    }

    fn storage_array_push(
        &self,
        loc: &Loc,
        args: &[ast::Expression],
        cfg: &mut ControlFlowGraph,
        contract_no: usize,
        func: Option<&Function>,
        ns: &Namespace,
        vartab: &mut Vartable,
        opt: &Options,
    ) -> Expression {
        // TODO: implement real RISC-V / r55 storage array push semantics
        array_push(loc, args, cfg, contract_no, func, ns, vartab, opt, self)
    }

    fn storage_array_pop(
        &self,
        loc: &Loc,
        args: &[ast::Expression],
        return_ty: &Type,
        cfg: &mut ControlFlowGraph,
        contract_no: usize,
        func: Option<&Function>,
        ns: &Namespace,
        vartab: &mut Vartable,
        opt: &Options,
    ) -> Expression {
        // TODO: implement real RISC-V / r55 storage array pop semantics
        array_pop(
            loc,
            args,
            return_ty,
            cfg,
            contract_no,
            func,
            ns,
            vartab,
            opt,
            self,
        )
    }

    fn event_emitter<'a>(
        &self,
        _loc: &pt::Loc,
        _event_no: usize,
        _args: &'a [ast::Expression],
        _ns: &'a Namespace,
    ) -> Box<dyn EventEmitter + 'a> {
        // TODO: implement real RISC-V / r55 event emitter
        Box::new(RiscvEventEmitter)
    }

    fn lower_storage_struct_member(
        &self,
        loc: &Loc,
        var_expr: Expression,
        struct_ty: &StructType,
        field_no: usize,
        ns: &Namespace,
        _cfg: &mut ControlFlowGraph,
        _vartab: &mut Vartable,
    ) -> Expression {
        // TODO: implement real RISC-V / r55 struct member storage layout
        let offset: BigInt = struct_ty.definition(ns).fields[..field_no]
            .iter()
            .filter(|field| !field.infinite_size)
            .map(|field| field.ty.storage_slots(ns))
            .sum();
        Expression::Add {
            loc: *loc,
            ty: ns.storage_type(),
            overflowing: true,
            left: Box::new(var_expr),
            right: Box::new(Expression::NumberLiteral {
                loc: *loc,
                ty: ns.storage_type(),
                value: offset,
            }),
        }
    }
}
