// SPDX-License-Identifier: Apache-2.0

#![cfg(test)]

use crate::codegen::cfg::ControlFlowGraph;
use crate::codegen::vartable::Vartable;
use crate::codegen::yul::expression::expression;
use crate::codegen::{Builtin, Expression, Options};
use crate::sema::ast::ArrayLength;
use crate::sema::ast::{Contract, Layout, Mutability, Namespace, Type, Variable};
use crate::sema::yul::ast;
use crate::sema::yul::ast::YulSuffix;
use crate::{sema, Target};
use num_bigint::{BigInt, Sign};
use once_cell::unsync::OnceCell;
use solang_parser::pt::{self, ContractTy, Loc, StorageLocation, Visibility};

#[test]
fn bool_literal() {
    let loc = Loc::File(1, 2, 3);
    let ns = Namespace::new(Target::Solana);
    let mut vartab = Vartable::new(2);
    let mut cfg = ControlFlowGraph::placeholder();
    let opt = Options::default();

    let expr = ast::YulExpression::BoolLiteral(loc, true, Type::Bool);
    let res = expression(&expr, 0, &ns, &mut vartab, &mut cfg, &opt);
    assert_eq!(res, Expression::BoolLiteral { loc, value: true });

    let expr = ast::YulExpression::BoolLiteral(loc, true, Type::Uint(32));
    let res = expression(&expr, 0, &ns, &mut vartab, &mut cfg, &opt);
    assert_eq!(
        res,
        Expression::NumberLiteral {
            loc,
            ty: Type::Uint(32),
            value: BigInt::from(1)
        }
    );

    let expr = ast::YulExpression::BoolLiteral(loc, false, Type::Uint(32));
    let res = expression(&expr, 0, &ns, &mut vartab, &mut cfg, &opt);
    assert_eq!(
        res,
        Expression::NumberLiteral {
            loc,
            ty: Type::Uint(32),
            value: BigInt::from(0)
        }
    );
}

#[test]
fn number_literal() {
    let loc = Loc::File(1, 2, 3);
    let ns = Namespace::new(Target::Solana);
    let mut vartab = Vartable::new(2);
    let mut cfg = ControlFlowGraph::placeholder();
    let opt = Options::default();

    let expr = ast::YulExpression::NumberLiteral(loc, BigInt::from(32), Type::Uint(256));
    let res = expression(&expr, 0, &ns, &mut vartab, &mut cfg, &opt);
    assert_eq!(
        res,
        Expression::NumberLiteral {
            loc,
            ty: Type::Uint(256),
            value: BigInt::from(32)
        }
    );
}

#[test]
fn string_literal() {
    let loc = Loc::File(1, 2, 3);
    let ns = Namespace::new(Target::Solana);
    let mut vartab = Vartable::new(2);
    let mut cfg = ControlFlowGraph::placeholder();
    let opt = Options::default();

    let expr = ast::YulExpression::StringLiteral(loc, vec![0, 3, 255, 127], Type::Uint(128));
    let res = expression(&expr, 0, &ns, &mut vartab, &mut cfg, &opt);
    assert_eq!(
        res,
        Expression::NumberLiteral {
            loc,
            ty: Type::Uint(128),
            value: BigInt::from_bytes_be(Sign::Plus, &[0, 3, 255, 127])
        }
    );
}

#[test]
fn yul_local_variable() {
    let loc = Loc::File(1, 2, 3);
    let ns = Namespace::new(Target::Solana);
    let mut vartab = Vartable::new(2);
    let mut cfg = ControlFlowGraph::placeholder();
    let opt = Options::default();

    let expr = ast::YulExpression::YulLocalVariable(loc, Type::Int(16), 5);
    let res = expression(&expr, 0, &ns, &mut vartab, &mut cfg, &opt);
    assert_eq!(
        res,
        Expression::Variable {
            loc,
            ty: Type::Int(16),
            var_no: 5
        }
    );
}

