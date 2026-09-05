// SPDX-License-Identifier: Apache-2.0

use self::{
    expression::{constructor::check_circular_reference, strings::unescape},
    functions::{resolve_params, resolve_returns},
    symtable::Symtable,
    unused_variable::check_unused_errors,
    unused_variable::{check_unused_events, check_unused_namespace_variables},
    variables::variable_decl,
};
use crate::file_resolver::{FileResolver, ResolvedFile};
use num_bigint::BigInt;
use solang_parser::{
    doccomment::{parse_doccomments, DocComment},
    parse,
    pt::{self, CodeLocation},
};
use std::{ffi::OsString, str};

mod address;
pub mod ast;
pub mod builtin;
pub mod builtin_structs;
pub(crate) mod contracts;
pub mod diagnostics;
mod dotgraphviz;
pub(crate) mod eval;
pub(crate) mod expression;
mod external_functions;
pub mod file;
mod format;
mod function_annotation;
mod functions;
mod mutability;
mod namespace;
mod pragma;
pub(crate) mod solana_accounts;
mod statements;
pub mod symtable;
pub mod tags;
mod tests;
mod types;
mod unused_variable;
mod using;
mod variables;
pub(crate) mod yul;

pub type ArrayDimension = Option<(pt::Loc, BigInt)>;

// small prime number
pub const SOLANA_BUCKET_SIZE: u64 = 251;
pub const SOLANA_SPARSE_ARRAY_SIZE: u64 = 1024;

pub struct SourceUnit<'a> {
    items: Vec<SourceUnitPart<'a>>,
    contracts: Vec<ContractDefinition<'a>>,
}

pub struct SourceUnitPart<'a> {
    pub doccomments: Vec<DocComment>,
    pub annotations: Vec<&'a pt::Annotation>,
    pub part: &'a pt::SourceUnitPart,
}

pub struct ContractPart<'a> {
    pub doccomments: Vec<DocComment>,
    pub annotations: Vec<&'a pt::Annotation>,
    pub part: &'a pt::ContractPart,
}

pub struct ContractDefinition<'a> {
    pub contract_no: usize,
    pub loc: pt::Loc,
    pub ty: pt::ContractTy,
    pub doccomments: Vec<DocComment>,
    pub annotations: Vec<&'a pt::Annotation>,
    pub name: Option<&'a pt::Identifier>,
    pub base: Vec<pt::Base>,
    pub parts: Vec<ContractPart<'a>>,
}

/// Load a file file from the cache, parse and resolve it. The file must be present in
/// the cache.
pub fn sema(file: &ResolvedFile, resolver: &mut FileResolver, ns: &mut ast::Namespace) {
    sema_file(file, resolver, ns);

    if !ns.diagnostics.any_errors() {
        // Checks for unused variables
        check_unused_namespace_variables(ns);
        check_unused_events(ns);
        check_unused_errors(ns);
    }
}

