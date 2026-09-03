// SPDX-License-Identifier: Apache-2.0

use core::panic;

use solang_parser::pt::Loc;

use crate::codegen;
use crate::lir::converter::Converter;
use crate::lir::expressions::{BinaryOperator, Expression, Operand, UnaryOperator};
use crate::lir::instructions::Instruction;
use crate::lir::vartable::Vartable;
use crate::sema::ast;

impl Converter<'_> {
    /**
    <pre>
    This function lowers an expression (tree) into a list of three-address code
    instructions and assign the last expression with maxmum two operands to the
    <code>dest</code> operand.

    For example, suppose <code>a</code> and <code>b</code> and <code>c</code> all have <code>int32</code> type in the <code>Vartable</code>,
    the expression <code>a + b + c</code> will be lowered into the following:
    <code>int32 temp1 = a + b</code>
    <code>int32 dest = temp1 + c</code>

    Input:
    <li><code>dest</code>: the destination operand for the last expression
    <li><code>expr</code>: the expression to be lowered
    <li><code>vartable</code>: the variable table storing all the variables and their types
    <li><code>results</code>: the list of instructions to be appended with the lowered instructions
    </pre>
    */
    pub(crate) fn lower_expression(
        &self,
        dest: &Operand,
        expr: &codegen::Expression,
        vartable: &mut Vartable,
        results: &mut Vec<Instruction>,
    ) {
        match expr {
            codegen::Expression::Add {
                loc,
                ty,
                overflowing,
                left,
                right,
            } => {
                let operator = BinaryOperator::Add {
                    overflowing: *overflowing,
                };
                self.binary_operation(dest, loc, ty, operator, left, right, vartable, results)
            }
            codegen::Expression::AllocDynamicBytes {
                loc,
                ty,
                size,
                initializer,
                ..
            } => self.alloc_dynamic_bytes(dest, loc, ty, size, initializer, vartable, results),
            codegen::Expression::ArrayLiteral {
                loc,
                ty,
                dimensions,
                values,
                ..
            } => self.array_literal(dest, loc, ty, dimensions, values, vartable, results),
            codegen::Expression::BitwiseAnd {
                loc,
                left,
                right,
                ty,
                ..
            } => {
                let operator = BinaryOperator::BitAnd;
                self.binary_operation(dest, loc, ty, operator, left, right, vartable, results)
            }
            codegen::Expression::BitwiseOr {
                loc,
                left,
                right,
                ty,
                ..
            } => {
                let operator = BinaryOperator::BitOr;
                self.binary_operation(dest, loc, ty, operator, left, right, vartable, results)
            }
            codegen::Expression::BitwiseXor {
                loc,
                left,
                right,
                ty,
                ..
            } => {
                let operator = BinaryOperator::BitXor;
                self.binary_operation(dest, loc, ty, operator, left, right, vartable, results)
            }
            codegen::Expression::BoolLiteral { loc, value, .. } => {
                self.bool_literal(dest, loc, value, results)
            }
            codegen::Expression::Builtin {
                loc, kind, args, ..
            } => self.builtin(dest, loc, kind, args, vartable, results),
            codegen::Expression::BytesCast { loc, expr, ty, .. } => {
                self.byte_cast(dest, loc, expr, ty, vartable, results)
            }
            codegen::Expression::BytesLiteral { loc, ty, value, .. } => {
                self.bytes_literal(dest, loc, ty, value, results)
            }
            codegen::Expression::Cast { loc, ty, expr, .. } => {
                self.cast(dest, loc, ty, expr, vartable, results)
            }
            codegen::Expression::BitwiseNot { loc, expr, .. } => {
                let operator = UnaryOperator::BitNot;
                self.unary_operation(dest, loc, operator, expr, vartable, results)
            }
            codegen::Expression::ConstArrayLiteral {
                loc,
                ty,
                dimensions,
                values,
                ..
            } => self.const_array_literal(dest, loc, ty, dimensions, values, vartable, results),
            codegen::Expression::UnsignedDivide {
                loc,
                left,
                right,
                ty,
                ..
            } => {
                let operator = BinaryOperator::UDiv;
                self.binary_operation(dest, loc, ty, operator, left, right, vartable, results)
            }
            codegen::Expression::SignedDivide {
                loc,
                left,
                right,
                ty,
                ..
            } => {
                let operator = BinaryOperator::Div;
                self.binary_operation(dest, loc, ty, operator, left, right, vartable, results)
            }
            codegen::Expression::Equal {
                loc, left, right, ..
            } => {
                let operator = BinaryOperator::Eq;
                self.binary_operation(
                    dest,
                    loc,
                    &ast::Type::Bool,
                    operator,
                    left,
                    right,
                    vartable,
                    results,
                )
            }
            codegen::Expression::FormatString { loc, args, .. } => {
                self.format_string(dest, loc, args, vartable, results)
            }
            codegen::Expression::FunctionArg {
                loc, ty, arg_no, ..
            } => self.function_arg(dest, loc, ty, arg_no, vartable, results),
            codegen::Expression::GetRef { loc, expr, .. } => {
                self.get_ref(dest, loc, expr, vartable, results)
            }
            codegen::Expression::InternalFunctionCfg { cfg_no, .. } => {
                self.internal_function_cfg(dest, cfg_no, results)
            }
            codegen::Expression::Keccak256 { loc, exprs, .. } => {
                self.keccak256(dest, loc, exprs, vartable, results)
            }
            codegen::Expression::Less {
                loc,
                left,
                right,
                signed,
                ..
            } => {
                let operator = if *signed {
                    BinaryOperator::Lt
                } else {
                    BinaryOperator::ULt
                };
                self.binary_operation(
                    dest,
                    loc,
                    &ast::Type::Bool,
                    operator,
                    left,
                    right,
                    vartable,
                    results,
                )
            }
            codegen::Expression::LessEqual {
                loc,
                left,
                right,
                signed,
                ..
            } => {
                let operator = if *signed {
                    BinaryOperator::Lte
                } else {
                    BinaryOperator::ULte
                };
                self.binary_operation(
                    dest,
                    loc,
                    &ast::Type::Bool,
                    operator,
                    left,
                    right,
                    vartable,
                    results,
                )
            }
            codegen::Expression::Load { loc, expr, .. } => {
                self.load(dest, loc, expr, vartable, results)
            }
            codegen::Expression::UnsignedModulo {
                loc,
                left,
                right,
                ty,
                ..
            } => {
                let operator = BinaryOperator::UMod;
                self.binary_operation(dest, loc, ty, operator, left, right, vartable, results)
            }
            codegen::Expression::SignedModulo {
                loc,
                left,
                right,
                ty,
                ..
            } => {
                let operator = BinaryOperator::Mod;
                self.binary_operation(dest, loc, ty, operator, left, right, vartable, results)
            }
            codegen::Expression::More {
                left,
                right,
                signed,
                ..
            } => {
                let operator = if *signed {
                    BinaryOperator::Gt
                } else {
                    BinaryOperator::UGt
                };
                self.binary_operation(
                    dest,
                    &Loc::Codegen,
                    &ast::Type::Bool,
                    operator,
                    left,
                    right,
                    vartable,
                    results,
                )
            }
            codegen::Expression::MoreEqual {
                left,
                right,
                signed,
                ..
            } => {
                let operator = if *signed {
                    BinaryOperator::Gte
                } else {
                    BinaryOperator::UGte
                };
                self.binary_operation(
                    dest,
                    &Loc::Codegen,
                    &ast::Type::Bool,
                    operator,
                    left,
                    right,
                    vartable,
                    results,
                )
            }
            codegen::Expression::Multiply {
                loc,
                left,
                right,
                overflowing,
                ty,
                ..
            } => {
                let operator = BinaryOperator::Mul {
                    overflowing: *overflowing,
                };
                self.binary_operation(dest, loc, ty, operator, left, right, vartable, results)
            }
            codegen::Expression::Not { loc, expr, .. } => {
                let operator = UnaryOperator::Not;
                self.unary_operation(dest, loc, operator, expr, vartable, results)
            }
            codegen::Expression::NotEqual {
                loc, left, right, ..
            } => {
                let operator = BinaryOperator::Neq;
                self.binary_operation(
                    dest,
                    loc,
                    &ast::Type::Bool,
                    operator,
                    left,
                    right,
                    vartable,
                    results,
                )
            }
            codegen::Expression::NumberLiteral { loc, value, .. } => {
                self.number_literal(dest, loc, value, results)
            }
            codegen::Expression::Poison => panic!("Poison expression shouldn't be here"),
            codegen::Expression::Power {
                loc,
                base,
                exp,
                overflowing,
                ty,
                ..
            } => {
                let operator = BinaryOperator::Pow {
                    overflowing: *overflowing,
                };
                self.binary_operation(dest, loc, ty, operator, base, exp, vartable, results)
            }
            codegen::Expression::RationalNumberLiteral { .. } => {
                panic!("RationalNumberLiteral shouldn't be here")
            }
            codegen::Expression::ReturnData { loc, .. } => self.return_data(dest, loc, results),
            codegen::Expression::SignExt { loc, ty, expr, .. } => {
                self.sign_ext(dest, loc, ty, expr, vartable, results)
            }
            codegen::Expression::ShiftLeft {
                loc,
                ty,
                left,
                right,
                ..
            } => {
                let operator = BinaryOperator::Shl;
                self.binary_operation(dest, loc, ty, operator, left, right, vartable, results)
            }
            codegen::Expression::ShiftRight {
                loc,
                left,
                right,
                signed,
                ty,
                ..
            } => {
                let operator = if *signed {
                    BinaryOperator::Shr
                } else {
                    BinaryOperator::UShr
                };
                self.binary_operation(dest, loc, ty, operator, left, right, vartable, results)
            }
            codegen::Expression::StorageArrayLength { loc, array, .. } => {
                self.storage_array_length(dest, loc, array, vartable, results)
            }
            codegen::Expression::StringCompare {
                loc, left, right, ..
            } => self.string_compare(dest, loc, left, right, vartable, results),
            codegen::Expression::StructLiteral {
                loc, ty, values, ..
            } => self.struct_literal(dest, loc, ty, values, vartable, results),
            codegen::Expression::StructMember {
                loc, expr, member, ..
            } => self.struct_member(dest, loc, expr, member, vartable, results),
            codegen::Expression::Subscript {
                loc, expr, index, ..
            } => self.subscript(dest, loc, expr, index, vartable, results),
            codegen::Expression::Subtract {
                loc,
                left,
                right,
                overflowing,
                ty,
                ..
            } => {
                let operator = BinaryOperator::Sub {
                    overflowing: *overflowing,
                };
                self.binary_operation(dest, loc, ty, operator, left, right, vartable, results)
            }
            codegen::Expression::Trunc { loc, ty, expr, .. } => {
                self.trunc(dest, loc, ty, expr, vartable, results)
            }
            codegen::Expression::Negate {
                loc,
                expr,
                overflowing,
                ..
            } => {
                let operator = UnaryOperator::Neg {
                    overflowing: *overflowing,
                };
                self.unary_operation(dest, loc, operator, expr, vartable, results)
            }
            codegen::Expression::Undefined { .. } => {
                panic!("Undefined expression shouldn't be here")
            }
            codegen::Expression::Variable { loc, var_no, .. } => {
                self.variable(dest, loc, var_no, results)
            }
            codegen::Expression::ZeroExt { loc, ty, expr, .. } => {
                self.zero_ext(dest, loc, ty, expr, vartable, results)
            }
            codegen::Expression::AdvancePointer {
                pointer,
                bytes_offset,
                ..
            } => self.advance_pointer(dest, pointer, bytes_offset, vartable, results),

            codegen::Expression::VectorData { pointer } => {
                self.pointer_position(dest, pointer, vartable, results)
            }
        }
    }

    fn pointer_position(
        &self,
        dest: &Operand,
        pointer: &codegen::Expression,
        vartable: &mut Vartable,
        results: &mut Vec<Instruction>,
    ) {
        let pointer_op = self.to_operand_and_insns(pointer, vartable, results);
        results.push(Instruction::Set {
            loc: Loc::Codegen,
            res: dest.get_id_or_error(),
            expr: Expression::VectorData {
                pointer: Box::new(pointer_op),
            },
        });
    }

    fn advance_pointer(
        &self,
        dest: &Operand,
        pointer: &codegen::Expression,
        bytes_offset: &codegen::Expression,
        vartable: &mut Vartable,
        results: &mut Vec<Instruction>,
    ) {
        let pointer_op = self.to_operand_and_insns(pointer, vartable, results);
        let bytes_offset_op = self.to_operand_and_insns(bytes_offset, vartable, results);
        results.push(Instruction::Set {
            loc: Loc::Codegen,
            res: dest.get_id_or_error(),
            expr: Expression::AdvancePointer {
                loc: /*missing from cfg*/ Loc::Codegen,
                pointer: Box::new(pointer_op),
                bytes_offset: Box::new(bytes_offset_op),
            },
        });
    }

    fn zero_ext(
        &self,
        dest: &Operand,
        loc: &Loc,
        ty: &ast::Type,
        expr: &codegen::Expression,
        vartable: &mut Vartable,
        results: &mut Vec<Instruction>,
    ) {
        let from_op = self.to_operand_and_insns(expr, vartable, results);
        results.push(Instruction::Set {
            loc: *loc,
            res: dest.get_id_or_error(),
            expr: Expression::ZeroExt {
                loc: *loc,
                operand: Box::new(from_op),
                to_ty: self.lower_ast_type(ty),
            },
        });
    }

    fn trunc(
        &self,
        dest: &Operand,
        loc: &Loc,
        ty: &ast::Type,
        expr: &codegen::Expression,
        vartable: &mut Vartable,
        results: &mut Vec<Instruction>,
    ) {
        let from_op = self.to_operand_and_insns(expr, vartable, results);
        results.push(Instruction::Set {
            loc: *loc,
            res: dest.get_id_or_error(),
            expr: Expression::Trunc {
                loc: *loc,
                operand: Box::new(from_op),
                to_ty: self.lower_ast_type(ty),
            },
        });
    }

    fn subscript(
        &self,
        dest: &Operand,
        loc: &Loc,
        expr: &codegen::Expression,
        index: &codegen::Expression,
        vartable: &mut Vartable,
        results: &mut Vec<Instruction>,
    ) {
        let array_op = self.to_operand_and_insns(expr, vartable, results);
        let index_op = self.to_operand_and_insns(index, vartable, results);
        results.push(Instruction::Set {
            loc: *loc,
            res: dest.get_id_or_error(),
            expr: Expression::Subscript {
                loc: *loc,
                arr: Box::new(array_op),
                index: Box::new(index_op),
            },
        });
    }

    fn struct_member(
        &self,
        dest: &Operand,
        loc: &Loc,
        expr: &codegen::Expression,
        member: &usize,
        vartable: &mut Vartable,
        results: &mut Vec<Instruction>,
    ) {
        let struct_op = self.to_operand_and_insns(expr, vartable, results);
        results.push(Instruction::Set {
            loc: *loc,
            res: dest.get_id_or_error(),
            expr: Expression::StructMember {
                loc: *loc,
                operand: Box::new(struct_op),
                member: *member,
            },
        });
    }

    fn struct_literal(
        &self,
        dest: &Operand,
        loc: &Loc,
        ty: &ast::Type,
        values: &[codegen::Expression],
        vartable: &mut Vartable,
        results: &mut Vec<Instruction>,
    ) {
        let value_ops = values
            .iter()
            .map(|value| self.to_operand_and_insns(value, vartable, results))
            .collect::<Vec<Operand>>();

        results.push(Instruction::Set {
            loc: *loc,
            res: dest.get_id_or_error(),
            expr: Expression::StructLiteral {
                loc: *loc,
                ty: self.lower_ast_type(ty),
                values: value_ops,
            },
        });
    }

    fn string_compare(
        &self,
        dest: &Operand,
        loc: &Loc,
        left: &ast::StringLocation<codegen::Expression>,
        right: &ast::StringLocation<codegen::Expression>,
        vartable: &mut Vartable,
        results: &mut Vec<Instruction>,
    ) {
        let left_string_loc = self.to_string_location_and_insns(left, vartable, results);
        let right_string_loc = self.to_string_location_and_insns(right, vartable, results);

        results.push(Instruction::Set {
            loc: *loc,
            res: dest.get_id_or_error(),
            expr: Expression::StringCompare {
                loc: *loc,
                left: left_string_loc,
                right: right_string_loc,
            },
        });
    }

    fn storage_array_length(
        &self,
        dest: &Operand,
        loc: &Loc,
        array: &codegen::Expression,
        vartable: &mut Vartable,
        results: &mut Vec<Instruction>,
    ) {
        let array_op = self.to_operand_and_insns(array, vartable, results);
        results.push(Instruction::Set {
            loc: *loc,
            res: dest.get_id_or_error(),
            expr: Expression::StorageArrayLength {
                loc: *loc,
                array: Box::new(array_op),
            },
        });
    }

    fn sign_ext(
        &self,
        dest: &Operand,
        loc: &Loc,
        ty: &ast::Type,
        expr: &codegen::Expression,
        vartable: &mut Vartable,
        results: &mut Vec<Instruction>,
    ) {
        let tmp = self.to_operand_and_insns(expr, vartable, results);
        let sext = Expression::SignExt {
            loc: *loc,
            operand: Box::new(tmp),
            to_ty: self.lower_ast_type(ty),
        };
        results.push(Instruction::Set {
            loc: *loc,
            res: dest.get_id_or_error(),
            expr: sext,
        });
    }

    fn load(
        &self,
        dest: &Operand,
        loc: &Loc,
        expr: &codegen::Expression,
        vartable: &mut Vartable,
        results: &mut Vec<Instruction>,
    ) {
        let from_op = self.to_operand_and_insns(expr, vartable, results);
        results.push(Instruction::Set {
            loc: *loc,
            res: dest.get_id_or_error(),
            expr: Expression::Load {
                loc: *loc,
                operand: Box::new(from_op),
            },
        });
    }

    fn keccak256(
        &self,
        dest: &Operand,
        loc: &Loc,
        exprs: &Vec<codegen::Expression>,
        vartable: &mut Vartable,
        results: &mut Vec<Instruction>,
    ) {
        let mut expr_ops = vec![];
        for expr in exprs {
            let op = self.to_operand_and_insns(expr, vartable, results);
            expr_ops.push(op);
        }
        results.push(Instruction::Set {
            loc: *loc,
            res: dest.get_id_or_error(),
            expr: Expression::Keccak256 {
                loc: *loc,
                args: expr_ops,
            },
        });
    }

    fn get_ref(
        &self,
        dest: &Operand,
        loc: &Loc,
        expr: &codegen::Expression,
        vartable: &mut Vartable,
        results: &mut Vec<Instruction>,
    ) {
        let from_op = self.to_operand_and_insns(expr, vartable, results);
        results.push(Instruction::Set {
            loc: *loc,
            res: dest.get_id_or_error(),
            expr: Expression::GetRef {
                loc: *loc,
                operand: Box::new(from_op),
            },
        });
    }

    fn function_arg(
        &self,
        dest: &Operand,
        loc: &Loc,
        ty: &ast::Type,
        arg_no: &usize,
        vartable: &mut Vartable,
        results: &mut Vec<Instruction>,
    ) {
        let arg_ty = self.lower_ast_type(ty);
        let expr = Expression::FunctionArg {
            loc: *loc,
            ty: arg_ty,
            arg_no: *arg_no,
        };
        let res = dest.get_id_or_error();
        vartable.add_function_arg(*arg_no, res);
        results.push(Instruction::Set {
            loc: *loc,
            res,
            expr,
        });
    }

    fn format_string(
        &self,
        dest: &Operand,
        loc: &Loc,
        args: &Vec<(ast::FormatArg, codegen::Expression)>,
        vartable: &mut Vartable,
        results: &mut Vec<Instruction>,
    ) {
        let mut arg_ops = vec![];
        for (format, arg) in args {
            let op = self.to_operand_and_insns(arg, vartable, results);
            arg_ops.push((*format, op));
        }
        results.push(Instruction::Set {
            loc: *loc,
            res: dest.get_id_or_error(),
            expr: Expression::FormatString {
                loc: *loc,
                args: arg_ops,
            },
        });
    }

    fn const_array_literal(
        &self,
        dest: &Operand,
        loc: &Loc,
        ty: &ast::Type,
        dimensions: &[u32],
        values: &[codegen::Expression],
        vartable: &mut Vartable,
        results: &mut Vec<Instruction>,
    ) {
        let value_ops = values
            .iter()
            .map(|value| self.to_operand_and_insns(value, vartable, results))
            .collect::<Vec<Operand>>();

        results.push(Instruction::Set {
            loc: *loc,
            res: dest.get_id_or_error(),
            expr: Expression::ConstArrayLiteral {
                loc: *loc,
                ty: self.lower_ast_type(ty),
                dimensions: dimensions.to_owned(),
                values: value_ops,
            },
        });
    }

    fn cast(
        &self,
        dest: &Operand,
        loc: &Loc,
        ty: &ast::Type,
        expr: &codegen::Expression,
        vartable: &mut Vartable,
        results: &mut Vec<Instruction>,
    ) {
        let from_op = self.to_operand_and_insns(expr, vartable, results);
        results.push(Instruction::Set {
            loc: *loc,
            res: dest.get_id_or_error(),
            expr: Expression::Cast {
                loc: *loc,
                operand: Box::new(from_op),
                to_ty: self.lower_ast_type(ty),
            },
        });
    }

    fn byte_cast(
        &self,
        dest: &Operand,
        loc: &Loc,
        expr: &codegen::Expression,
        ty: &ast::Type,
        vartable: &mut Vartable,
        results: &mut Vec<Instruction>,
    ) {
        let from_op = self.to_operand_and_insns(expr, vartable, results);
        results.push(Instruction::Set {
            loc: *loc,
            res: dest.get_id_or_error(),
            expr: Expression::BytesCast {
                loc: *loc,
                operand: Box::new(from_op),
                to_ty: self.lower_ast_type(ty),
            },
        });
    }

    fn builtin(
        &self,
        dest: &Operand,
        loc: &Loc,
        kind: &crate::codegen::Builtin,
        args: &Vec<codegen::Expression>,
        vartable: &mut Vartable,
        results: &mut Vec<Instruction>,
    ) {
        let mut arg_ops = vec![];
        for arg in args {
            let op = self.to_operand_and_insns(arg, vartable, results);
            arg_ops.push(op);
        }
        results.push(Instruction::Set {
            loc: *loc,
            res: dest.get_id_or_error(),
            expr: Expression::Builtin {
                loc: *loc,
                kind: *kind,
                args: arg_ops,
            },
        });
    }

    fn binary_operation(
        &self,
        dest: &Operand,
        loc: &Loc,
        _: &ast::Type,
        operator: BinaryOperator,
        left: &codegen::Expression,
        right: &codegen::Expression,
        vartable: &mut Vartable,
        results: &mut Vec<Instruction>,
    ) {
        let left_op = self.to_operand_and_insns(left, vartable, results);
        let right_op = self.to_operand_and_insns(right, vartable, results);
        results.push(Instruction::Set {
            loc: *loc,
            res: dest.get_id_or_error(),
            expr: Expression::BinaryExpr {
                loc: *loc,
                operator,
                left: Box::new(left_op),
                right: Box::new(right_op),
            },
        });
    }

    fn unary_operation(
        &self,
        dest: &Operand,
        loc: &Loc,
        operator: UnaryOperator,
        expr: &codegen::Expression,
        vartable: &mut Vartable,
        results: &mut Vec<Instruction>,
    ) {
        let expr_op = self.to_operand_and_insns(expr, vartable, results);
        results.push(Instruction::Set {
            loc: *loc,
            res: dest.get_id_or_error(),
            expr: Expression::UnaryExpr {
                loc: *loc,
                operator,
                right: Box::new(expr_op),
            },
        });
    }

    fn alloc_dynamic_bytes(
        &self,
        dest: &Operand,
        loc: &Loc,
        ty: &ast::Type,
        size: &codegen::Expression,
        initializer: &Option<Vec<u8>>,
        vartable: &mut Vartable,
        results: &mut Vec<Instruction>,
    ) {
        let size_op = self.to_operand_and_insns(size, vartable, results);
        results.push(Instruction::Set {
            loc: *loc,
            res: dest.get_id_or_error(),
            expr: Expression::AllocDynamicBytes {
                loc: *loc,
                ty: self.lower_ast_type(ty),
                size: Box::new(size_op),
                initializer: initializer.clone(),
            },
        });
    }

    fn bytes_literal(
        &self,
        dest: &Operand,
        loc: &Loc,
        ty: &ast::Type,
        value: &[u8],
        results: &mut Vec<Instruction>,
    ) {
        let expr = Expression::BytesLiteral {
            loc: *loc,
            ty: self.lower_ast_type(ty),
            value: value.to_owned(),
        };
        results.push(Instruction::Set {
            loc: *loc,
            res: dest.get_id_or_error(),
            expr,
        });
    }

    fn bool_literal(
        &self,
        dest: &Operand,
        loc: &Loc,
        value: &bool,
        results: &mut Vec<Instruction>,
    ) {
        let expr = Expression::BoolLiteral {
            loc: *loc,
            value: *value,
        };
        results.push(Instruction::Set {
            loc: *loc,
            res: dest.get_id_or_error(),
            expr,
        });
    }

    fn number_literal(
        &self,
        dest: &Operand,
        loc: &Loc,
        value: &num_bigint::BigInt,
        results: &mut Vec<Instruction>,
    ) {
        results.push(
            // assign the constant value to the destination
            Instruction::Set {
                loc: *loc,
                res: dest.get_id_or_error(),
                expr: Expression::NumberLiteral {
                    loc: *loc,
                    value: value.clone(),
                },
            },
        );
    }

    fn array_literal(
        &self,
        dest: &Operand,
        loc: &Loc,
        ty: &ast::Type,
        dimensions: &[u32],
        values: &[codegen::Expression],
        vartable: &mut Vartable,
        results: &mut Vec<Instruction>,
    ) {
        let value_ops = values
            .iter()
            .map(|value| self.to_operand_and_insns(value, vartable, results))
            .collect::<Vec<Operand>>();

        results.push(Instruction::Set {
            loc: *loc,
            res: dest.get_id_or_error(),
            expr: Expression::ArrayLiteral {
                loc: *loc,
                ty: self.lower_ast_type(ty),
                dimensions: dimensions.to_owned(),
                values: value_ops,
            },
        });
    }

    fn internal_function_cfg(
        &self,
        dest: &Operand,
        cfg_no: &usize,
        results: &mut Vec<Instruction>,
    ) {
        let expr = Expression::InternalFunctionCfg { loc: /*missing from cfg*/ Loc::Codegen, cfg_no: *cfg_no };
        results.push(Instruction::Set {
            loc: Loc::Codegen,
            res: dest.get_id_or_error(),
            expr,
        });
    }

    fn variable(&self, dest: &Operand, loc: &Loc, var_no: &usize, results: &mut Vec<Instruction>) {
        let expr = Expression::Id {
            loc: *loc,
            id: *var_no,
        };
        results.push(Instruction::Set {
            loc: *loc,
            res: dest.get_id_or_error(),
            expr,
        });
    }

    fn return_data(&self, dest: &Operand, loc: &Loc, results: &mut Vec<Instruction>) {
        results.push(Instruction::Set {
            loc: *loc,
            res: dest.get_id_or_error(),
            expr: Expression::ReturnData { loc: *loc },
        });
    }
}