#[test]
fn contract_constant_variable() {
    let loc = Loc::File(1, 2, 3);
    let mut ns = Namespace::new(Target::Solana);
    let mut vartab = Vartable::new(2);
    let mut cfg = ControlFlowGraph::placeholder();
    let opt = Options::default();

    let var = Variable {
        tags: vec![],
        name: "const_var".to_string(),
        loc,
        ty: Type::Uint(64),
        visibility: Visibility::Public(None),
        constant: false,
        immutable: false,
        initializer: Some(sema::ast::Expression::NumberLiteral {
            loc,
            ty: Type::Uint(64),
            value: BigInt::from(64),
        }),
        assigned: false,
        read: false,
        storage_type: None,
    };

    let contract = Contract {
        tags: vec![],
        loc,
        ty: ContractTy::Contract(loc),
        id: pt::Identifier {
            name: "".to_string(),
            loc: pt::Loc::Codegen,
        },
        bases: vec![],
        using: vec![],
        layout: vec![],
        fixed_layout_size: Default::default(),
        functions: vec![],
        all_functions: Default::default(),
        virtual_functions: Default::default(),
        yul_functions: vec![],
        variables: vec![var],
        creates: vec![],
        emits_events: vec![],
        initializer: None,
        default_constructor: None,
        cfg: vec![],
        code: OnceCell::new(),
        instantiable: true,
        program_id: None,
    };
    ns.contracts.push(contract);

    let expr = ast::YulExpression::ConstantVariable(loc, Type::Uint(64), Some(0), 0);
    let res = expression(&expr, 0, &ns, &mut vartab, &mut cfg, &opt);
    assert_eq!(
        res,
        Expression::NumberLiteral {
            loc,
            ty: Type::Uint(64),
            value: BigInt::from(64)
        }
    );
}

#[test]
fn global_constant_variable() {
    let loc = Loc::File(1, 2, 3);
    let mut ns = Namespace::new(Target::Solana);
    let mut vartab = Vartable::new(2);
    let mut cfg = ControlFlowGraph::placeholder();
    let opt = Options::default();

    let var = Variable {
        tags: vec![],
        name: "const_var".to_string(),
        loc,
        ty: Type::Uint(64),
        visibility: Visibility::Public(None),
        constant: false,
        immutable: false,
        initializer: Some(sema::ast::Expression::NumberLiteral {
            loc,
            ty: Type::Uint(64),
            value: BigInt::from(64),
        }),
        assigned: false,
        read: false,
        storage_type: None,
    };
    ns.constants.push(var);
    let expr = ast::YulExpression::ConstantVariable(loc, Type::Uint(64), None, 0);
    let res = expression(&expr, 0, &ns, &mut vartab, &mut cfg, &opt);
    assert_eq!(
        res,
        Expression::NumberLiteral {
            loc,
            ty: Type::Uint(64),
            value: BigInt::from(64)
        }
    );
}

#[test]
#[should_panic]
fn storage_variable() {
    let loc = Loc::File(1, 2, 3);
    let ns = Namespace::new(Target::Solana);
    let mut vartab = Vartable::new(2);
    let mut cfg = ControlFlowGraph::placeholder();
    let opt = Options::default();

    let expr = ast::YulExpression::StorageVariable(loc, Type::Bool, 0, 0);
    let _ = expression(&expr, 0, &ns, &mut vartab, &mut cfg, &opt);
}

#[test]
#[should_panic]
fn storage_variable_reference() {
    let loc = Loc::File(1, 2, 3);
    let ns = Namespace::new(Target::Solana);
    let mut vartab = Vartable::new(2);
    let mut cfg = ControlFlowGraph::placeholder();
    let opt = Options::default();

    let expr = ast::YulExpression::SolidityLocalVariable(
        loc,
        Type::Int(32),
        Some(StorageLocation::Storage(loc)),
        0,
    );
    let _ = expression(&expr, 0, &ns, &mut vartab, &mut cfg, &opt);
}

#[test]
fn solidity_local_variable() {
    let loc = Loc::File(1, 2, 3);
    let ns = Namespace::new(Target::Solana);
    let mut vartab = Vartable::new(2);
    let mut cfg = ControlFlowGraph::placeholder();
    let opt = Options::default();

    let expr = ast::YulExpression::SolidityLocalVariable(loc, Type::Uint(32), None, 7);
    let res = expression(&expr, 0, &ns, &mut vartab, &mut cfg, &opt);
    assert_eq!(
        res,
        Expression::Variable {
            loc,
            ty: Type::Uint(32),
            var_no: 7
        }
    );
}

