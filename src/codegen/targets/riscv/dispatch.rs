// SPDX-License-Identifier: Apache-2.0

use crate::codegen::{
    cfg::{ASTFunction, ControlFlowGraph, Instr, InternalCallTy},
    encoding::{abi_decode, abi_encode},
    vartable::Vartable,
    Builtin, Expression, Options,
};
use crate::sema::ast::{Namespace, Parameter, Type, Type::Uint};
use num_bigint::{BigInt, Sign};
use solang_parser::pt::{FunctionTy, Loc::Codegen};
use std::fmt::{Display, Formatter, Result};

/// r55 runs a contract twice with two different binaries: the *deploy* binary
/// is executed by `CREATE` and receives the raw constructor arguments, and the
/// *runtime* binary is executed by every later `CALL` and receives the usual
/// selector-prefixed calldata.
///
/// This mirrors the Polkadot target, which likewise emits one dispatcher for
/// constructors and one for externally callable functions.
pub enum DispatchType {
    Deploy,
    Call,
}

impl Display for DispatchType {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::Deploy => f.write_str("riscv_deploy_dispatch"),
            Self::Call => f.write_str("riscv_call_dispatch"),
        }
    }
}

impl From<FunctionTy> for DispatchType {
    fn from(value: FunctionTy) -> Self {
        match value {
            FunctionTy::Constructor => Self::Deploy,
            FunctionTy::Function => Self::Call,
            _ => unreachable!("only constructors and functions have corresponding dispatch types"),
        }
    }
}

pub(crate) fn function_dispatch(
    _contract_no: usize,
    all_cfg: &[ControlFlowGraph],
    ns: &mut Namespace,
    opt: &Options,
) -> Vec<ControlFlowGraph> {
    vec![
        Dispatch::new(all_cfg, ns, opt, FunctionTy::Constructor).build(),
        Dispatch::new(all_cfg, ns, opt, FunctionTy::Function).build(),
    ]
}

struct Dispatch<'a> {
    start: usize,
    input_len: usize,
    /// Points at the calldata *after* the selector on the call dispatcher, and
    /// at the start of the constructor arguments on the deploy dispatcher.
    input_ptr: Expression,
    vartab: Vartable,
    cfg: ControlFlowGraph,
    all_cfg: &'a [ControlFlowGraph],
    ns: &'a mut Namespace,
    selector_len: Box<Expression>,
    #[allow(dead_code)]
    opt: &'a Options,
    ty: FunctionTy,
}

/// The dispatcher is called from the `_start` assembly stub, which passes a
/// pointer to the calldata payload and its length.
fn new_cfg(ty: FunctionTy) -> ControlFlowGraph {
    let mut cfg = ControlFlowGraph::new(DispatchType::from(ty).to_string(), ASTFunction::None);
    let input_ptr = Parameter {
        loc: Codegen,
        id: None,
        ty: Type::BufferPointer,
        ty_loc: None,
        indexed: false,
        readonly: true,
        infinite_size: false,
        recursive: false,
        annotation: None,
    };
    let mut input_len = input_ptr.clone();
    input_len.ty = Uint(32);
    cfg.params = vec![input_ptr, input_len].into();
    cfg
}

