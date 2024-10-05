// SPDX-License-Identifier: Apache-2.0

use super::{
    annotions_not_allowed,
    ast::{
        Diagnostic, Expression, Function, Mapping, Namespace, Parameter, Statement, StructType,
        Symbol, Type, Variable,
    },
    contracts::is_base,
    diagnostics::Diagnostics,
    expression::{ExprContext, ResolveTo},
    symtable::{Symtable, VariableInitializer, VariableUsage},
    tags::resolve_tags,
    ContractDefinition,
};
use crate::sema::expression::resolve_expression::expression;
use crate::sema::namespace::ResolveTypeContext;
use crate::Target;
use solang_parser::{
    doccomment::DocComment,
    pt::{self, CodeLocation, OptionalCodeLocation},
};
use std::sync::Arc;

pub struct DelayedResolveInitializer<'a> {
    var_no: usize,
    contract_no: usize,
    initializer: &'a pt::Expression,
}

pub fn contract_variables<'a>(
    def: &'a ContractDefinition,
    file_no: usize,
    ns: &mut Namespace,
) -> Vec<DelayedResolveInitializer<'a>> {
    let mut symtable = Symtable::default();
    let mut delayed = Vec::new();

    for part in &def.parts {
        if let pt::ContractPart::VariableDefinition(ref s) = &part.part {
            annotions_not_allowed(&part.annotations, "variable", ns);

            if let Some(delay) = variable_decl(
                Some(def),
                s,
                file_no,
                &part.doccomments,
                Some(def.contract_no),
                ns,
                &mut symtable,
            ) {
                delayed.push(delay);
            }
        }
    }

    delayed
}