#[test]
fn slot_suffix() {
    let mut ns = Namespace::new(Target::Solana);
    let loc = Loc::File(1, 2, 3);
    let layout = Layout {
        slot: BigInt::from(2),
        contract_no: 0,
        var_no: 0,
        ty: Type::Uint(256),
    };
    let contract = Contract {
        tags: vec![],
        loc: Loc::Builtin,
        ty: ContractTy::Contract(loc),
        id: pt::Identifier {
            name: "".to_string(),
            loc: pt::Loc::Builtin,
        },
        bases: vec![],
        using: vec![],
        layout: vec![layout],
        fixed_layout_size: Default::default(),
        functions: vec![],
        all_functions: Default::default(),
        virtual_functions: Default::default(),
        yul_functions: vec![],
        variables: vec![],
        creates: vec![],
        emits_events: vec![],
        initializer: None,
        default_constructor: None,
        cfg: vec![],
        code: OnceCell::new(),
        instantiable: true,
        program_id: None,
    };
    ns.contracts.push(contract);

    let mut vartab = Vartable::new(2);
    let mut cfg = ControlFlowGraph::placeholder();
    let opt = Options::default();

    let expr = ast::YulExpression::SuffixAccess(
        loc,
        Box::new(ast::YulExpression::StorageVariable(
            loc,
            Type::Uint(256),
            0,
            0,
        )),
        YulSuffix::Slot,
    );
    let res = expression(&expr, 0, &ns, &mut vartab, &mut cfg, &opt);
    assert_eq!(
        res,
        Expression::NumberLiteral {
            loc: Loc::File(1, 2, 3),
            ty: Type::Uint(256),
            value: BigInt::from(2)
        }
    );

    let expr = ast::YulExpression::SuffixAccess(
        loc,
        Box::new(ast::YulExpression::SolidityLocalVariable(
            loc,
            Type::Uint(16),
            Some(StorageLocation::Storage(loc)),
            0,
        )),
        YulSuffix::Slot,
    );
    let res = expression(&expr, 0, &ns, &mut vartab, &mut cfg, &opt);
    assert_eq!(
        res,
        Expression::Variable {
            loc,
            ty: Type::Uint(256),
            var_no: 0
        }
    );
}

#[test]
#[should_panic]
fn slot_suffix_panic() {
    let ns = Namespace::new(Target::Solana);
    let loc = Loc::File(1, 2, 3);
    let mut vartab = Vartable::new(2);
    let mut cfg = ControlFlowGraph::placeholder();
    let opt = Options::default();

    let expr = ast::YulExpression::SuffixAccess(
        loc,
        Box::new(ast::YulExpression::SolidityLocalVariable(
            loc,
            Type::Int(32),
            None,
            2,
        )),
        YulSuffix::Slot,
    );

    let _res = expression(&expr, 0, &ns, &mut vartab, &mut cfg, &opt);
}

#[test]
fn offset_suffix() {
    let ns = Namespace::new(Target::Solana);
    let loc = Loc::File(1, 2, 3);
    let mut vartab = Vartable::new(2);
    let mut cfg = ControlFlowGraph::placeholder();
    let opt = Options::default();

    let expr = ast::YulExpression::SuffixAccess(
        loc,
        Box::new(ast::YulExpression::StorageVariable(
            loc,
            Type::Int(32),
            1,
            0,
        )),
        YulSuffix::Offset,
    );

    let res = expression(&expr, 0, &ns, &mut vartab, &mut cfg, &opt);
    assert_eq!(
        res,
        Expression::NumberLiteral {
            loc: Loc::Codegen,
            ty: Type::Uint(256),
            value: BigInt::from(0)
        }
    );

    let expr = ast::YulExpression::SuffixAccess(
        loc,
        Box::new(ast::YulExpression::SolidityLocalVariable(
            loc,
            Type::Uint(24),
            Some(StorageLocation::Storage(loc)),
            0,
        )),
        YulSuffix::Offset,
    );
    let res = expression(&expr, 0, &ns, &mut vartab, &mut cfg, &opt);
    assert_eq!(
        res,
        Expression::NumberLiteral {
            loc: Loc::Codegen,
            ty: Type::Uint(256),
            value: BigInt::from(0)
        }
    );

    let expr = ast::YulExpression::SuffixAccess(
        loc,
        Box::new(ast::YulExpression::SolidityLocalVariable(
            loc,
            Type::Array(Box::new(Type::Uint(256)), vec![ArrayLength::Dynamic]),
            Some(StorageLocation::Calldata(loc)),
            1,
        )),
        YulSuffix::Offset,
    );
    let res = expression(&expr, 0, &ns, &mut vartab, &mut cfg, &opt);
    assert_eq!(
        res,
        Expression::Cast {
            loc,
            ty: Type::Uint(256),
            expr: Box::new(Expression::Variable {
                loc,
                ty: Type::Array(Box::new(Type::Uint(256)), vec![ArrayLength::Dynamic]),
                var_no: 1
            })
        }
    );
}