/// Parse and resolve a file and its imports in a recursive manner.
fn sema_file(file: &ResolvedFile, resolver: &mut FileResolver, ns: &mut ast::Namespace) {
    let file_no = ns.files.len();

    let (source_code, file_cache_no) = resolver.get_file_contents_and_number(&file.full_path);

    ns.files.push(ast::File::new(
        file.full_path.clone(),
        &source_code,
        file_cache_no,
        file.import_no,
    ));

    let (pt, comments) = match parse(&source_code, file_no) {
        Ok(s) => s,
        Err(mut errors) => {
            ns.diagnostics.append(&mut errors);

            return;
        }
    };

    let tree = collect_annotations_doccomments(&pt, &comments, ns);

    // first resolve all the types we can find
    let fields = types::resolve_typenames(&tree, file_no, ns);
    // resolve pragmas and imports
    for item in &tree.items {
        match &item.part {
            pt::SourceUnitPart::PragmaDirective(pragma) => {
                annotions_not_allowed(&item.annotations, "pragma", ns);
                pragma::resolve_pragma(pragma, ns);
            }
            pt::SourceUnitPart::ImportDirective(import) => {
                annotions_not_allowed(&item.annotations, "import", ns);
                resolve_import(import, Some(file), file_no, resolver, ns);
            }
            _ => (),
        }
    }

    contracts::resolve_base_contracts(&tree.contracts, file_no, ns);

    // once all the types are resolved, we can resolve the structs and events. This is because
    // struct fields or event fields can have types defined elsewhere.
    types::resolve_fields(fields, file_no, ns);

    // resolve functions/constants outside of contracts
    let mut resolve_bodies = Vec::new();

    for item in &tree.items {
        match item.part {
            pt::SourceUnitPart::FunctionDefinition(func) => {
                annotions_not_allowed(&item.annotations, "function", ns);

                if let Some(func_no) = functions::function(func, file_no, &item.doccomments, ns) {
                    resolve_bodies.push((func_no, func));
                }
            }
            pt::SourceUnitPart::VariableDefinition(var) => {
                annotions_not_allowed(&item.annotations, "variable", ns);

                variable_decl(
                    None,
                    var,
                    file_no,
                    &item.doccomments,
                    None,
                    ns,
                    &mut Symtable::default(),
                );
            }
            _ => (),
        }
    }

    // Now we can resolve the global using directives
    for item in &tree.items {
        if let pt::SourceUnitPart::Using(using) = item.part {
            annotions_not_allowed(&item.annotations, "using", ns);

            if let Ok(using) = using::using_decl(using, file_no, None, ns) {
                ns.using.push(using);
            }
        }
    }

    // now resolve the contracts
    contracts::resolve(&tree.contracts, file_no, ns);

    // now we can resolve the body of functions outside of contracts
    for (func_no, func) in resolve_bodies {
        let _ = statements::resolve_function_body(func, &[], file_no, None, func_no, ns);
    }

    if !ns.diagnostics.any_errors() {
        for contract_no in 0..ns.contracts.len() {
            external_functions::add_external_functions(contract_no, ns);
            check_circular_reference(contract_no, ns);
        }
    }

    // check for stray semi colons
    for part in &pt.0 {
        match part {
            pt::SourceUnitPart::StraySemicolon(loc) => {
                ns.diagnostics
                    .push(ast::Diagnostic::error(*loc, "stray semicolon".to_string()));
            }
            pt::SourceUnitPart::ContractDefinition(contract) => {
                for part in &contract.parts {
                    if let pt::ContractPart::StraySemicolon(loc) = part {
                        ns.diagnostics
                            .push(ast::Diagnostic::error(*loc, "stray semicolon".to_string()));
                    }
                }
            }
            _ => (),
        }
    }

    // now check state mutability for all contracts
    mutability::mutability(file_no, ns);
}

/// Find import file, resolve it by calling sema and add it to the namespace
fn replace_builtin_loc(symbol: ast::Symbol, import_loc: pt::Loc) -> ast::Symbol {
    match symbol {
        ast::Symbol::Enum(loc, enum_no) => ast::Symbol::Enum(
            if loc == pt::Loc::Builtin {
                import_loc
            } else {
                loc
            },
            enum_no,
        ),
        ast::Symbol::Function(items) => ast::Symbol::Function(
            items
                .into_iter()
                .map(|(loc, function_no)| {
                    if loc == pt::Loc::Builtin {
                        (import_loc, function_no)
                    } else {
                        (loc, function_no)
                    }
                })
                .collect(),
        ),
        ast::Symbol::Variable(loc, contract_no, var_no) => ast::Symbol::Variable(
            if loc == pt::Loc::Builtin {
                import_loc
            } else {
                loc
            },
            contract_no,
            var_no,
        ),
        ast::Symbol::Struct(loc, struct_no) => ast::Symbol::Struct(
            if loc == pt::Loc::Builtin {
                import_loc
            } else {
                loc
            },
            struct_no,
        ),
        ast::Symbol::Event(items) => ast::Symbol::Event(
            items
                .into_iter()
                .map(|(loc, event_no)| {
                    if loc == pt::Loc::Builtin {
                        (import_loc, event_no)
                    } else {
                        (loc, event_no)
                    }
                })
                .collect(),
        ),
        ast::Symbol::Error(loc, error_no) => ast::Symbol::Error(
            if loc == pt::Loc::Builtin {
                import_loc
            } else {
                loc
            },
            error_no,
        ),
        ast::Symbol::Contract(loc, contract_no) => ast::Symbol::Contract(
            if loc == pt::Loc::Builtin {
                import_loc
            } else {
                loc
            },
            contract_no,
        ),
        ast::Symbol::Import(loc, import_no) => ast::Symbol::Import(
            if loc == pt::Loc::Builtin {
                import_loc
            } else {
                loc
            },
            import_no,
        ),
        ast::Symbol::UserType(loc, user_type_no) => ast::Symbol::UserType(
            if loc == pt::Loc::Builtin {
                import_loc
            } else {
                loc
            },
            user_type_no,
        ),
    }
}