impl<'a> Dispatch<'a> {
    fn new(
        all_cfg: &'a [ControlFlowGraph],
        ns: &'a mut Namespace,
        opt: &'a Options,
        ty: FunctionTy,
    ) -> Self {
        let mut vartab = Vartable::new(ns.next_id);
        let mut cfg = new_cfg(ty);

        let input_len = vartab.temp_name("input_len", &Uint(32));
        cfg.add(
            &mut vartab,
            Instr::Set {
                loc: Codegen,
                res: input_len,
                expr: Expression::FunctionArg {
                    loc: Codegen,
                    ty: Uint(32),
                    arg_no: 1,
                },
            },
        );

        let input_ptr_var = vartab.temp_name("input_ptr", &Type::BufferPointer);
        cfg.add(
            &mut vartab,
            Instr::Set {
                loc: Codegen,
                res: input_ptr_var,
                expr: Expression::FunctionArg {
                    loc: Codegen,
                    ty: Type::BufferPointer,
                    arg_no: 0,
                },
            },
        );
        let input_ptr = Expression::Variable {
            loc: Codegen,
            ty: Type::BufferPointer,
            var_no: input_ptr_var,
        };

        let selector_len: Box<Expression> = Expression::NumberLiteral {
            loc: Codegen,
            ty: Uint(32),
            value: ns.target.selector_length().into(),
        }
        .into();

        // CREATE hands the constructor its arguments without a selector, so
        // only the call dispatcher skips over one.
        let input_ptr = match ty {
            FunctionTy::Constructor => input_ptr,
            _ => Expression::AdvancePointer {
                pointer: input_ptr.into(),
                bytes_offset: selector_len.clone(),
            },
        };

        Self {
            start: cfg.new_basic_block("start_dispatch".into()),
            input_len,
            input_ptr,
            vartab,
            cfg,
            all_cfg,
            ns,
            selector_len,
            opt,
            ty,
        }
    }

    fn build(self) -> ControlFlowGraph {
        if matches!(self.ty, FunctionTy::Constructor) {
            self.build_deploy()
        } else {
            self.build_call()
        }
    }

    /// The deploy dispatcher runs the constructor (if any) against the raw
    /// calldata. Returning the runtime code is the responsibility of the emit
    /// layer, which appends it after this dispatcher returns.
    fn build_deploy(mut self) -> ControlFlowGraph {
        // Terminate the entry block that `new` populated before moving on.
        self.add(Instr::Branch { block: self.start });
        self.cfg.set_basic_block(self.start);

        let constructor = self.all_cfg.iter().enumerate().find(|(_, func_cfg)| {
            matches!(func_cfg.ty, FunctionTy::Constructor) && func_cfg.public
        });

        if let Some((func_no, _)) = constructor {
            let args = self.decode_args(func_no, self.input_len_expr());
            self.add(Instr::Call {
                res: vec![],
                call: InternalCallTy::Static { cfg_no: func_no },
                args,
                return_tys: vec![],
            });
        }

        self.return_empty();
        self.vartab.finalize(self.ns, &mut self.cfg);
        self.cfg
    }

    fn build_call(mut self) -> ControlFlowGraph {
        // Anything shorter than a selector cannot be dispatched.
        let cond = Expression::Less {
            loc: Codegen,
            signed: false,
            left: Expression::Variable {
                loc: Codegen,
                ty: Uint(32),
                var_no: self.input_len,
            }
            .into(),
            right: self.selector_len.clone(),
        };
        let invalid = self.cfg.new_basic_block("invalid_selector".into());
        self.add(Instr::BranchCond {
            cond,
            true_block: invalid,
            false_block: self.start,
        });

        let selector_ty = Uint(8 * self.ns.target.selector_length() as u16);
        self.cfg.set_basic_block(self.start);

        let selector_var = self.vartab.temp_name("selector", &selector_ty);
        self.add(Instr::Set {
            loc: Codegen,
            res: selector_var,
            expr: Expression::Builtin {
                loc: Codegen,
                tys: vec![selector_ty.clone()],
                kind: Builtin::ReadFromBuffer,
                args: vec![
                    Expression::FunctionArg {
                        loc: Codegen,
                        ty: Type::BufferPointer,
                        arg_no: 0,
                    },
                    Expression::NumberLiteral {
                        loc: Codegen,
                        ty: Uint(32),
                        value: 0u64.into(),
                    },
                ],
            },
        });
        let selector = Expression::Variable {
            loc: Codegen,
            ty: selector_ty.clone(),
            var_no: selector_var,
        };

        let cases = self
            .all_cfg
            .iter()
            .enumerate()
            .filter(|(_, func_cfg)| matches!(func_cfg.ty, FunctionTy::Function) && func_cfg.public)
            .map(|(func_no, func_cfg)| {
                // `ReadFromBuffer` loads the selector as a little-endian
                // integer, so the big-endian ABI bytes must be reversed to
                // match.
                let value = BigInt::from_bytes_le(Sign::Plus, &func_cfg.selector);
                let case = Expression::NumberLiteral {
                    loc: Codegen,
                    ty: selector_ty.clone(),
                    value,
                };
                (case, func_no)
            })
            .collect::<Vec<_>>();

        let cases = cases
            .into_iter()
            .map(|(case, func_no)| (case, self.dispatch_case(func_no)))
            .collect();

        self.cfg.set_basic_block(self.start);
        self.add(Instr::Switch {
            cond: selector,
            cases,
            default: invalid,
        });

        self.cfg.set_basic_block(invalid);
        self.add(Instr::AssertFailure { encoded_args: None });

        self.vartab.finalize(self.ns, &mut self.cfg);
        self.cfg
    }