#[test]
#[should_panic]
fn offset_suffix_panic_calldata() {
    let ns = Namespace::new(Target::Solana);
    let loc = Loc::File(1, 2, 3);
    let mut vartab = Vartable::new(2);
    let mut cfg = ControlFlowGraph::placeholder();
    let opt = Options::default();

    let expr = ast::YulExpression::SuffixAccess(
        loc,
        Box::new(ast::YulExpression::SolidityLocalVariable(
            loc,
            Type::Array(
                Box::new(Type::Uint(32)),
                vec![ArrayLength::Dynamic, ArrayLength::Fixed(BigInt::from(3))],
            ),
            Some(StorageLocation::Calldata(loc)),
            3,
        )),
        YulSuffix::Offset,
    );

    let _res = expression(&expr, 0, &ns, &mut vartab, &mut cfg, &opt);
}

#[test]
#[should_panic]
fn offset_suffix_panic_other() {
    let ns = Namespace::new(Target::Solana);
    let loc = Loc::File(1, 2, 3);
    let mut vartab = Vartable::new(2);
    let mut cfg = ControlFlowGraph::placeholder();
    let opt = Options::default();

    let expr = ast::YulExpression::SuffixAccess(
        loc,
        Box::new(ast::YulExpression::YulLocalVariable(loc, Type::Int(32), 3)),
        YulSuffix::Offset,
    );

    let _res = expression(&expr, 0, &ns, &mut vartab, &mut cfg, &opt);
}

#[test]
fn length_suffix() {
    let ns = Namespace::new(Target::Solana);
    let loc = Loc::File(1, 2, 3);
    let mut vartab = Vartable::new(2);
    let mut cfg = ControlFlowGraph::placeholder();
    let opt = Options::default();

    let expr = ast::YulExpression::SuffixAccess(
        loc,
        Box::new(ast::YulExpression::SolidityLocalVariable(
            loc,
            Type::Array(
                Box::new(Type::Uint(32)),
                vec![
                    ArrayLength::Dynamic,
                    ArrayLength::Fixed(BigInt::from(3)),
                    ArrayLength::Dynamic,
                ],
            ),
            Some(StorageLocation::Calldata(loc)),
            3,
        )),
        YulSuffix::Length,
    );

    let res = expression(&expr, 0, &ns, &mut vartab, &mut cfg, &opt);
    assert_eq!(
        res,
        Expression::Builtin {
            loc,
            tys: vec![Type::Uint(32)],
            kind: Builtin::ArrayLength,
            args: vec![Expression::Variable {
                loc,
                ty: Type::Array(
                    Box::new(Type::Uint(32)),
                    vec![
                        ArrayLength::Dynamic,
                        ArrayLength::Fixed(BigInt::from(3)),
                        ArrayLength::Dynamic
                    ]
                ),
                var_no: 3
            }]
        }
    );
}

#[test]
#[should_panic]
fn length_suffix_panic() {
    let ns = Namespace::new(Target::Solana);
    let loc = Loc::File(1, 2, 3);
    let mut vartab = Vartable::new(2);
    let mut cfg = ControlFlowGraph::placeholder();
    let opt = Options::default();

    let expr = ast::YulExpression::SuffixAccess(
        loc,
        Box::new(ast::YulExpression::SolidityLocalVariable(
            loc,
            Type::Array(
                Box::new(Type::Uint(32)),
                vec![ArrayLength::Dynamic, ArrayLength::Fixed(BigInt::from(3))],
            ),
            Some(StorageLocation::Calldata(loc)),
            3,
        )),
        YulSuffix::Length,
    );

    let res = expression(&expr, 0, &ns, &mut vartab, &mut cfg, &opt);
    assert_eq!(
        res,
        Expression::Builtin {
            loc,
            tys: vec![Type::Uint(256)],
            kind: Builtin::ArrayLength,
            args: vec![Expression::Variable {
                loc,
                ty: Type::Array(
                    Box::new(Type::Uint(32)),
                    vec![
                        ArrayLength::Dynamic,
                        ArrayLength::Fixed(BigInt::from(3)),
                        ArrayLength::Dynamic
                    ]
                ),
                var_no: 3
            }]
        }
    );
}

