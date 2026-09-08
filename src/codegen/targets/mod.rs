// SPDX-License-Identifier: Apache-2.0

pub(crate) mod abi;
pub(crate) mod buffer_validator;
pub(crate) mod polkadot;
pub(crate) mod riscv;
pub(crate) mod solana;
pub(crate) mod soroban;

use crate::codegen::interface::TargetCodegen;
use crate::sema::ast::Namespace;
use crate::Target;

use self::polkadot::{EvmTarget, PolkadotTarget};
use self::riscv::RiscvTarget;
use self::solana::SolanaTarget;
use self::soroban::SorobanTarget;

pub(crate) fn make_target(ns: &Namespace) -> Box<dyn TargetCodegen> {
    match &ns.target {
        Target::Soroban => Box::new(SorobanTarget),
        Target::Solana => Box::new(SolanaTarget),
        Target::Polkadot { .. } => Box::new(PolkadotTarget),
        Target::EVM => Box::new(EvmTarget(PolkadotTarget)),
        Target::Riscv => Box::new(RiscvTarget),
    }
}