    /// Length of the calldata that follows the selector.
    fn input_len_expr(&self) -> Expression {
        let len = Expression::Variable {
            loc: Codegen,
            ty: Uint(32),
            var_no: self.input_len,
        };
        match self.ty {
            FunctionTy::Constructor => len,
            _ => Expression::Subtract {
                loc: Codegen,
                ty: Uint(32),
                overflowing: false,
                left: len.into(),
                right: self.selector_len.clone(),
            },
        }
    }

    fn decode_args(&mut self, func_no: usize, arg_len: Expression) -> Vec<Expression> {
        let tys = self.all_cfg[func_no]
            .params
            .iter()
            .map(|p| p.ty.clone())
            .collect::<Vec<_>>();

        if tys.is_empty() {
            return vec![];
        }

        let input_ptr = self.input_ptr.clone();
        abi_decode(
            &Codegen,
            &input_ptr,
            &tys,
            self.ns,
            &mut self.vartab,
            &mut self.cfg,
            Some(arg_len),
        )
    }

    fn dispatch_case(&mut self, func_no: usize) -> usize {
        let case_bb = self.cfg.new_basic_block(format!("func_{func_no}_dispatch"));
        self.cfg.set_basic_block(case_bb);

        let args = self.decode_args(func_no, self.input_len_expr());

        let mut returns = Vec::with_capacity(self.all_cfg[func_no].returns.len());
        let mut return_tys = Vec::with_capacity(self.all_cfg[func_no].returns.len());
        let mut returns_expr = Vec::with_capacity(self.all_cfg[func_no].returns.len());
        for item in self.all_cfg[func_no].returns.iter() {
            let v = self.vartab.temp_anonymous(&item.ty);
            returns.push(v);
            return_tys.push(item.ty.clone());
            returns_expr.push(Expression::Variable {
                loc: Codegen,
                ty: item.ty.clone(),
                var_no: v,
            });
        }

        self.add(Instr::Call {
            res: returns,
            call: InternalCallTy::Static { cfg_no: func_no },
            args,
            return_tys,
        });

        if returns_expr.is_empty() {
            self.return_empty();
        } else {
            let (data, data_len) = abi_encode(
                &Codegen,
                returns_expr,
                self.ns,
                &mut self.vartab,
                &mut self.cfg,
                false,
            );
            self.add(Instr::ReturnData { data, data_len });
        }

        case_bb
    }

    fn return_empty(&mut self) {
        let data_len = Expression::NumberLiteral {
            loc: Codegen,
            ty: Uint(32),
            value: 0u64.into(),
        };
        let data = Expression::AllocDynamicBytes {
            loc: Codegen,
            ty: Type::DynamicBytes,
            size: data_len.clone().into(),
            initializer: None,
        };
        self.add(Instr::ReturnData { data, data_len });
    }

    fn add(&mut self, ins: Instr) {
        self.cfg.add(&mut self.vartab, ins);
    }
}
