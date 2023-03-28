use crate::{
    codegen::{
        cfg::{ASTFunction, ControlFlowGraph, Instr, InternalCallTy, ReturnCode},
        encoding::abi_decode,
        vartable::Vartable,
        Builtin, Expression, Options,
    },
    sema::ast::{Namespace, Type, Type::Uint},
};
use num_bigint::{BigInt, Sign};
use solang_parser::pt::{FunctionTy, Loc::Codegen};

/// The dispatching algorithm consists of these steps:
/// 1. If the input is less than the expected selector length (default 4 bytes), fallback or receive.
/// 2. Match the function selector
///     - If no selector matches, fallback or receive.
///     - If the function is non-payable but the call features endowment, revert.
/// 3. ABI decode the arguments.
/// 4. Call the matching function.
/// 5. Return the result:
///     - On success, ABI encode the result (if any) and return.
///     - On failure, trap the contract.
///
/// We distinguish between fallback and receive:
/// - If there is no endowment, dispatch to fallback
/// - If there is endowment, dispatch to receive
pub(crate) fn function_dispatch(
    _contract_no: usize,
    all_cfg: &[ControlFlowGraph],
    ns: &mut Namespace,
    _opt: &Options,
) -> ControlFlowGraph {
    Dispatch::new(all_cfg, ns).build()
}

struct Dispatch<'a> {
    fail_bb: usize,
    input: usize,
    value: usize,
    vartab: Vartable,
    cfg: ControlFlowGraph,
    all_cfg: &'a [ControlFlowGraph],
    ns: &'a Namespace,
}

impl<'a> Dispatch<'a> {
    fn new(all_cfg: &'a [ControlFlowGraph], ns: &'a Namespace) -> Self {
        let mut vartab = Vartable::new(ns.next_id);
        let mut cfg = ControlFlowGraph::new("solang_dispatch".into(), ASTFunction::None);

        // Read input length from args
        let input_expr = Expression::FunctionArg(Codegen, Type::DynamicBytes, 0);
        let input = vartab.temp_name("input_len", &Uint(32));
        let expr = Expression::Builtin(
            Codegen,
            vec![Uint(32)],
            Builtin::ArrayLength,
            vec![input_expr.clone()],
        );
        cfg.add(
            &mut vartab,
            Instr::Set {
                loc: Codegen,
                res: input,
                expr,
            },
        );

        // Read transferred value from args
        let input_epxr = Expression::FunctionArg(Codegen, Type::DynamicBytes, 1);
        let value = vartab.temp_name("value", &Uint(ns.value_length as u16));
        cfg.add(
            &mut vartab,
            Instr::Set {
                loc: Codegen,
                res: value,
                expr: input_epxr,
            },
        );

        let fail_bb = cfg.new_basic_block("assert_failure".into());
        cfg.set_basic_block(fail_bb);
        cfg.add(&mut vartab, Instr::AssertFailure { encoded_args: None });

        Self {
            fail_bb,
            vartab,
            value,
            input,
            cfg,
            all_cfg,
            ns,
        }
    }

    fn build(mut self) -> ControlFlowGraph {
        // Go to fallback or receive if there is no selector in the call input
        let default = self.cfg.new_basic_block("fb_or_recv".into());
        let start_dispatch_block = self.cfg.new_basic_block("start_dispatch".into());
        let selector_len = self.ns.target.selector_length().into();
        let cond = Expression::Less {
            loc: Codegen,
            signed: false,
            left: Expression::NumberLiteral(Codegen, Uint(32), selector_len).into(),
            right: Expression::Variable(Codegen, Uint(32), self.input).into(),
        };
        self.add(Instr::BranchCond {
            cond,
            true_block: default,
            false_block: start_dispatch_block,
        });

        // Read selector
        self.cfg.set_basic_block(start_dispatch_block);
        let selector_ty = Uint(8 * self.ns.target.selector_length() as u16);
        let cond = Expression::Builtin(
            Codegen,
            vec![selector_ty.clone()],
            Builtin::ReadFromBuffer,
            vec![
                Expression::FunctionArg(Codegen, Type::DynamicBytes, 0),
                Expression::NumberLiteral(Codegen, selector_ty.clone(), 0.into()),
            ],
        );
        let cases = self
            .all_cfg
            .iter()
            .enumerate()
            .filter(|(_, msg_cfg)| {
                msg_cfg.public
                    && matches!(msg_cfg.ty, FunctionTy::Function | FunctionTy::Constructor)
            })
            .map(|(msg_no, msg_cfg)| {
                let selector = BigInt::from_bytes_le(Sign::Plus, &msg_cfg.selector);
                let case = Expression::NumberLiteral(Codegen, selector_ty.clone(), selector);
                (case, self.dispatch_case(msg_no))
            })
            .collect::<Vec<_>>();
        let switch = Instr::Switch {
            cond,
            cases,
            default,
        };
        self.cfg.add(&mut self.vartab, switch);

        // Handle fallback or receive case
        self.cfg.set_basic_block(default);
        self.fallback_or_receive();

        self.cfg
    }