pub fn variable_decl<'a>(
    contract: Option<&ContractDefinition>,
    def: &'a pt::VariableDefinition,
    file_no: usize,
    tags: &[DocComment],
    contract_no: Option<usize>,
    ns: &mut Namespace,
    symtable: &mut Symtable,
) -> Option<DelayedResolveInitializer<'a>> {
    let mut attrs = def.attrs.clone();
    let mut ty = def.ty.clone();
    let mut ret = None;

    // For function types, the parser adds the attributes incl visibility to the type,
    // not the pt::VariableDefinition attrs. We need to chomp off the visibility
    // from the attributes before resolving the type
    if let pt::Expression::Type(
        _,
        pt::Type::Function {
            attributes,
            returns,
            ..
        },
    ) = &mut ty
    {
        let mut filter_var_attrs = |attributes: &mut Vec<pt::FunctionAttribute>| {
            if attributes.is_empty() {
                return;
            }

            let mut seen_visibility = false;

            // here we must iterate in reverse order: we can only remove the *last* visibility attribute
            // This is due to the insane syntax
            // contract c {
            //    function() external internal x;
            // }
            // The first external means the function type is external, the second internal means the visibility
            // of the x is internal.
            for attr_no in (0..attributes.len()).rev() {
                if let pt::FunctionAttribute::Immutable(loc) = &attributes[attr_no] {
                    attrs.push(pt::VariableAttribute::Immutable(*loc));
                    attributes.remove(attr_no);
                } else if !seen_visibility {
                    if let pt::FunctionAttribute::Visibility(v) = &attributes[attr_no] {
                        attrs.push(pt::VariableAttribute::Visibility(v.clone()));
                        attributes.remove(attr_no);
                        seen_visibility = true;
                    }
                }
            }
        };

        if let Some((_, trailing_attributes)) = returns {
            filter_var_attrs(trailing_attributes);
        } else {
            filter_var_attrs(attributes);
        }
    }

    let mut diagnostics = Diagnostics::default();

    let ty = match ns.resolve_type(
        file_no,
        contract_no,
        ResolveTypeContext::None,
        &ty,
        &mut diagnostics,
    ) {
        Ok(s) => s,
        Err(()) => {
            ns.diagnostics.extend(diagnostics);
            return None;
        }
    };

    let mut constant = false;
    let mut visibility: Option<pt::Visibility> = None;
    let mut has_immutable: Option<pt::Loc> = None;
    let mut is_override: Option<(pt::Loc, Vec<usize>)> = None;
    let mut storage_type: Option<pt::StorageType> = None;

    for attr in attrs {
        match &attr {
            pt::VariableAttribute::Constant(loc) => {
                if constant {
                    ns.diagnostics.push(Diagnostic::error(
                        *loc,
                        "duplicate constant attribute".to_string(),
                    ));
                }
                constant = true;
            }
            pt::VariableAttribute::Immutable(loc) => {
                if let Some(prev) = &has_immutable {
                    ns.diagnostics.push(Diagnostic::error_with_note(
                        *loc,
                        "duplicate 'immutable' attribute".to_string(),
                        *prev,
                        "previous 'immutable' attribute".to_string(),
                    ));
                }
                has_immutable = Some(*loc);
            }
            pt::VariableAttribute::Override(loc, bases) => {
                if let Some((prev, _)) = &is_override {
                    ns.diagnostics.push(Diagnostic::error_with_note(
                        *loc,
                        "duplicate 'override' attribute".to_string(),
                        *prev,
                        "previous 'override' attribute".to_string(),
                    ));
                }

                let mut list = Vec::new();
                let mut diagnostics = Diagnostics::default();

                if let Some(contract_no) = contract_no {
                    for name in bases {
                        if let Ok(no) =
                            ns.resolve_contract_with_namespace(file_no, name, &mut diagnostics)
                        {
                            if list.contains(&no) {
                                diagnostics.push(Diagnostic::error(
                                    name.loc,
                                    format!("duplicate override '{name}'"),
                                ));
                            } else if !is_base(no, contract_no, ns) {
                                diagnostics.push(Diagnostic::error(
                                    name.loc,
                                    format!(
                                        "override '{}' is not a base contract of '{}'",
                                        name, ns.contracts[contract_no].id
                                    ),
                                ));
                            } else {
                                list.push(no);
                            }
                        }
                    }

                    is_override = Some((*loc, list));
                } else {
                    diagnostics.push(Diagnostic::error(
                        *loc,
                        "global variable has no bases contracts to override".to_string(),
                    ));
                }

                ns.diagnostics.extend(diagnostics);
            }
            pt::VariableAttribute::Visibility(v) if contract_no.is_none() => {
                ns.diagnostics.push(Diagnostic::error(
                    v.loc_opt().unwrap(),
                    format!("'{v}': global variable cannot have visibility specifier"),
                ));
                return None;
            }
            pt::VariableAttribute::Visibility(pt::Visibility::External(loc)) => {
                ns.diagnostics.push(Diagnostic::error(
                    loc.unwrap(),
                    "variable cannot be declared external".to_string(),
                ));
                return None;
            }
            pt::VariableAttribute::Visibility(v) => {
                if let Some(e) = &visibility {
                    ns.diagnostics.push(Diagnostic::error_with_note(
                        v.loc_opt().unwrap(),
                        format!("variable visibility redeclared '{v}'"),
                        e.loc_opt().unwrap(),
                        format!("location of previous declaration of '{e}'"),
                    ));
                    return None;
                }

                visibility = Some(v.clone());
            }
            pt::VariableAttribute::StorageType(s) => {
                if storage_type.is_some() {
                    ns.diagnostics.push(Diagnostic::error(
                        attr.loc(),
                        format!(
                            "mutliple storage type specifiers for '{}'",
                            def.name.as_ref().unwrap().name
                        ),
                    ));
                } else {
                    storage_type = Some(s.clone());
                }
            }
        }
    }

    if ns.target == Target::Soroban {
        if storage_type.is_none() {
            ns.diagnostics.push(Diagnostic::warning(
                def.loc,
                format!(
                    "storage type not specified for `{}`, defaulting to `persistent`",
                    def.name.as_ref().unwrap().name
                ),
            ));
        }
    } else if storage_type.is_some() {
        ns.diagnostics.push(Diagnostic::warning(
            def.loc,
            format!(
                "variable `{}`: storage types are only valid for Soroban targets",
                def.name.as_ref().unwrap().name
            ),
        ));
    }

    if let Some(loc) = &has_immutable {
        if constant {
            ns.diagnostics.push(Diagnostic::error(
                *loc,
                "variable cannot be declared both 'immutable' and 'constant'".to_string(),
            ));
            constant = false;
        }
    }

    let visibility = match visibility {
        Some(v) => v,
        None => pt::Visibility::Internal(Some(def.ty.loc())),
    };

    if let pt::Visibility::Public(_) = &visibility {
        // override allowed
    } else if let Some((loc, _)) = &is_override {
        ns.diagnostics.push(Diagnostic::error(
            *loc,
            "only public variable can be declared 'override'".to_string(),
        ));
        is_override = None;
    }

    if let Some(contract) = contract {
        if matches!(contract.ty, pt::ContractTy::Interface(_))
            || (matches!(contract.ty, pt::ContractTy::Library(_)) && !constant)
        {
            if contract.name.is_none() || def.name.is_none() {
                return None;
            }
            ns.diagnostics.push(Diagnostic::error(
                def.loc,
                format!(
                    "{} '{}' is not allowed to have contract variable '{}'",
                    contract.ty,
                    contract.name.as_ref().unwrap().name,
                    def.name.as_ref().unwrap().name
                ),
            ));
            return None;
        }
    } else {
        if !constant {
            ns.diagnostics.push(Diagnostic::error(
                def.ty.loc(),
                "global variable must be constant".to_string(),
            ));
            return None;
        }
        if ty.contains_internal_function(ns) {
            ns.diagnostics.push(Diagnostic::error(
                def.ty.loc(),
                "global variable cannot be of type internal function".to_string(),
            ));
            return None;
        }
    }

    if ty.contains_internal_function(ns)
        && matches!(
            visibility,
            pt::Visibility::Public(_) | pt::Visibility::External(_)
        )
    {
        ns.diagnostics.push(Diagnostic::error(
            def.ty.loc(),
            format!("variable of type internal function cannot be '{visibility}'"),
        ));
        return None;
    } else if let Some(ty) = ty.contains_builtins(ns, &StructType::AccountInfo) {
        let message = format!("variable cannot be of builtin type '{}'", ty.to_string(ns));
        ns.diagnostics
            .push(Diagnostic::error(def.ty.loc(), message));
        return None;
    } else if let Some(ty) = ty.contains_builtins(ns, &StructType::AccountMeta) {
        let message = format!("variable cannot be of builtin type '{}'", ty.to_string(ns));
        ns.diagnostics
            .push(Diagnostic::error(def.ty.loc(), message));
        return None;
    }

    let mut diagnostics = Diagnostics::default();

    let initializer = if constant {
        if let Some(initializer) = &def.initializer {
            let mut context = ExprContext {
                file_no,
                contract_no,
                constant,
                ..Default::default()
            };
            context.enter_scope();

            match expression(
                initializer,
                &mut context,
                ns,
                symtable,
                &mut diagnostics,
                ResolveTo::Type(&ty),
            ) {
                Ok(res) => {
                    // implicitly conversion to correct ty
                    match res.cast(&def.loc, &ty, true, ns, &mut diagnostics) {
                        Ok(res) => {
                            res.check_constant_overflow(&mut diagnostics);
                            Some(res)
                        }
                        Err(_) => None,
                    }
                }
                Err(()) => None,
            }
        } else {
            diagnostics.push(Diagnostic::decl_error(
                def.loc,
                "missing initializer for constant".to_string(),
            ));

            None
        }
    } else {
        None
    };

    ns.diagnostics.extend(diagnostics);

    let bases = contract_no.map(|contract_no| ns.contract_bases(contract_no));

    let tags = resolve_tags(
        def.name.as_ref().unwrap().loc.file_no(),
        if contract_no.is_none() {
            "global variable"
        } else {
            "state variable"
        },
        tags,
        None,
        None,
        bases,
        ns,
    );

    let sdecl = Variable {
        name: def.name.as_ref().unwrap().name.to_string(),
        loc: def.loc,
        tags,
        visibility: visibility.clone(),
        ty: ty.clone(),
        constant,
        immutable: has_immutable.is_some(),
        assigned: def.initializer.is_some(),
        initializer,
        read: matches!(visibility, pt::Visibility::Public(_)),
        storage_type,
    };

    let var_no = if let Some(contract_no) = contract_no {
        let var_no = ns.contracts[contract_no].variables.len();

        ns.contracts[contract_no].variables.push(sdecl);

        if !constant {
            if let Some(initializer) = &def.initializer {
                ret = Some(DelayedResolveInitializer {
                    var_no,
                    contract_no,
                    initializer,
                });
            }
        }

        var_no
    } else {
        let var_no = ns.constants.len();

        ns.constants.push(sdecl);

        var_no
    };

    let success = ns.add_symbol(
        file_no,
        contract_no,
        def.name.as_ref().unwrap(),
        Symbol::Variable(def.loc, contract_no, var_no),
    );

    // for public variables in contracts, create an accessor function
    if success && matches!(visibility, pt::Visibility::Public(_)) {
        if let Some(contract_no) = contract_no {
            // The accessor function returns the value of the storage variable, constant or not.
            let mut expr = if constant {
                Expression::ConstantVariable {
                    loc: pt::Loc::Implicit,
                    ty: ty.clone(),
                    contract_no: Some(contract_no),
                    var_no,
                }
            } else {
                Expression::StorageVariable {
                    loc: pt::Loc::Implicit,
                    ty: Type::StorageRef(false, Box::new(ty.clone())),
                    contract_no,
                    var_no,
                }
            };

            // If the variable is an array or mapping, the accessor function takes mapping keys
            // or array indices as arguments, and returns the dereferenced value
            let mut symtable = Symtable::default();
            let mut context = ExprContext::default();
            context.enter_scope();
            let mut params = Vec::new();
            let param = collect_parameters(
                &ty,
                &def.name,
                &mut symtable,
                &mut context,
                &mut params,
                &mut expr,
                ns,
            )?;

            if param.ty.contains_mapping(ns) {
                // we can't return a mapping
                ns.diagnostics.push(Diagnostic::decl_error(
                    def.loc,
                    "mapping in a struct variable cannot be public".to_string(),
                ));
            }

            let (body, returns) =
                accessor_body(expr, param, constant, &mut symtable, &mut context, ns);

            let mut func = Function::new(
                def.name.as_ref().unwrap().loc,
                def.name.as_ref().unwrap().loc,
                def.name.as_ref().unwrap().clone(),
                Some(contract_no),
                Vec::new(),
                pt::FunctionTy::Function,
                // accessors for constant variables have view mutability
                Some(pt::Mutability::View(def.name.as_ref().unwrap().loc)),
                pt::Visibility::External(None),
                params,
                returns,
                ns,
            );

            func.body = body;
            func.is_accessor = true;
            func.has_body = true;
            func.is_override = is_override;
            func.symtable = symtable;

            // add the function to the namespace and then to our contract
            let func_no = ns.functions.len();

            ns.functions.push(func);

            ns.contracts[contract_no].functions.push(func_no);

            // we already have a symbol for
            let symbol = Symbol::Function(vec![(def.loc, func_no)]);

            ns.function_symbols.insert(
                (
                    def.loc.file_no(),
                    Some(contract_no),
                    def.name.as_ref().unwrap().name.to_owned(),
                ),
                symbol,
            );
        }
    }

    ret
}

