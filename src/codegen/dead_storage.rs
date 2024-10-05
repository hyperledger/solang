// SPDX-License-Identifier: Apache-2.0

use super::cfg::{BasicBlock, ControlFlowGraph, Instr};
use crate::codegen::Expression;
use crate::sema::ast::{Namespace, RetrieveType, Type};
use solang_parser::pt::Loc;
use std::collections::{HashMap, HashSet};
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
enum Definition {
    Undefined,
    Instr {
        block_no: usize,
        instr_no: usize,
        assignment_no: usize,
    },
}

impl fmt::Display for Definition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Definition::Undefined => {
                write!(f, "undef")
            }
            Definition::Instr {
                block_no,
                instr_no,
                assignment_no,
            } => {
                write!(f, "({block_no}, {instr_no}, {assignment_no})")
            }
        }
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, PartialEq, Eq)]
enum Transfer {
    Gen {
        def: Definition,
        var_no: usize,
    },
    Copy {
        var_no: usize,
        src: usize,
    },
    Kill {
        var_no: usize,
    },
    Store {
        def: Definition,
        expr: Option<Expression>,
    },
}

impl fmt::Display for Transfer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Transfer::Gen { def, var_no } => {
                write!(f, "Gen %{var_no} = {def}")
            }
            Transfer::Copy { var_no, src } => {
                write!(f, "Copy %{var_no} from %{src}")
            }
            Transfer::Kill { var_no } => {
                write!(f, "Kill %{var_no}")
            }
            Transfer::Store { def, expr } => {
                write!(f, "Storage: {expr:?} at {def}")
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct ReachingDefs {
    vars: HashMap<usize, HashMap<Definition, Option<Expression>>>,
    stores: Vec<(Definition, Expression)>,
}

type BlockVars = HashMap<usize, Vec<ReachingDefs>>;

/// Calculate all the reaching definitions for the contract. This is a flow
/// analysis which is used for further optimizations
#[allow(clippy::map_entry)]
fn reaching_definitions(cfg: &mut ControlFlowGraph) -> (Vec<Vec<Vec<Transfer>>>, BlockVars) {
    // the transfers
    let mut block_transfers: Vec<Vec<Vec<Transfer>>> = Vec::new();
    let mut block_vars: BlockVars = HashMap::new();

    // calculate the per-instruction reaching defs
    for (block_no, block) in cfg.blocks.iter().enumerate() {
        let transfer = instr_transfers(block_no, block);

        debug_assert_eq!(block_no, block_transfers.len());

        block_transfers.push(transfer);

        debug_assert_eq!(
            cfg.blocks[block_no].instr.len(),
            block_transfers[block_no].len()
        );
    }

    let mut blocks_todo: HashSet<usize> = HashSet::new();
    blocks_todo.insert(0);

    while let Some(block_no) = blocks_todo.iter().next() {
        let block_no = *block_no;
        blocks_todo.remove(&block_no);

        let mut vars = if let Some(vars) = block_vars.get(&block_no) {
            vars[0].clone()
        } else {
            debug_assert_eq!(block_no, 0);

            // On entry all variables should be undefined
            let mut vars: HashMap<usize, HashMap<Definition, Option<Expression>>> = HashMap::new();

            for var_no in cfg.vars.keys() {
                let mut defs = HashMap::new();

                defs.insert(Definition::Undefined, None);

                vars.insert(*var_no, defs);
            }

            ReachingDefs {
                vars,
                stores: Vec::new(),
            }
        };

        apply_transfers(
            &block_transfers[block_no],
            &mut vars,
            cfg,
            block_no,
            &mut block_vars,
        );

        for edge in cfg.blocks[block_no].successors() {
            if !block_vars.contains_key(&edge) {
                blocks_todo.insert(edge);
                block_vars.insert(edge, vec![vars.clone()]);
            } else if block_vars[&edge][0] != vars {
                blocks_todo.insert(edge);
                let block_vars = block_vars
                    .get_mut(&edge)
                    .expect("block vars must contain edge");
                // merge incoming vars
                for (var_no, defs) in &vars.vars {
                    if let Some(entry) = block_vars[0].vars.get_mut(var_no) {
                        for (incoming_def, storage) in defs {
                            if !entry.contains_key(incoming_def) {
                                entry.insert(*incoming_def, storage.clone());
                            }
                        }
                    } else {
                        block_vars[0].vars.insert(*var_no, defs.clone());
                    }
                }

                // merge storage stores
                for store in &vars.stores {
                    if !block_vars[0].stores.iter().any(|(def, _)| *def == store.0) {
                        block_vars[0].stores.push(store.clone());
                    }
                }
            }
        }
    }

    (block_transfers, block_vars)
}

/// Instruction defs
fn instr_transfers(block_no: usize, block: &BasicBlock) -> Vec<Vec<Transfer>> {
    let mut transfers = Vec::new();

    for (instr_no, instr) in block.instr.iter().enumerate() {
        let def = Definition::Instr {
            block_no,
            instr_no,
            assignment_no: 0,
        };
        let set_var = |var_nos: &[usize]| {
            let mut transfer = Vec::new();

            for (assignment_no, var_no) in var_nos.iter().enumerate() {
                transfer.insert(0, Transfer::Kill { var_no: *var_no });

                transfer.push(Transfer::Gen {
                    def: Definition::Instr {
                        block_no,
                        instr_no,
                        assignment_no,
                    },
                    var_no: *var_no,
                });
            }

            transfer
        };

        transfers.push(match instr {
            Instr::Set {
                res,
                expr: Expression::Variable { var_no: src, .. },
                ..
            } => {
                vec![
                    Transfer::Kill { var_no: *res },
                    Transfer::Copy {
                        var_no: *res,
                        src: *src,
                    },
                ]
            }
            Instr::Set { res, .. } => set_var(&[*res]),
            Instr::Call { res, .. } => {
                // We don't know what this function does to storage, so clear all storage
                // possibly we should check if the function is pure/view and not clear storage references
                let mut v = set_var(res);

                v.push(Transfer::Store { def, expr: None });

                v
            }
            Instr::LoadStorage { res, .. } => set_var(&[*res]),
            Instr::PushMemory { array, res, .. } => {
                let mut v = set_var(&[*res]);
                v.push(Transfer::Kill { var_no: *array });

                v
            }
            Instr::PopMemory { array, .. } => {
                vec![Transfer::Kill { var_no: *array }]
            }
            Instr::ExternalCall {
                success: Some(res), ..
            }
            | Instr::Constructor {
                success: None, res, ..
            }
            | Instr::ValueTransfer {
                success: Some(res), ..
            } => {
                // A constructor/external call can call us back and modify storage
                vec![
                    Transfer::Kill { var_no: *res },
                    Transfer::Store { def, expr: None },
                ]
            }
            Instr::Store { dest, .. } => {
                let mut v = Vec::new();

                if let Some(var_no) = array_var(dest) {
                    v.push(Transfer::Kill { var_no });
                }

                v
            }
            Instr::Constructor {
                success: Some(success),
                res,
                ..
            } => {
                // A constructor can call us back and modify storage
                vec![
                    Transfer::Kill { var_no: *res },
                    Transfer::Kill { var_no: *success },
                    Transfer::Store { def, expr: None },
                ]
            }
            Instr::SetStorageBytes { storage, .. }
            | Instr::ClearStorage { storage, .. }
            | Instr::SetStorage { storage, .. } => {
                vec![Transfer::Store {
                    def,
                    expr: Some(storage.clone()),
                }]
            }
            Instr::PopStorage {
                res: Some(res),
                storage,
                ..
            }
            | Instr::PushStorage { res, storage, .. } => {
                vec![
                    Transfer::Kill { var_no: *res },
                    Transfer::Gen { def, var_no: *res },
                    Transfer::Store {
                        def,
                        expr: Some(storage.clone()),
                    },
                ]
            }
            Instr::Return { .. } => {
                vec![Transfer::Store { def, expr: None }]
            }
            _ => Vec::new(),
        });
    }

    debug_assert_eq!(transfers.len(), block.instr.len());

    transfers
}

fn array_var(expr: &Expression) -> Option<usize> {
    match expr {
        Expression::Variable { var_no, .. } => Some(*var_no),
        Expression::Subscript { expr, .. } | Expression::StructMember { expr, .. } => {
            array_var(expr)
        }
        _ => None,
    }
}

fn apply_transfers(
    transfers: &[Vec<Transfer>],
    vars: &mut ReachingDefs,
    cfg: &ControlFlowGraph,
    block_no: usize,
    block_vars: &mut BlockVars,
) {
    let mut res = Vec::new();

    debug_assert_eq!(transfers.len(), cfg.blocks[block_no].instr.len());

    // this is done in two passes. The first pass just deals with variables.
    // The second pass deals with storage stores

    // for each instruction
    for transfers in transfers {
        res.push(vars.clone());

        // each instruction has a list of transfers
        for transfer in transfers {
            match transfer {
                Transfer::Kill { var_no } => {
                    vars.vars.remove(var_no);
                }
                Transfer::Copy { var_no, src } => {
                    if let Some(defs) = vars.vars.get(src) {
                        let defs = defs.clone();

                        vars.vars.insert(*var_no, defs);
                    }
                }
                Transfer::Gen { var_no, def } => {
                    if let Some(entry) = vars.vars.get_mut(var_no) {
                        entry.insert(*def, None);
                    } else {
                        let mut v = HashMap::new();
                        v.insert(*def, None);
                        vars.vars.insert(*var_no, v);
                    }
                }
                // For the second pass
                Transfer::Store { .. } => (),
            }
        }
    }

    block_vars.insert(block_no, res.clone());

    *vars = res[0].clone();
    let mut res = Vec::new();

    // 2nd pass
    for transfers in transfers {
        res.push(vars.clone());

        // each instruction has a list of transfers
        for transfer in transfers {
            match transfer {
                Transfer::Kill { var_no } => {
                    vars.vars.remove(var_no);
                }
                Transfer::Copy { var_no, src } => {
                    if let Some(defs) = vars.vars.get(src) {
                        let defs = defs.clone();

                        vars.vars.insert(*var_no, defs);
                    }
                }
                Transfer::Gen { var_no, def } => {
                    if let Some(entry) = vars.vars.get_mut(var_no) {
                        entry.insert(*def, None);
                    } else {
                        let mut v = HashMap::new();
                        v.insert(*def, None);
                        vars.vars.insert(*var_no, v);
                    }
                }
                Transfer::Store { def, expr } => {
                    // store to contract storage. This should kill any equal
                    let mut eliminated_vars = Vec::new();

                    for (var_no, def) in vars.vars.iter() {
                        for def in def.keys() {
                            if let Some(storage_def) = get_storage_definition(def, cfg) {
                                if let Some(expr) = expr {
                                    let storage_vars = get_vars_at(def, block_vars);

                                    if expression_compare(
                                        expr,
                                        vars,
                                        storage_def.slot,
                                        &storage_vars,
                                        cfg,
                                        block_vars,
                                    ) != ExpressionCmp::NotEqual
                                    {
                                        eliminated_vars.push(*var_no);
                                    }
                                } else {
                                    // all storage references must be killed
                                    eliminated_vars.push(*var_no);
                                }
                            }
                        }
                    }

                    for var_no in eliminated_vars {
                        vars.vars.remove(&var_no);
                    }

                    // Now handle the reaching storage stores
                    if let Some(expr) = expr {
                        // all stores should are no longer reaching if they are clobbered by this store
                        let mut eliminated_stores = Vec::new();

                        for (no, (def, storage)) in vars.stores.iter().enumerate() {
                            let storage_vars = get_vars_at(def, block_vars);

                            if expression_compare(
                                expr,
                                vars,
                                storage,
                                &storage_vars,
                                cfg,
                                block_vars,
                            ) == ExpressionCmp::Equal
                            {
                                eliminated_stores.push(no);
                            }
                        }

                        for no in eliminated_stores.into_iter().rev() {
                            vars.stores.remove(no);
                        }

                        vars.stores.push((*def, expr.clone()));
                    } else {
                        // flush all reaching stores
                        vars.stores.truncate(0);
                    }
                }
            }
        }
    }

    assert_eq!(res.len(), transfers.len());
    assert_eq!(res.len(), cfg.blocks[block_no].instr.len());

    block_vars.insert(block_no, res);
}

/// Eliminate dead storage load/store.
pub fn dead_storage(cfg: &mut ControlFlowGraph, _ns: &mut Namespace) {
    // first calculate reaching definitions. We use a special case reaching definitions, which we track
    let (blocktransfers, block_vars) = reaching_definitions(cfg);

    let mut redundant_stores = HashMap::new();

    // for each block, instruction
    for block_no in 0..cfg.blocks.len() {
        for instr_no in 0..cfg.blocks[block_no].instr.len() {
            if !block_vars.contains_key(&block_no) {
                // do not consider unreachable blocks
                continue;
            }

            let vars = &block_vars[&block_no][instr_no];

            match &cfg.blocks[block_no].instr[instr_no] {
                Instr::LoadStorage {
                    res, ty, storage, ..
                } => {
                    // is there a definition which has the same storage expression
                    let mut found = None;

                    for var_no in vars.vars.keys() {
                        let defs = &vars.vars[var_no];

                        if defs.len() == 1 {
                            let (def, _) = defs.iter().next().unwrap();

                            if let Some(storage_def) = get_storage_definition(def, cfg) {
                                let def_vars = get_vars_at(def, &block_vars);

                                if expression_compare(
                                    storage,
                                    vars,
                                    storage_def.slot,
                                    &def_vars,
                                    cfg,
                                    &block_vars,
                                ) == ExpressionCmp::Equal
                                    && storage_def.var_no != *res
                                    && ty == storage_def.ty
                                {
                                    found = Some(var_no);
                                    break;
                                }
                            }
                        }
                    }

                    if let Some(var_no) = found {
                        cfg.blocks[block_no].instr[instr_no] = Instr::Set {
                            loc: Loc::Codegen,
                            res: *res,
                            expr: Expression::Variable {
                                loc: Loc::Codegen,
                                ty: ty.clone(),
                                var_no: *var_no,
                            },
                        };
                    } else {
                        for (def, expr) in &vars.stores {
                            let def_vars = get_vars_at(def, &block_vars);

                            if expression_compare(storage, vars, expr, &def_vars, cfg, &block_vars)
                                != ExpressionCmp::NotEqual
                            {
                                if let Some(entry) = redundant_stores.get_mut(def) {
                                    *entry = false;
                                }
                            }
                        }
                    }
                }
                Instr::PushStorage { storage, .. } | Instr::PopStorage { storage, .. } => {
                    for (def, expr) in &vars.stores {
                        let def_vars = get_vars_at(def, &block_vars);

                        if expression_compare(storage, vars, expr, &def_vars, cfg, &block_vars)
                            != ExpressionCmp::NotEqual
                        {
                            if let Some(entry) = redundant_stores.get_mut(def) {
                                *entry = false;
                            }
                        }
                    }
                }
                Instr::SetStorage { .. }
                | Instr::SetStorageBytes { .. }
                | Instr::ClearStorage { .. } => {
                    let def = Definition::Instr {
                        block_no,
                        instr_no,
                        assignment_no: 0,
                    };

                    // add an entry if there is not one there already
                    redundant_stores.entry(def).or_insert(true);
                }
                _ => (),
            }

            let transfers = &blocktransfers[block_no][instr_no];

            if transfers
                .iter()
                .any(|t| matches!(t, Transfer::Store { expr: None, .. }))
            {
                for (def, _) in &vars.stores {
                    // insert new entry or override existing one
                    redundant_stores.insert(*def, false);
                }
            }
        }
    }

    // remove all stores which are marked as still redundant
    for (def, redundant) in &redundant_stores {
        if *redundant {
            if let Definition::Instr {
                block_no, instr_no, ..
            } = def
            {
                // Function calls should never be eliminated from the CFG, as they might have side effects
                if !matches!(cfg.blocks[*block_no].instr[*instr_no], Instr::Call { .. }) {
                    cfg.blocks[*block_no].instr[*instr_no] = Instr::Nop;
                }
            }
        }
    }
}

struct StorageDef<'a> {
    var_no: usize,
    slot: &'a Expression,
    ty: &'a Type,
}