    fn dispatch_case(&mut self, msg_no: usize) -> usize {
        let case = self.cfg.new_basic_block(format!("dispatch_case_{msg_no}"));
        self.cfg.set_basic_block(case);
        self.abort_if_value_transfer(msg_no);

        // TODO

        case
    }

    /// Insert a trap into the cfg, if the message `msg_no` is not payable but received value anyways.
    fn abort_if_value_transfer(&mut self, msg_no: usize) {
        if !self.all_cfg[msg_no].nonpayable {
            return;
        }
        let value_ty = Uint(self.ns.value_length as u16);
        let false_block = self.cfg.new_basic_block("no_value".into());
        self.add(Instr::BranchCond {
            cond: Expression::More {
                loc: Codegen,
                signed: false,
                left: Expression::NumberLiteral(Codegen, value_ty.clone(), 0.into()).into(),
                right: Expression::Variable(Codegen, value_ty, self.value).into(),
            },
            true_block: self.fail_bb,
            false_block,
        });
        self.cfg.set_basic_block(false_block);
    }

    fn fallback_or_receive(&mut self) {
        let fb_recv = self
            .all_cfg
            .iter()
            .enumerate()
            .fold([None, None], |mut acc, (no, cfg)| {
                match cfg.ty {
                    FunctionTy::Fallback if cfg.public => acc[0] = Some(no),
                    FunctionTy::Receive if cfg.public => acc[1] = Some(no),
                    _ => {}
                }
                acc
            });

        // No need to check value transferred; we will abort either way
        if fb_recv[0].is_none() && fb_recv[1].is_none() {
            return self.selector_invalid();
        }

        let value_ty = Uint(self.ns.value_length as u16);
        let fallback_block = self.cfg.new_basic_block("fallback".into());
        let receive_block = self.cfg.new_basic_block("receive".into());
        self.add(Instr::BranchCond {
            cond: Expression::More {
                loc: Codegen,
                signed: false,
                left: Expression::NumberLiteral(Codegen, value_ty.clone(), 0.into()).into(),
                right: Expression::Variable(Codegen, value_ty, self.value).into(),
            },
            true_block: receive_block,
            false_block: fallback_block,
        });

        self.cfg.set_basic_block(fallback_block);
        if let Some(cfg_no) = fb_recv[0] {
            self.add(Instr::Call {
                res: vec![],
                return_tys: vec![],
                call: InternalCallTy::Static { cfg_no },
                args: vec![],
            })
        } else {
            self.selector_invalid();
        }

        self.cfg.set_basic_block(receive_block);
        if let Some(cfg_no) = fb_recv[1] {
            self.add(Instr::Call {
                res: vec![],
                return_tys: vec![],
                call: InternalCallTy::Static { cfg_no },
                args: vec![],
            })
        } else {
            self.selector_invalid()
        }
    }

    fn selector_invalid(&mut self) {
        let code = ReturnCode::FunctionSelectorInvalid;
        self.add(Instr::ReturnCode { code });
    }

    fn add(&mut self, ins: Instr) {
        self.cfg.add(&mut self.vartab, ins);
    }
}