#[test]
fn selector_suffix() {
    let ns = Namespace::new(Target::Solana);
    let loc = Loc::File(1, 2, 3);
    let mut vartab = Vartable::new(2);
    let mut cfg = ControlFlowGraph::placeholder();
    let opt = Options::default();

    let expr = ast::YulExpression::SuffixAccess(
        loc,
        Box::new(ast::YulExpression::SolidityLocalVariable(
            loc,
            Type::ExternalFunction {
                mutability: Mutability::Pure(loc),
                params: vec![],
                returns: vec![],
            },
            None,
            4,
        )),
        YulSuffix::Selector,
    );
    let res = expression(&expr, 0, &ns, &mut vartab, &mut cfg, &opt);

    assert_eq!(
        res,
        Expression::Load {
            loc,
            ty: Type::FunctionSelector,
            expr: Box::new(Expression::StructMember {
                loc,
                ty: Type::Ref(Box::new(Type::FunctionSelector)),
                expr: Box::new(Expression::Variable {
                    loc,
                    ty: Type::ExternalFunction {
                        mutability: Mutability::Pure(loc),
                        params: vec![],
                        returns: vec![],
                    },
                    var_no: 4
                }),
                member: 0
            })
        }
    );
}

#[test]
#[should_panic]
fn selector_suffix_panic() {
    let ns = Namespace::new(Target::Solana);
    let loc = Loc::File(1, 2, 3);
    let mut vartab = Vartable::new(2);
    let mut cfg = ControlFlowGraph::placeholder();
    let opt = Options::default();

    let expr = ast::YulExpression::SuffixAccess(
        loc,
        Box::new(ast::YulExpression::SolidityLocalVariable(
            loc,
            Type::Bool,
            None,
            4,
        )),
        YulSuffix::Selector,
    );
    let _res = expression(&expr, 0, &ns, &mut vartab, &mut cfg, &opt);
}

#[test]
fn address_suffix() {
    let ns = Namespace::new(Target::Solana);
    let loc = Loc::File(1, 2, 3);
    let mut vartab = Vartable::new(2);
    let mut cfg = ControlFlowGraph::placeholder();
    let opt = Options::default();

    let expr = ast::YulExpression::SuffixAccess(
        loc,
        Box::new(ast::YulExpression::SolidityLocalVariable(
            loc,
            Type::ExternalFunction {
                mutability: Mutability::Pure(loc),
                params: vec![],
                returns: vec![],
            },
            None,
            4,
        )),
        YulSuffix::Address,
    );
    let res = expression(&expr, 0, &ns, &mut vartab, &mut cfg, &opt);

    assert_eq!(
        res,
        Expression::Load {
            loc,
            ty: Type::Address(false),
            expr: Box::new(Expression::StructMember {
                loc,
                ty: Type::Ref(Box::new(Type::Address(false))),
                expr: Box::new(Expression::Variable {
                    loc,
                    ty: Type::ExternalFunction {
                        mutability: Mutability::Pure(loc),
                        params: vec![],
                        returns: vec![]
                    },
                    var_no: 4
                }),
                member: 1
            })
        }
    );
}

#[test]
#[should_panic]
fn address_suffix_panic() {
    let ns = Namespace::new(Target::Solana);
    let loc = Loc::File(1, 2, 3);
    let mut vartab = Vartable::new(2);
    let mut cfg = ControlFlowGraph::placeholder();
    let opt = Options::default();

    let expr = ast::YulExpression::SuffixAccess(
        loc,
        Box::new(ast::YulExpression::SolidityLocalVariable(
            loc,
            Type::Bool,
            None,
            4,
        )),
        YulSuffix::Address,
    );
    let _res = expression(&expr, 0, &ns, &mut vartab, &mut cfg, &opt);
}