fn get_storage_definition<'a>(
    def: &Definition,
    cfg: &'a ControlFlowGraph,
) -> Option<StorageDef<'a>> {
    if let Definition::Instr {
        block_no, instr_no, ..
    } = def
    {
        match &cfg.blocks[*block_no].instr[*instr_no] {
            Instr::LoadStorage {
                storage, res, ty, ..
            } => Some(StorageDef {
                var_no: *res,
                slot: storage,
                ty,
            }),
            _ => None,
        }
    } else {
        None
    }
}

fn get_definition<'a>(
    def: &Definition,
    cfg: &'a ControlFlowGraph,
) -> Option<(&'a Expression, Type)> {
    if let Definition::Instr {
        block_no, instr_no, ..
    } = def
    {
        match &cfg.blocks[*block_no].instr[*instr_no] {
            Instr::LoadStorage { storage, ty, .. } => Some((storage, ty.clone())),
            Instr::Set { expr, .. } => Some((expr, expr.ty())),
            _ => None,
        }
    } else {
        None
    }
}

fn get_vars_at(def: &Definition, block_vars: &BlockVars) -> ReachingDefs {
    match def {
        Definition::Instr {
            block_no, instr_no, ..
        } => {
            let vars = if let Some(vars) = block_vars.get(block_no) {
                vars[*instr_no].clone()
            } else {
                ReachingDefs {
                    vars: HashMap::new(),
                    stores: Vec::new(),
                }
            };

            vars
        }
        Definition::Undefined => {
            unreachable!("cannot get reaching definitions for undefined");
        }
    }
}