fn resolve_import(
    import: &pt::Import,
    parent: Option<&ResolvedFile>,
    file_no: usize,
    resolver: &mut FileResolver,
    ns: &mut ast::Namespace,
) {
    let import_loc = import.loc();

    let path = match import {
        pt::Import::Plain(f, _)
        | pt::Import::GlobalSymbol(f, _, _)
        | pt::Import::Rename(f, _, _) => f,
    };

    let filename = match path {
        pt::ImportPath::Filename(f) => f,
        pt::ImportPath::Path(path) => {
            ns.diagnostics.push(ast::Diagnostic::error(
                path.loc,
                "experimental import paths not supported".into(),
            ));

            return;
        }
    };

    if filename.string.is_empty() {
        ns.diagnostics.push(ast::Diagnostic::error(
            filename.loc,
            "import path empty".into(),
        ));
        return;
    }

    let (valid, bs) = unescape(
        &filename.string,
        filename.loc.start(),
        filename.loc.file_no(),
        &mut ns.diagnostics,
    );

    if !valid {
        return;
    }

    let os_filename = if let Some(res) = osstring_from_vec(&filename.loc, bs, ns) {
        res
    } else {
        return;
    };

    let import_file_no = if let Some(builtin_file_no) = ns
        .files
        .iter()
        .position(|file| file.cache_no.is_none() && file.path == os_filename)
    {
        // import "solana"
        builtin_file_no
    } else {
        match resolver.resolve_file(parent, &os_filename) {
            Err(message) => {
                ns.diagnostics
                    .push(ast::Diagnostic::error(filename.loc, message));

                return;
            }
            Ok(file) => {
                if !ns.files.iter().any(|f| f.path == file.full_path) {
                    sema_file(&file, resolver, ns);

                    // give up if we failed
                    if ns.diagnostics.any_errors() {
                        return;
                    }
                }

                ns.files
                    .iter()
                    .position(|f| f.path == file.full_path)
                    .expect("import should be loaded by now")
            }
        }
    };

    match import {
        pt::Import::Rename(_, renames, _) => {
            for (from, rename_to) in renames {
                if let Some(import) =
                    ns.variable_symbols
                        .get(&(import_file_no, None, from.name.to_owned()))
                {
                    let import = replace_builtin_loc(import.clone(), import_loc);

                    let symbol = rename_to.as_ref().unwrap_or(from);

                    // Only add symbol if it does not already exist with same definition
                    if let Some(existing) =
                        ns.variable_symbols
                            .get(&(file_no, None, symbol.name.clone()))
                    {
                        if existing == &import {
                            continue;
                        }
                    }

                    ns.add_symbol(file_no, None, symbol, import);
                } else if let Some(import) =
                    ns.function_symbols
                        .get(&(import_file_no, None, from.name.to_owned()))
                {
                    let import = replace_builtin_loc(import.clone(), import_loc);

                    let symbol = rename_to.as_ref().unwrap_or(from);

                    // Only add symbol if it does not already exist with same definition
                    if let Some(existing) =
                        ns.function_symbols
                            .get(&(file_no, None, symbol.name.clone()))
                    {
                        if existing == &import {
                            continue;
                        }
                    }

                    ns.add_symbol(file_no, None, symbol, import);
                } else {
                    ns.diagnostics.push(ast::Diagnostic::error(
                        from.loc,
                        format!(
                            "import '{}' does not export '{}'",
                            filename.string, from.name
                        ),
                    ));
                }
            }
        }
        pt::Import::Plain(..) => {
            // find all the exports for the file
            let exports = ns
                .variable_symbols
                .iter()
                .filter_map(|((file_no, contract_no, id), symbol)| {
                    if *file_no == import_file_no {
                        Some((id.clone(), *contract_no, symbol.clone()))
                    } else {
                        None
                    }
                })
                .collect::<Vec<(String, Option<usize>, ast::Symbol)>>();

            for (name, contract_no, symbol) in exports {
                let symbol = replace_builtin_loc(symbol, import_loc);
                let new_symbol = pt::Identifier {
                    name: name.clone(),
                    loc: filename.loc,
                };

                // Only add symbol if it does not already exist with same definition
                if let Some(existing) =
                    ns.variable_symbols
                        .get(&(file_no, contract_no, name.clone()))
                {
                    if existing == &symbol {
                        continue;
                    }
                }

                ns.add_symbol(file_no, contract_no, &new_symbol, symbol);
            }

            let exports = ns
                .function_symbols
                .iter()
                .filter_map(|((file_no, contract_no, id), symbol)| {
                    if *file_no == import_file_no && contract_no.is_none() {
                        Some((id.clone(), symbol.clone()))
                    } else {
                        None
                    }
                })
                .collect::<Vec<(String, ast::Symbol)>>();

            for (name, symbol) in exports {
                let symbol = replace_builtin_loc(symbol, import_loc);
                let new_symbol = pt::Identifier {
                    name: name.clone(),
                    loc: filename.loc,
                };

                // Only add symbol if it does not already exist with same definition
                if let Some(existing) = ns.function_symbols.get(&(file_no, None, name.clone())) {
                    if existing == &symbol {
                        continue;
                    }
                }

                ns.add_symbol(file_no, None, &new_symbol, symbol);
            }
        }
        pt::Import::GlobalSymbol(_, symbol, _) => {
            ns.add_symbol(
                file_no,
                None,
                symbol,
                ast::Symbol::Import(symbol.loc, import_file_no),
            );
        }
    }
}

