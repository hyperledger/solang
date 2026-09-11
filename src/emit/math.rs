// SPDX-License-Identifier: Apache-2.0

use crate::codegen::revert::PanicCode;
use crate::emit::binary::Binary;
use crate::emit::{BinaryOp, TargetRuntime};
use inkwell::types::IntType;
use inkwell::values::{FunctionValue, IntValue, PointerValue};
use inkwell::IntPredicate;
use solang_parser::pt::Loc;

/// Signed overflow detection is handled by the following steps:
/// 1- Do an unsigned multiplication first, This step will check if the generated value will fit in N bits. (unsigned overflow)
/// 2- Get the result, and negate it if needed.
/// 3- Check for signed overflow, by checking for an unexpected change in the sign of the result.
fn signed_ovf_detect<'a, T: TargetRuntime<'a> + ?Sized>(
    target: &T,
    bin: &Binary<'a>,
    mul_ty: IntType<'a>,
    mul_bits: u32,
    left: IntValue<'a>,
    right: IntValue<'a>,
    bits: u32,
    function: FunctionValue<'a>,
    loc: Loc,
) -> IntValue<'a> {
    // We check for signed overflow based on the facts:
    //  - * - = +
    //  + * + = +
    //  - * + = - (if op1 and op2 != 0)
    // if one of the operands is zero, discard the last rule.
    let left_negative = bin
        .builder
        .build_int_compare(
            IntPredicate::SLT,
            left,
            left.get_type().const_zero(),
            "left_negative",
        )
        .unwrap();

    let left_abs = bin
        .builder
        .build_select(
            left_negative,
            bin.builder.build_int_neg(left, "signed_left").unwrap(),
            left,
            "left_abs",
        )
        .unwrap()
        .into_int_value();

    let right_negative = bin
        .builder
        .build_int_compare(
            IntPredicate::SLT,
            right,
            right.get_type().const_zero(),
            "right_negative",
        )
        .unwrap();

    let right_abs = bin
        .builder
        .build_select(
            right_negative,
            bin.builder.build_int_neg(right, "signed_right").unwrap(),
            right,
            "right_abs",
        )
        .unwrap()
        .into_int_value();

    let l = bin.build_alloca(function, mul_ty, "");
    let r = bin.build_alloca(function, mul_ty, "");
    let o = bin.build_alloca(function, mul_ty, "");

    bin.builder
        .build_store(
            l,
            bin.builder
                .build_int_z_extend(left_abs, mul_ty, "")
                .unwrap(),
        )
        .unwrap();
    bin.builder
        .build_store(
            r,
            bin.builder
                .build_int_z_extend(right_abs, mul_ty, "")
                .unwrap(),
        )
        .unwrap();

    let return_val = bin.builder.build_call(
        bin.module.get_function("__mul32_with_builtin_ovf").unwrap(),
        &[
            l.into(),
            r.into(),
            o.into(),
            bin.context
                .i32_type()
                .const_int(mul_bits as u64 / 32, false)
                .into(),
        ],
        "",
    );

    let res = bin.builder.build_load(mul_ty, o, "mul").unwrap();
    let ovf_any_type = if mul_bits != bits {
        // If there are any set bits, then there is an overflow.
        let check_ovf = bin
            .builder
            .build_right_shift(
                res.into_int_value(),
                mul_ty.const_int((bits).into(), false),
                false,
                "",
            )
            .unwrap();
        bin.builder
            .build_int_compare(
                IntPredicate::NE,
                check_ovf,
                check_ovf.get_type().const_zero(),
                "",
            )
            .unwrap()
    } else {
        // If no size extension took place, there is no overflow in most significant N bits
        bin.context.bool_type().const_zero()
    };

    let negate_result = bin
        .builder
        .build_xor(left_negative, right_negative, "negate_result")
        .unwrap();

    let res = bin
        .builder
        .build_select(
            negate_result,
            bin.builder
                .build_int_neg(res.into_int_value(), "unsigned_res")
                .unwrap(),
            res.into_int_value(),
            "res",
        )
        .unwrap();

    let error_block = bin.context.append_basic_block(function, "error");
    let return_block = bin.context.append_basic_block(function, "return_block");

    // Extract sign bit of the operands and the result
    let left_sign_bit = extract_sign_bit(bin, left, left.get_type());
    let right_sign_bit = extract_sign_bit(bin, right, right.get_type());
    let res_sign_bit = if mul_bits == bits {
        // If no extension took place, get the leftmost bit(sign bit).
        extract_sign_bit(bin, res.into_int_value(), res.into_int_value().get_type())
    } else {
        // If extension took place, truncate the result to the type of the operands then extract the leftmost bit(sign bit).
        extract_sign_bit(
            bin,
            bin.builder
                .build_int_truncate(res.into_int_value(), left.get_type(), "")
                .unwrap(),
            left.get_type(),
        )
    };

    let value_fits_n_bits = bin
        .builder
        .build_not(
            bin.builder
                .build_or(
                    return_val
                        .unwrap()
                        .try_as_basic_value()
                        .left()
                        .unwrap()
                        .into_int_value(),
                    ovf_any_type,
                    "",
                )
                .unwrap(),
            "",
        )
        .unwrap();

    let left_is_zero = bin
        .builder
        .build_int_compare(IntPredicate::EQ, left, left.get_type().const_zero(), "")
        .unwrap();
    let right_is_zero = bin
        .builder
        .build_int_compare(IntPredicate::EQ, right, right.get_type().const_zero(), "")
        .unwrap();

    // If one of the operands is zero
    let mul_by_zero = bin
        .builder
        .build_or(left_is_zero, right_is_zero, "")
        .unwrap();

    // Will resolve to one if signs are differnet
    let different_signs = bin
        .builder
        .build_xor(left_sign_bit, right_sign_bit, "")
        .unwrap();

    let not_ok_operation = bin
        .builder
        .build_not(
            bin.builder
                .build_xor(different_signs, res_sign_bit, "")
                .unwrap(),
            "",
        )
        .unwrap();

    // Here, we disregard the last rule mentioned above if there is a multiplication by zero.
    bin.builder
        .build_conditional_branch(
            bin.builder
                .build_and(
                    bin.builder
                        .build_or(not_ok_operation, mul_by_zero, "")
                        .unwrap(),
                    value_fits_n_bits,
                    "",
                )
                .unwrap(),
            return_block,
            error_block,
        )
        .unwrap();

    bin.builder.position_at_end(error_block);

    bin.log_runtime_error(target, "multiplication overflow".to_string(), Some(loc));
    let (revert_out, revert_out_len) = bin.panic_data_const(PanicCode::MathOverflow);
    target.assert_failure(bin, revert_out, revert_out_len);

    bin.builder.position_at_end(return_block);

    bin.builder
        .build_int_truncate(res.into_int_value(), left.get_type(), "")
        .unwrap()
}

