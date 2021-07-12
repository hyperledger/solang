use crate::parser::pt::Loc;
use crate::sema::ast::{
    Builtin, Diagnostic, ErrorType, Expression, Level, Namespace, Note, Statement,
};
use crate::sema::symtable::{Symtable, VariableUsage};
use crate::sema::{ast, symtable};

/// Mark variables as assigned, either in the symbol table (for local variables) or in the
/// Namespace (for storage variables)
pub fn assigned_variable(ns: &mut Namespace, exp: &Expression, symtable: &mut Symtable) {
    match &exp {
        Expression::StorageVariable(_, _, contract_no, offset) => {
            ns.contracts[*contract_no].variables[*offset].assigned = true;
        }

        Expression::Variable(_, _, offset) => {
            let var = symtable.vars.get_mut(offset).unwrap();
            (*var).assigned = true;
        }

        Expression::StructMember(_, _, str, _) => {
            assigned_variable(ns, str, symtable);
        }

        Expression::Subscript(_, _, array, index)
        | Expression::DynamicArraySubscript(_, _, array, index)
        | Expression::StorageBytesSubscript(_, array, index) => {
            assigned_variable(ns, array, symtable);
            used_variable(ns, index, symtable);
        }

        Expression::Trunc(_, _, var)
        | Expression::Cast(_, _, var)
        | Expression::BytesCast(_, _, _, var) => {
            assigned_variable(ns, var, symtable);
        }

        _ => {}
    }
}

/// Mark variables as used, either in the symbol table (for local variables) or in the
/// Namespace (for global constants and storage variables)
/// The functions handles complex expressions in a recursive fashion, such as array length call,
/// assign expressions and array subscripts.
pub fn used_variable(ns: &mut Namespace, exp: &Expression, symtable: &mut Symtable) {
    match &exp {
        Expression::StorageVariable(_, _, contract_no, offset) => {
            ns.contracts[*contract_no].variables[*offset].read = true;
        }

        Expression::Variable(_, _, offset) => {
            let var = symtable.vars.get_mut(offset).unwrap();
            (*var).read = true;
        }

        Expression::ConstantVariable(_, _, Some(contract_no), offset) => {
            ns.contracts[*contract_no].variables[*offset].read = true;
        }

        Expression::ConstantVariable(_, _, None, offset) => {
            ns.constants[*offset].read = true;
        }

        Expression::StructMember(_, _, str, _) => {
            used_variable(ns, str, symtable);
        }

        Expression::Subscript(_, _, array, index)
        | Expression::DynamicArraySubscript(_, _, array, index)
        | Expression::StorageBytesSubscript(_, array, index) => {
            used_variable(ns, array, symtable);
            used_variable(ns, index, symtable);
        }

        Expression::DynamicArrayLength(_, array)
        | Expression::StorageArrayLength {
            loc: _,
            ty: _,
            array,
            ..
        } => {
            used_variable(ns, array, symtable);
        }

        Expression::StorageLoad(_, _, expr)
        | Expression::SignExt(_, _, expr)
        | Expression::ZeroExt(_, _, expr)
        | Expression::Trunc(_, _, expr)
        | Expression::Cast(_, _, expr)
        | Expression::BytesCast(_, _, _, expr) => {
            used_variable(ns, expr, symtable);
        }

        _ => {}
    }
}

/// Mark function arguments as used. If the function is an attribute of another variable, mark the
/// usage of the latter as well
pub fn check_function_call(ns: &mut Namespace, exp: &Expression, symtable: &mut Symtable) {
    match &exp {
        Expression::InternalFunctionCall {
            loc: _,
            returns: _,
            function,
            args,
        } => {
            for arg in args {
                used_variable(ns, arg, symtable);
            }
            check_function_call(ns, function, symtable);
        }

        Expression::ExternalFunctionCall {
            loc: _,
            returns: _,
            function,
            args,
            value,
            gas,
        } => {
            for arg in args {
                used_variable(ns, arg, symtable);
            }
            used_variable(ns, gas, symtable);
            used_variable(ns, value, symtable);
            check_function_call(ns, function, symtable);
        }

        Expression::Constructor {
            loc: _,
            contract_no: _,
            constructor_no: _,
            args,
            gas,
            value,
            salt,
            space,
        } => {
            for arg in args {
                used_variable(ns, arg, symtable);
            }
            used_variable(ns, gas, symtable);
            if let Some(expr) = value {
                used_variable(ns, expr, symtable);
            }

            if let Some(expr) = salt {
                used_variable(ns, expr, symtable);
            }

            if let Some(expr) = space {
                used_variable(ns, expr, symtable);
            }
        }

        Expression::ExternalFunctionCallRaw {
            loc: _,
            ty: _,
            address,
            args,
            value,
            gas,
        } => {
            used_variable(ns, args, symtable);
            used_variable(ns, address, symtable);
            used_variable(ns, value, symtable);
            used_variable(ns, gas, symtable);
        }

        Expression::ExternalFunction {
            loc: _,
            ty: _,
            address,
            function_no,
        } => {
            used_variable(ns, address, symtable);
            if ns.functions[*function_no].is_accessor {
                let body = ns.functions[*function_no].body[0].clone();
                if let Statement::Return(_, exprs) = &body {
                    used_variable(ns, &exprs[0], symtable);
                }
            }
        }

        Expression::Builtin(_, _, expr_type, args) => match expr_type {
            Builtin::ArrayPush => {
                assigned_variable(ns, &args[0], symtable);
                if args.len() > 1 {
                    used_variable(ns, &args[1], symtable);
                }
            }

            _ => {
                for arg in args {
                    used_variable(ns, arg, symtable);
                }
            }
        },

        Expression::DynamicArrayPush(_, array, _, arg) => {
            assigned_variable(ns, array, symtable);
            used_variable(ns, arg, symtable);
        }

        Expression::DynamicArrayPop(_, array, _) => {
            used_variable(ns, array, symtable);
        }

        _ => {}
    }
}