/// Walk through the parse tree and collect all the annotations and doccomments for
/// each item, also inside contracts.
fn collect_annotations_doccomments<'a>(
    pt: &'a pt::SourceUnit,
    comments: &'a [pt::Comment],
    ns: &mut ast::Namespace,
) -> SourceUnit<'a> {
    let mut items = Vec::new();
    let mut annotations: Vec<&pt::Annotation> = Vec::new();
    let mut contract_no = ns.contracts.len();
    let mut contracts = Vec::new();
    let mut doc_comment_start = 0;

    for part in &pt.0 {
        if let pt::SourceUnitPart::Annotation(anno) = part {
            annotations.push(anno);
            continue;
        }

        let loc = part.loc();

        let doccomments = parse_doccomments(comments, doc_comment_start, loc.start());

        if let pt::SourceUnitPart::ContractDefinition(contract) = part {
            let mut parts = Vec::new();
            let mut parts_annotations: Vec<&pt::Annotation> = Vec::new();
            let mut doc_comment_start = contract.loc.start();

            for part in &contract.parts {
                match part {
                    pt::ContractPart::Annotation(note) => {
                        parts_annotations.push(note);
                        continue;
                    }
                    _ => {
                        let tags =
                            parse_doccomments(comments, doc_comment_start, part.loc().start());

                        parts.push(ContractPart {
                            part,
                            doccomments: tags,
                            annotations: parts_annotations,
                        });
                        parts_annotations = Vec::new();
                    }
                }

                if let pt::ContractPart::FunctionDefinition(f) = &part {
                    if let Some(pt::Statement::Block { loc, .. }) = &f.body {
                        doc_comment_start = loc.end();
                        continue;
                    }
                }

                doc_comment_start = part.loc().end();
            }

            if !parts_annotations.is_empty() {
                for note in parts_annotations {
                    ns.diagnostics.push(ast::Diagnostic::error(
                        note.loc,
                        "annotations should precede 'constructor' or other item".into(),
                    ));
                }
            }

            contracts.push(ContractDefinition {
                contract_no,
                loc: contract.loc,
                ty: contract.ty.clone(),
                annotations,
                doccomments,
                name: contract.name.as_ref(),
                base: contract.base.clone(),
                parts,
            });

            contract_no += 1;
        } else {
            items.push(SourceUnitPart {
                doccomments,
                annotations,
                part,
            });
        }
        annotations = Vec::new();

        if let pt::SourceUnitPart::FunctionDefinition(f) = part {
            if let Some(pt::Statement::Block { loc, .. }) = &f.body {
                doc_comment_start = loc.end();
                continue;
            }
        }

        doc_comment_start = part.loc().end();
    }

    if !annotations.is_empty() {
        for note in annotations {
            ns.diagnostics.push(ast::Diagnostic::error(
                note.loc,
                "annotations should precede 'contract' or other item".into(),
            ));
        }
    }

    SourceUnit { items, contracts }
}

/// If an item does not allow annotations, then generate diagnostic errors for any annotions
fn annotions_not_allowed(annotations: &[&pt::Annotation], item: &str, ns: &mut ast::Namespace) {
    for note in annotations {
        ns.diagnostics.push(ast::Diagnostic::error(
            note.loc,
            format!("annotations not allowed on {item}"),
        ));
    }
}

pub trait Recurse {
    type ArgType;
    /// recurse over a structure
    fn recurse<T>(&self, cx: &mut T, f: fn(expr: &Self::ArgType, ctx: &mut T) -> bool);
}

#[cfg(unix)]
fn osstring_from_vec(_: &pt::Loc, bs: Vec<u8>, _: &mut ast::Namespace) -> Option<OsString> {
    use std::os::unix::ffi::OsStringExt;

    Some(OsString::from_vec(bs))
}

#[cfg(not(unix))]
fn osstring_from_vec(loc: &pt::Loc, bs: Vec<u8>, ns: &mut ast::Namespace) -> Option<OsString> {
    match str::from_utf8(&bs) {
        Ok(s) => Some(OsString::from(s)),
        Err(_) => {
            ns.diagnostics.push(ast::Diagnostic::error(
                *loc,
                "string is not a valid filename".into(),
            ));

            None
        }
    }
}