/// Call void __mul32 and return the result.
fn call_mul32_without_ovf<'a>(
    bin: &Binary<'a>,
    l: PointerValue<'a>,
    r: PointerValue<'a>,
    o: PointerValue<'a>,
    mul_bits: u32,
    mul_type: IntType<'a>,
    res_type: IntType<'a>,
) -> IntValue<'a> {
    bin.builder
        .build_call(
            bin.module.get_function("__mul32").unwrap(),
            &[
                l.into(),
                r.into(),
                o.into(),
                bin.context
                    .i32_type()
                    .const_int(mul_bits as u64 / 32, false)
                    .into(),
            ],
            "",
        )
        .unwrap();

    let res = bin.builder.build_load(mul_type, o, "mul").unwrap();

    bin.builder
        .build_int_truncate(res.into_int_value(), res_type, "")
        .unwrap()
}

/// Utility function to extract the sign bit of an IntValue
fn extract_sign_bit<'a>(
    bin: &Binary<'a>,
    operand: IntValue<'a>,
    int_type: IntType<'a>,
) -> IntValue<'a> {
    let n_bits_to_shift = int_type.get_bit_width() - 1;
    let val_to_shift = int_type.const_int(n_bits_to_shift as u64, false);
    let shifted = bin
        .builder
        .build_right_shift(operand, val_to_shift, false, "")
        .unwrap();
    bin.builder
        .build_int_truncate(shifted, bin.context.bool_type(), "")
        .unwrap()
}