/// For accessor functions, create the parameter list and the return expression
fn collect_parameters(
    ty: &Type,
    name: &Option<pt::Identifier>,
    symtable: &mut Symtable,
    context: &mut ExprContext,
    params: &mut Vec<Parameter<Type>>,
    expr: &mut Expression,
    ns: &mut Namespace,
) -> Option<Parameter<Type>> {
    match ty {
        Type::Mapping(Mapping {
            key,
            key_name,
            value,
            value_name,
        }) => {
            let map = (*expr).clone();

            let id = if let Some(name) = &key_name {
                name.clone()
            } else {
                pt::Identifier {
                    loc: pt::Loc::Implicit,
                    name: String::new(),
                }
            };
            let arg_ty = key.as_ref().clone();

            let arg_no = symtable.add(
                &id,
                arg_ty.clone(),
                ns,
                VariableInitializer::Solidity(None),
                VariableUsage::Parameter,
                None,
                context,
            )?;

            symtable.arguments.push(Some(arg_no));

            *expr = Expression::Subscript {
                loc: pt::Loc::Implicit,
                ty: ty.storage_array_elem(),
                array_ty: Type::StorageRef(false, Box::new(ty.clone())),
                array: Box::new(map),
                index: Box::new(Expression::Variable {
                    loc: pt::Loc::Implicit,
                    ty: arg_ty,
                    var_no: arg_no,
                }),
            };

            params.push(Parameter {
                id: key_name.clone(),
                loc: pt::Loc::Implicit,
                ty: key.as_ref().clone(),
                ty_loc: None,
                indexed: false,
                readonly: false,
                infinite_size: false,
                recursive: false,
                annotation: None,
            });

            collect_parameters(value, value_name, symtable, context, params, expr, ns)
        }
        Type::Array(elem_ty, dims) => {
            let mut ty = Type::StorageRef(false, Box::new(ty.clone()));
            for _ in 0..dims.len() {
                let map = (*expr).clone();

                let id = pt::Identifier {
                    loc: pt::Loc::Implicit,
                    name: "".to_owned(),
                };
                let arg_ty = Type::Uint(256);

                let var_no = symtable
                    .add(
                        &id,
                        arg_ty.clone(),
                        ns,
                        VariableInitializer::Solidity(None),
                        VariableUsage::Parameter,
                        None,
                        context,
                    )
                    .unwrap();

                symtable.arguments.push(Some(var_no));

                *expr = Expression::Subscript {
                    loc: pt::Loc::Implicit,
                    ty: ty.storage_array_elem(),
                    array_ty: ty.clone(),
                    array: Box::new(map),
                    index: Box::new(Expression::Variable {
                        loc: pt::Loc::Implicit,
                        ty: Type::Uint(256),
                        var_no,
                    }),
                };

                ty = ty.storage_array_elem();

                params.push(Parameter {
                    id: Some(id),
                    loc: pt::Loc::Implicit,
                    ty: arg_ty,
                    ty_loc: None,
                    indexed: false,
                    readonly: false,
                    infinite_size: false,
                    recursive: false,
                    annotation: None,
                });
            }

            collect_parameters(elem_ty, &None, symtable, context, params, expr, ns)
        }
        _ => Some(Parameter {
            id: name.clone(),
            loc: if let Some(name) = name {
                name.loc
            } else {
                pt::Loc::Implicit
            },
            ty: ty.clone(),
            ty_loc: None,
            indexed: false,
            readonly: false,
            infinite_size: false,
            recursive: false,
            annotation: None,
        }),
    }
}