#[derive(PartialEq, Eq, Copy, Clone, Debug)]
enum ExpressionCmp {
    Equal,
    NotEqual,
    Unknown,
}

/// Compare two expressions that express storage locations. There are a limited amount of expressions needed here
fn expression_compare(
    left: &Expression,
    left_vars: &ReachingDefs,
    right: &Expression,
    right_vars: &ReachingDefs,
    cfg: &ControlFlowGraph,
    block_vars: &BlockVars,
) -> ExpressionCmp {
    let v = match (left, right) {
        (
            Expression::NumberLiteral { value: left, .. },
            Expression::NumberLiteral { value: right, .. },
        ) => {
            if left == right {
                ExpressionCmp::Equal
            } else {
                ExpressionCmp::NotEqual
            }
        }
        (Expression::Keccak256 { exprs: left, .. }, Expression::Keccak256 { exprs: right, .. }) => {
            // This could be written with fold_first() rather than collect(), but that is an unstable feature.
            // Also fold first does not short circuit
            let cmps: Vec<ExpressionCmp> = left
                .iter()
                .zip(right.iter())
                .map(|(left, right)| {
                    expression_compare(left, left_vars, right, right_vars, cfg, block_vars)
                })
                .collect();

            let first = cmps[0];

            if cmps.into_iter().any(|c| c != first) {
                ExpressionCmp::Unknown
            } else {
                first
            }
        }
        (Expression::ZeroExt { expr: left, .. }, Expression::ZeroExt { expr: right, .. })
        | (Expression::Trunc { expr: left, .. }, Expression::Trunc { expr: right, .. }) => {
            expression_compare(left, left_vars, right, right_vars, cfg, block_vars)
        }
        (
            Expression::FunctionArg { arg_no: left, .. },
            Expression::FunctionArg { arg_no: right, .. },
        ) => {
            if left == right {
                ExpressionCmp::Equal
            } else {
                // two function arguments can have the same value
                ExpressionCmp::Unknown
            }
        }
        (
            Expression::Add {
                left: l1,
                right: r1,
                ..
            },
            Expression::Add {
                left: l2,
                right: r2,
                ..
            },
        )
        | (
            Expression::Multiply {
                left: l1,
                right: r1,
                ..
            },
            Expression::Multiply {
                left: l2,
                right: r2,
                ..
            },
        )
        | (
            Expression::Subtract {
                left: l1,
                right: r1,
                ..
            },
            Expression::Subtract {
                left: l2,
                right: r2,
                ..
            },
        )
        | (
            Expression::Subscript {
                expr: l1,
                index: r1,
                ..
            },
            Expression::Subscript {
                expr: l2,
                index: r2,
                ..
            },
        ) => {
            let l = expression_compare(l1, left_vars, l2, right_vars, cfg, block_vars);

            let r = expression_compare(r1, left_vars, r2, right_vars, cfg, block_vars);

            if l == r {
                l
            } else if (l == ExpressionCmp::Equal && r == ExpressionCmp::NotEqual)
                || (l == ExpressionCmp::NotEqual && r == ExpressionCmp::Equal)
            {
                ExpressionCmp::NotEqual
            } else {
                ExpressionCmp::Unknown
            }
        }
        (Expression::Variable { var_no: left, .. }, Expression::Variable { var_no: right, .. }) => {
            // let's check that the variable left has the same reaching definitions as right
            let left = match left_vars.vars.get(left) {
                Some(left) => left,
                None => {
                    return ExpressionCmp::Unknown;
                }
            };
            let right = match right_vars.vars.get(right) {
                Some(right) => right,
                None => {
                    return ExpressionCmp::Unknown;
                }
            };

            if left == right {
                ExpressionCmp::Equal
            } else if left.len() == 1 && right.len() == 1 {
                let left = left.iter().next().unwrap();
                let right = right.iter().next().unwrap();

                match (get_definition(left.0, cfg), get_definition(right.0, cfg)) {
                    (Some((left_expr, left_ty)), Some((right_expr, right_ty))) => {
                        let left_vars = get_vars_at(left.0, block_vars);
                        let right_vars = get_vars_at(right.0, block_vars);

                        let cmp = expression_compare(
                            left_expr,
                            &left_vars,
                            right_expr,
                            &right_vars,
                            cfg,
                            block_vars,
                        );

                        if cmp == ExpressionCmp::Equal && left_ty != right_ty {
                            ExpressionCmp::Unknown
                        } else {
                            cmp
                        }
                    }
                    _ => ExpressionCmp::Unknown,
                }
            } else {
                ExpressionCmp::Unknown
            }
        }
        _ => ExpressionCmp::Unknown,
    };

    v
}