/// Emit a multiply for any width with or without overflow checking
pub(super) fn multiply<'a, T: TargetRuntime<'a> + ?Sized>(
    target: &T,
    bin: &Binary<'a>,
    function: FunctionValue<'a>,
    unchecked: bool,
    left: IntValue<'a>,
    right: IntValue<'a>,
    signed: bool,
    loc: Loc,
) -> IntValue<'a> {
    let bits = left.get_type().get_bit_width();

    // Mul with overflow is not supported beyond this bit range, so we implement our own function
    if bits > 32 {
        // Round up the number of bits to the next 32
        let mul_bits = (bits + 31) & !31;
        let mul_ty = bin.context.custom_width_int_type(mul_bits);

        // Round up bits
        let l = bin.build_alloca(function, mul_ty, "");
        let r = bin.build_alloca(function, mul_ty, "");
        let o = bin.build_alloca(function, mul_ty, "");

        if mul_bits == bits {
            bin.builder.build_store(l, left).unwrap();
            bin.builder.build_store(r, right).unwrap();
        }
        // LLVM-IR can handle multiplication of sizes up to 64 bits. If the size is larger, we need to implement our own multiplication function.
        // We divide the operands into sizes of 32 bits (check __mul32 in stdlib/bigint.c documentation).
        // If the size is not divisble by 32, we extend it to the next 32 bits. For example, int72 will be extended to int96.
        // Here, we zext the operands to the nearest 32 bits. zext is called instead of sext because we need to do unsigned multiplication by default.
        // It will not matter in terms of mul without overflow, because we always truncate the result to the bit size of the operands.
        // In mul with overflow however, it is needed so that overflow can be detected if the most significant bits of the result are not zeros.
        else {
            bin.builder
                .build_store(l, bin.builder.build_int_z_extend(left, mul_ty, "").unwrap())
                .unwrap();
            bin.builder
                .build_store(
                    r,
                    bin.builder.build_int_z_extend(right, mul_ty, "").unwrap(),
                )
                .unwrap();
        }

        if !unchecked {
            if signed {
                return signed_ovf_detect(
                    target, bin, mul_ty, mul_bits, left, right, bits, function, loc,
                );
            }

            // Unsigned overflow detection Approach:
            // If the size is a multiple of 32, we call __mul32_with_builtin_ovf and it returns an overflow flag (check __mul32_with_builtin_ovf in stdlib/bigint.c documentation)
            // If that is not the case, some extra work has to be done. We have to check the extended bits for any set bits. If there is any, an overflow occurred.
            // For example, if we have uint72, it will be extended to uint96. __mul32 with ovf will raise an ovf flag if the result overflows 96 bits, not 72.
            // We account for that by checking the extended leftmost bits. In the example mentioned, they will be 96-72=24 bits.
            let return_val = bin
                .builder
                .build_call(
                    bin.module.get_function("__mul32_with_builtin_ovf").unwrap(),
                    &[
                        l.into(),
                        r.into(),
                        o.into(),
                        bin.context
                            .i32_type()
                            .const_int(mul_bits as u64 / 32, false)
                            .into(),
                    ],
                    "ovf",
                )
                .unwrap();

            let res = bin.builder.build_load(mul_ty, o, "mul").unwrap();

            let error_block = bin.context.append_basic_block(function, "error");
            let return_block = bin.context.append_basic_block(function, "return_block");

            // If the operands were extended to nearest 32 bit size, check the most significant N bits, where N equals bit width after extension minus original bit width.
            let ovf_any_type = if mul_bits != bits {
                // If there are any set bits, then there is an overflow.
                let check_ovf = bin
                    .builder
                    .build_right_shift(
                        res.into_int_value(),
                        mul_ty.const_int((bits).into(), false),
                        false,
                        "",
                    )
                    .unwrap();
                bin.builder
                    .build_int_compare(
                        IntPredicate::NE,
                        check_ovf,
                        check_ovf.get_type().const_zero(),
                        "",
                    )
                    .unwrap()
            } else {
                // If no size extension took place, there is no overflow in most significant N bits
                bin.context.bool_type().const_zero()
            };

            // Until this point, we only checked the extended bits for ovf. But mul ovf can take place any where from bit size to double bit size.
            // For example: If we have uint72, it will be extended to uint96. We only checked the most significant 24 bits for overflow, which can happen up to 72*2=144 bits.
            // bool __mul32_with_builtin_ovf takes care of overflowing bits beyond 96.
            // What is left now is to or these two ovf flags, and check if any one of them is set. If so, an overflow occurred.
            let lowbit = bin
                .builder
                .build_int_truncate(
                    bin.builder
                        .build_or(
                            ovf_any_type,
                            return_val
                                .try_as_basic_value()
                                .left()
                                .unwrap()
                                .into_int_value(),
                            "",
                        )
                        .unwrap(),
                    bin.context.bool_type(),
                    "bit",
                )
                .unwrap();

            // If ovf, raise an error, else return the result.
            bin.builder
                .build_conditional_branch(lowbit, error_block, return_block)
                .unwrap();

            bin.builder.position_at_end(error_block);

            bin.log_runtime_error(target, "multiplication overflow".to_string(), Some(loc));

            let (revert_out, revert_out_len) = bin.panic_data_const(PanicCode::MathOverflow);
            target.assert_failure(bin, revert_out, revert_out_len);

            bin.builder.position_at_end(return_block);

            bin.builder
                .build_int_truncate(res.into_int_value(), left.get_type(), "")
                .unwrap()
        } else {
            call_mul32_without_ovf(bin, l, r, o, mul_bits, mul_ty, left.get_type())
        }
    } else if !unchecked {
        build_binary_op_with_overflow_check(
            target,
            bin,
            function,
            left,
            right,
            BinaryOp::Multiply,
            signed,
            loc,
        )
    } else {
        bin.builder.build_int_mul(left, right, "").unwrap()
    }
}