/// Build up an ast for the implict accessor function for public state variables.
fn accessor_body(
    expr: Expression,
    param: Parameter<Type>,
    constant: bool,
    symtable: &mut Symtable,
    context: &mut ExprContext,
    ns: &mut Namespace,
) -> (Vec<Statement>, Vec<Parameter<Type>>) {
    // This could be done in codegen rather than sema, however I am not sure how we would implement
    // that. After building up the parameter/returns list for the implicit function, we have done almost
    // all the work already for building the body (see `expr` and `param` above). So, we would need to
    // share some code between codegen and sema for this.

    let mut ty = param.ty.clone();

    // If the return value of an accessor function is a struct, return the members rather than
    // the struct. This is a particular inconsistency in Solidity which does not apply recursively,
    // i.e. if the struct contains structs, then those values are returned as structs.
    if let Type::Struct(str) = &param.ty {
        let expr = Arc::new(expr);

        if !constant {
            ty = Type::StorageRef(false, ty.into());
        }

        let var_no = symtable
            .add(
                &pt::Identifier {
                    loc: pt::Loc::Implicit,
                    name: "_".into(),
                },
                ty.clone(),
                ns,
                VariableInitializer::Solidity(Some(expr.clone())),
                VariableUsage::LocalVariable,
                Some(pt::StorageLocation::Storage(pt::Loc::Implicit)),
                context,
            )
            .unwrap();

        symtable.vars[&var_no].read = true;
        symtable.vars[&var_no].assigned = true;

        let def = str.definition(ns);
        let returns = def.fields.clone();

        let list = if constant {
            def.fields
                .iter()
                .enumerate()
                .map(|(field, param)| Expression::Load {
                    loc: pt::Loc::Implicit,
                    ty: param.ty.clone(),
                    expr: Expression::StructMember {
                        loc: pt::Loc::Implicit,
                        ty: Type::Ref(def.fields[field].ty.clone().into()),
                        expr: Expression::Variable {
                            loc: pt::Loc::Implicit,
                            ty: ty.clone(),
                            var_no,
                        }
                        .into(),
                        field,
                    }
                    .into(),
                })
                .collect()
        } else {
            def.fields
                .iter()
                .enumerate()
                .map(|(field, param)| Expression::StorageLoad {
                    loc: pt::Loc::Implicit,
                    ty: param.ty.clone(),
                    expr: Expression::StructMember {
                        loc: pt::Loc::Implicit,
                        ty: Type::StorageRef(false, def.fields[field].ty.clone().into()),
                        expr: Expression::Variable {
                            loc: pt::Loc::Implicit,
                            ty: ty.clone(),
                            var_no,
                        }
                        .into(),
                        field,
                    }
                    .into(),
                })
                .collect()
        };

        let body = vec![
            Statement::VariableDecl(pt::Loc::Implicit, var_no, param, Some(expr)),
            Statement::Return(
                pt::Loc::Implicit,
                Some(Expression::List {
                    loc: pt::Loc::Implicit,
                    list,
                }),
            ),
        ];

        (body, returns)
    } else {
        let body = vec![Statement::Return(
            pt::Loc::Implicit,
            Some(if constant {
                expr
            } else {
                Expression::StorageLoad {
                    loc: pt::Loc::Implicit,
                    ty,
                    expr: Box::new(expr),
                }
            }),
        )];

        let returns = vec![param];

        (body, returns)
    }
}

pub fn resolve_initializers(
    initializers: &[DelayedResolveInitializer],
    file_no: usize,
    ns: &mut Namespace,
) {
    let mut symtable = Symtable::default();
    let mut diagnostics = Diagnostics::default();

    for DelayedResolveInitializer {
        var_no,
        contract_no,
        initializer,
    } in initializers
    {
        let var = &ns.contracts[*contract_no].variables[*var_no];
        let ty = var.ty.clone();

        let mut context = ExprContext {
            file_no,
            contract_no: Some(*contract_no),
            ..Default::default()
        };
        context.enter_scope();

        if let Ok(res) = expression(
            initializer,
            &mut context,
            ns,
            &mut symtable,
            &mut diagnostics,
            ResolveTo::Type(&ty),
        ) {
            if let Ok(res) = res.cast(&initializer.loc(), &ty, true, ns, &mut diagnostics) {
                res.check_constant_overflow(&mut diagnostics);
                ns.contracts[*contract_no].variables[*var_no].initializer = Some(res);
            }
        }
    }

    ns.diagnostics.extend(diagnostics);
}