/// Marks as used variables that appear in an expression with right and left hand side.
pub fn check_var_usage_expression(
    ns: &mut Namespace,
    left: &Expression,
    right: &Expression,
    symtable: &mut Symtable,
) {
    used_variable(ns, left, symtable);
    used_variable(ns, right, symtable);
}

/// Generate warnings for unused varibles
fn generate_unused_warning(loc: Loc, text: &str, notes: Vec<Note>) -> Diagnostic {
    Diagnostic {
        level: Level::Warning,
        ty: ErrorType::Warning,
        pos: Some(loc),
        message: text.parse().unwrap(),
        notes,
    }
}

/// Emit different warning types according to the function variable usage
pub fn emit_warning_local_variable(variable: &symtable::Variable) -> Option<Diagnostic> {
    match &variable.usage_type {
        VariableUsage::Parameter => {
            if !variable.read {
                return Some(generate_unused_warning(
                    variable.id.loc,
                    &format!(
                        "function parameter '{}' has never been read",
                        variable.id.name
                    ),
                    vec![],
                ));
            }
            None
        }

        VariableUsage::ReturnVariable => {
            if !variable.assigned {
                return Some(generate_unused_warning(
                    variable.id.loc,
                    &format!(
                        "return variable '{}' has never been assigned",
                        variable.id.name
                    ),
                    vec![],
                ));
            }
            None
        }

        VariableUsage::LocalVariable => {
            if !variable.assigned && !variable.read {
                return Some(generate_unused_warning(
                    variable.id.loc,
                    &format!(
                        "local variable '{}' has never been read nor assigned",
                        variable.id.name
                    ),
                    vec![],
                ));
            } else if variable.assigned && !variable.read {
                return Some(generate_unused_warning(
                    variable.id.loc,
                    &format!(
                        "local variable '{}' has been assigned, but never read",
                        variable.id.name
                    ),
                    vec![],
                ));
            } else if !variable.assigned && variable.read {
                return Some(generate_unused_warning(
                    variable.id.loc,
                    &format!(
                        "local variable '{}' has never been assigned a value, but has been read",
                        variable.id.name
                    ),
                    vec![],
                ));
            }
            None
        }

        VariableUsage::DestructureVariable => {
            if !variable.read {
                return Some(generate_unused_warning(
                    variable.id.loc,
                    &format!(
                        "destructure variable '{}' has never been used",
                        variable.id.name
                    ),
                    vec![],
                ));
            }

            None
        }

        VariableUsage::TryCatchReturns => {
            if !variable.read {
                return Some(generate_unused_warning(
                    variable.id.loc,
                    &format!(
                        "try-catch returns variable '{}' has never been read",
                        variable.id.name
                    ),
                    vec![],
                ));
            }

            None
        }

        VariableUsage::TryCatchErrorBytes => {
            if !variable.read {
                return Some(generate_unused_warning(
                    variable.id.loc,
                    &format!(
                        "try-catch error bytes '{}' has never been used",
                        variable.id.name
                    ),
                    vec![],
                ));
            }

            None
        }

        VariableUsage::TryCatchErrorString => {
            if !variable.read {
                return Some(generate_unused_warning(
                    variable.id.loc,
                    &format!(
                        "try-catch error string '{}' has never been used",
                        variable.id.name
                    ),
                    vec![],
                ));
            }

            None
        }
        VariableUsage::AnonymousReturnVariable => None,
    }
}

/// Emit warnings depending on the storage variable usage
fn emit_warning_contract_variables(variable: &ast::Variable) -> Option<Diagnostic> {
    if variable.assigned && !variable.read {
        return Some(generate_unused_warning(
            variable.loc,
            &format!(
                "storage variable '{}' has been assigned, but never read",
                variable.name
            ),
            vec![],
        ));
    } else if !variable.assigned && !variable.read {
        return Some(generate_unused_warning(
            variable.loc,
            &format!("storage variable '{}' has never been used", variable.name),
            vec![],
        ));
    }
    //Solidity attributes zero value to contract values that have never been assigned
    //There is no need to raise warning if we use them, as they have a valid value.

    None
}

/// Check for unused constants and storage variables
pub fn check_unused_namespace_variables(ns: &mut Namespace) {
    for contract in &ns.contracts {
        for variable in &contract.variables {
            if let Some(warning) = emit_warning_contract_variables(variable) {
                ns.diagnostics.push(warning);
            }
        }
    }

    //Global constants should have been initialized during declaration
    for constant in &ns.constants {
        if !constant.read {
            ns.diagnostics.push(generate_unused_warning(
                constant.loc,
                &format!("global constant '{}' has never been used", constant.name),
                vec![],
            ))
        }
    }
}

/// Check for unused events
pub fn check_unused_events(ns: &mut Namespace) {
    for event in &ns.events {
        if !event.used {
            ns.diagnostics.push(generate_unused_warning(
                event.loc,
                &format!("event '{}' has never been emitted", event.name),
                vec![],
            ))
        }
    }
}
