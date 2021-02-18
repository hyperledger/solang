use super::Contract;
use inkwell::basic_block::BasicBlock;
use inkwell::types::BasicType;
use inkwell::values::{BasicValueEnum, FunctionValue, IntValue, PhiValue};
use inkwell::IntPredicate;
use std::collections::HashMap;

/// This is for generating a simple loop, iterating over a number to a higher number (not inclusive). This
/// builder helps with the creation of the phi nodes.
pub struct LoopBuilder<'a> {
    phis: HashMap<&'static str, PhiValue<'a>>,
    entry_block: BasicBlock<'a>,
    condition_block: BasicBlock<'a>,
    body_block: BasicBlock<'a>,
    done_block: BasicBlock<'a>,
    loop_phi: Option<PhiValue<'a>>,
    next_index: Option<IntValue<'a>>,
}

impl<'a> LoopBuilder<'a> {
    /// Create a new loop. This creates the basic blocks and inserts a branch to start of the loop at
    /// the current location. This function should be called first.
    pub fn new(contract: &Contract<'a>, function: FunctionValue) -> Self {
        let entry_block = contract.builder.get_insert_block().unwrap();
        let condition_block = contract.context.append_basic_block(function, "cond");
        let body_block = contract.context.append_basic_block(function, "body");
        let done_block = contract.context.append_basic_block(function, "done");

        contract.builder.build_unconditional_branch(condition_block);

        contract.builder.position_at_end(condition_block);

        LoopBuilder {
            phis: HashMap::new(),
            entry_block,
            condition_block,
            body_block,
            done_block,
            loop_phi: None,
            next_index: None,
        }
    }

    /// Once LoopBuilder::new() has been called, we need to create the phi nodes for all the variables
    /// we wish to use and modify. The initial value which it will have on entry to the condition/body
    /// must be given.
    pub fn add_loop_phi<T: BasicType<'a>>(
        &mut self,
        contract: &Contract<'a>,
        name: &'static str,
        ty: T,
        initial_value: BasicValueEnum<'a>,
    ) -> BasicValueEnum<'a> {
        let phi = contract.builder.build_phi(ty, name);

        phi.add_incoming(&[(&initial_value, self.entry_block)]);

        self.phis.insert(name, phi);

        phi.as_basic_value()
    }

    /// Once all the phi nodes are created with add_loop_phi(), then call over(). This must be given
    /// the two loop bounds (lower and higher). The higher bound is not inclusive. This function
    /// builds the condition and then jumps to do the body; the return value is in the index
    /// which can be used in the body. The body of the loop can be inserted after calling this
    /// function.
    pub fn over(
        &mut self,
        contract: &Contract<'a>,
        from: IntValue<'a>,
        to: IntValue<'a>,
    ) -> IntValue<'a> {
        let loop_ty = from.get_type();
        let loop_phi = contract.builder.build_phi(loop_ty, "index");

        let loop_var = loop_phi.as_basic_value().into_int_value();

        let next =
            contract
                .builder
                .build_int_add(loop_var, loop_ty.const_int(1, false), "next_index");

        let comp = contract
            .builder
            .build_int_compare(IntPredicate::ULT, loop_var, to, "loop_cond");

        contract
            .builder
            .build_conditional_branch(comp, self.body_block, self.done_block);

        loop_phi.add_incoming(&[(&from, self.entry_block)]);

        contract.builder.position_at_end(self.body_block);

        self.loop_phi = Some(loop_phi);
        self.next_index = Some(next);

        loop_phi.as_basic_value().into_int_value()
    }

    /// Use this function to set the loop phis to their values at the end of the body
    pub fn set_loop_phi_value(
        &self,
        contract: &Contract<'a>,
        name: &'static str,
        value: BasicValueEnum<'a>,
    ) {
        let block = contract.builder.get_insert_block().unwrap();

        self.phis[name].add_incoming(&[(&value, block)]);
    }

    /// Can you use this to get the value of a phi in the body or after the loop exits
    pub fn get_loop_phi(&self, name: &'static str) -> BasicValueEnum<'a> {
        self.phis[name].as_basic_value()
    }

    /// Call this once the body of the loop has been generated. This will close the loop
    /// and ensure the exit block has been reached.
    pub fn finish(&self, contract: &Contract<'a>) {
        let block = contract.builder.get_insert_block().unwrap();

        let loop_phi = self.loop_phi.unwrap();

        loop_phi.add_incoming(&[(self.next_index.as_ref().unwrap(), block)]);

        contract
            .builder
            .build_unconditional_branch(self.condition_block);

        contract.builder.position_at_end(self.done_block);
    }
}