pub(super) fn power<'a, T: TargetRuntime<'a> + ?Sized>(
    target: &T,
    bin: &Binary<'a>,
    overflowing: bool,
    bits: u32,
    signed: bool,
    o: PointerValue<'a>,
    loc: Loc,
) -> FunctionValue<'a> {
    /*
        int ipow(int base, int exp)
        {
            int result = 1;
            for (;;)
            {
                if (exp & 1)
                    result *= base;
                exp >>= 1;
                if (!exp)
                    break;
                base *= base;
            }
            return result;
        }
    */
    let name = format!(
        "__{}power{}{}",
        if signed { 's' } else { 'u' },
        bits,
        if overflowing { "overflowing" } else { "" }
    );
    let ty = bin.context.custom_width_int_type(bits);

    if let Some(f) = bin.module.get_function(&name) {
        return f;
    }

    let pos = bin.builder.get_insert_block().unwrap();

    // __upower(base, exp)
    let function = bin.module.add_function(
        &name,
        bin.context
            .i64_type()
            .fn_type(&[ty.into(), ty.into(), o.get_type().into()], false),
        None,
    );

    let entry = bin.context.append_basic_block(function, "entry");
    let loop_block = bin.context.append_basic_block(function, "loop");
    let multiply_block = bin.context.append_basic_block(function, "multiply");
    let nomultiply = bin.context.append_basic_block(function, "nomultiply");
    let done = bin.context.append_basic_block(function, "done");
    let notdone = bin.context.append_basic_block(function, "notdone");

    bin.builder.position_at_end(entry);

    bin.builder.build_unconditional_branch(loop_block).unwrap();

    bin.builder.position_at_end(loop_block);
    let base = bin.builder.build_phi(ty, "base").unwrap();
    base.add_incoming(&[(&function.get_nth_param(0).unwrap(), entry)]);

    let exp = bin.builder.build_phi(ty, "exp").unwrap();
    exp.add_incoming(&[(&function.get_nth_param(1).unwrap(), entry)]);

    let result = bin.builder.build_phi(ty, "result").unwrap();
    result.add_incoming(&[(&ty.const_int(1, false), entry)]);

    let lowbit = bin
        .builder
        .build_int_truncate(
            exp.as_basic_value().into_int_value(),
            bin.context.bool_type(),
            "bit",
        )
        .unwrap();

    bin.builder
        .build_conditional_branch(lowbit, multiply_block, nomultiply)
        .unwrap();

    bin.builder.position_at_end(multiply_block);

    let result2 = multiply(
        target,
        bin,
        function,
        overflowing,
        result.as_basic_value().into_int_value(),
        base.as_basic_value().into_int_value(),
        signed,
        loc,
    );

    let multiply_block = bin.builder.get_insert_block().unwrap();

    bin.builder.build_unconditional_branch(nomultiply).unwrap();
    bin.builder.position_at_end(nomultiply);

    let result3 = bin.builder.build_phi(ty, "result").unwrap();
    result3.add_incoming(&[
        (&result.as_basic_value(), loop_block),
        (&result2, multiply_block),
    ]);

    let exp2 = bin
        .builder
        .build_right_shift(
            exp.as_basic_value().into_int_value(),
            ty.const_int(1, false),
            false,
            "exp",
        )
        .unwrap();
    let zero = bin
        .builder
        .build_int_compare(IntPredicate::EQ, exp2, ty.const_zero(), "zero")
        .unwrap();

    bin.builder
        .build_conditional_branch(zero, done, notdone)
        .unwrap();
    bin.builder.position_at_end(done);

    // If successful operation, load the result in the output pointer then return zero.
    bin.builder
        .build_store(
            function.get_nth_param(2).unwrap().into_pointer_value(),
            result3.as_basic_value(),
        )
        .unwrap();
    bin.builder
        .build_return(Some(&bin.context.i64_type().const_zero()))
        .unwrap();

    bin.builder.position_at_end(notdone);

    let base2 = multiply(
        target,
        bin,
        function,
        overflowing,
        base.as_basic_value().into_int_value(),
        base.as_basic_value().into_int_value(),
        signed,
        loc,
    );

    let notdone = bin.builder.get_insert_block().unwrap();

    base.add_incoming(&[(&base2, notdone)]);
    result.add_incoming(&[(&result3.as_basic_value(), notdone)]);
    exp.add_incoming(&[(&exp2, notdone)]);

    bin.builder.build_unconditional_branch(loop_block).unwrap();

    bin.builder.position_at_end(pos);

    function
}

/// Convenience function for generating binary operations with overflow checking.
pub(super) fn build_binary_op_with_overflow_check<'a, T: TargetRuntime<'a> + ?Sized>(
    target: &T,
    bin: &Binary<'a>,
    function: FunctionValue,
    left: IntValue<'a>,
    right: IntValue<'a>,
    op: BinaryOp,
    signed: bool,
    loc: Loc,
) -> IntValue<'a> {
    let ret_ty = bin.context.struct_type(
        &[
            left.get_type().into(),
            bin.context.custom_width_int_type(1).into(),
        ],
        false,
    );
    let binop = bin.llvm_overflow(ret_ty.into(), left.get_type(), signed, op);

    let op_res = bin
        .builder
        .build_call(binop, &[left.into(), right.into()], "res")
        .unwrap()
        .try_as_basic_value()
        .left()
        .unwrap()
        .into_struct_value();

    let overflow = bin
        .builder
        .build_extract_value(op_res, 1, "overflow")
        .unwrap()
        .into_int_value();

    let success_block = bin.context.append_basic_block(function, "success");
    let error_block = bin.context.append_basic_block(function, "error");

    bin.builder
        .build_conditional_branch(overflow, error_block, success_block)
        .unwrap();

    bin.builder.position_at_end(error_block);

    bin.log_runtime_error(target, "math overflow".to_string(), Some(loc));
    let (revert_out, revert_out_len) = bin.panic_data_const(PanicCode::MathOverflow);
    target.assert_failure(bin, revert_out, revert_out_len);

    bin.builder.position_at_end(success_block);

    bin.builder
        .build_extract_value(op_res, 0, "res")
        .unwrap()
        .into_int_value()
}
