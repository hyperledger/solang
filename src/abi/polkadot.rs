// SPDX-License-Identifier: Apache-2.0
use contract_metadata::{
    CodeHash, Compiler, Contract, ContractMetadata, Language, Source, SourceCompiler,
    SourceLanguage, SourceWasm,
};
use ink_metadata::{
    layout::{FieldLayout, Layout, LayoutKey, LeafLayout, RootLayout, StructLayout},
    ConstructorSpec, ContractSpec, EnvironmentSpec, EventParamSpec, EventSpec, InkProject,
    MessageParamSpec, MessageSpec, ReturnTypeSpec, TypeSpec,
};

use serde_json::Value;

use num_bigint::BigInt;
use num_traits::ToPrimitive;
use scale_info::{
    form::PortableForm, Field, Path, PortableRegistryBuilder, Type, TypeDef, TypeDefArray,
    TypeDefComposite, TypeDefPrimitive, TypeDefSequence, TypeDefTuple, TypeDefVariant, Variant,
};
use semver::Version;
use solang_parser::pt;

use crate::{
    codegen::revert::{SolidityError, ERROR_SELECTOR, PANIC_SELECTOR},
    emit::polkadot::SCRATCH_SIZE,
    sema::{
        ast::{self, ArrayLength, EventDecl, Function},
        tags::render,
    },
};

macro_rules! path {
    ($( $segments:expr ),*) => {
        Path::from_segments_unchecked([$($segments),*].iter().map(ToString::to_string))
    }
}

fn primitive_to_ty(ty: &ast::Type, registry: &mut PortableRegistryBuilder) -> u32 {
    match ty {
        ast::Type::Int(_) | ast::Type::Uint(_) => int_to_ty(ty, registry),
        ast::Type::Bool => registry.register_type(Type::new(
            path!("bool"),
            vec![],
            TypeDef::Primitive(TypeDefPrimitive::Bool),
            Default::default(),
        )),
        ast::Type::String => registry.register_type(Type::new(
            path!("string"),
            vec![],
            TypeDef::Primitive(TypeDefPrimitive::Str),
            Default::default(),
        )),
        _ => unreachable!("non primitive types"),
    }
}

fn int_to_ty(ty: &ast::Type, registry: &mut PortableRegistryBuilder) -> u32 {
    let (signed, scalety) = match ty {
        ast::Type::Uint(n) => ("uint", n.next_power_of_two()),
        ast::Type::Int(n) => ("int", n.next_power_of_two()),
        _ => unreachable!(),
    };
    let def = match (signed, scalety) {
        ("uint", n) => match n {
            8 => TypeDefPrimitive::U8,
            16 => TypeDefPrimitive::U16,
            32 => TypeDefPrimitive::U32,
            64 => TypeDefPrimitive::U64,
            128 => TypeDefPrimitive::U128,
            256 => TypeDefPrimitive::U256,
            _ => unreachable!(),
        },
        ("int", n) => match n {
            8 => TypeDefPrimitive::I8,
            16 => TypeDefPrimitive::I16,
            32 => TypeDefPrimitive::I32,
            64 => TypeDefPrimitive::I64,
            128 => TypeDefPrimitive::I128,
            256 => TypeDefPrimitive::I256,
            _ => unreachable!(),
        },
        _ => unreachable!(),
    };
    let path = path!(format!("{signed}{scalety}"));
    let ty = Type::new(path, vec![], TypeDef::Primitive(def), Default::default());
    registry.register_type(ty)
}

/// Build the `lang_error` type of this contract, where `errors` is a list
/// containing each error's name, selector and types. Returns a `TypeSpec`
/// of `TypeDefVariant` with each error as a variant.
fn lang_error(
    ns: &ast::Namespace,
    reg: &mut PortableRegistryBuilder,
    errors: Vec<(String, [u8; 4], Vec<ast::Type>)>,
) -> TypeSpec<PortableForm> {
    let variants = errors.iter().enumerate().map(|(n, (name, selector, ty))| {
        let struct_fields = ty
            .iter()
            .map(|ty| resolve_ast(ty, ns, reg).into())
            .map(|field| Field::new(None, field, None, Default::default()))
            .collect::<Vec<_>>();
        let path = path!(format!("0x{}", hex::encode(selector)));
        let type_def = TypeDef::Composite(TypeDefComposite::new(struct_fields));
        let ty = Type::new(path, vec![], type_def, Default::default());
        Variant {
            name: name.to_string(),
            fields: vec![Field::new(None, reg.register_type(ty).into(), None, vec![])],
            index: n.try_into().expect("we do not allow custome error types"),
            docs: Default::default(),
        }
    });
    let path = path!("SolidityError");
    let type_def = TypeDefVariant::new(variants);
    let id = reg.register_type(Type::new(path.clone(), vec![], type_def, vec![]));
    TypeSpec::new(id.into(), path)
}

/// Given an `ast::Type`, find and register the `scale_info::Type` definition in the registry
fn resolve_ast(ty: &ast::Type, ns: &ast::Namespace, registry: &mut PortableRegistryBuilder) -> u32 {
    match ty {
        //  should reflect address_length for different Parachain runtime configurations
        ast::Type::Address(_) | ast::Type::Contract(_) => {
            // substituted to [u8; address_length]
            let address_ty = resolve_ast(
                &ast::Type::Array(
                    Box::new(ast::Type::Uint(8)),
                    vec![ArrayLength::Fixed(BigInt::from(ns.address_length))],
                ),
                ns,
                registry,
            );
            // substituted to struct { AccountId }
            let field = Field::new(None, address_ty.into(), None, vec![]);
            let c = TypeDefComposite::new(vec![field]);
            let path = path!("ink_primitives", "types", "AccountId");
            let ty: Type<PortableForm> =
                Type::new(path, vec![], TypeDef::Composite(c), Default::default());
            registry.register_type(ty)
        }
        // primitive types
        ast::Type::Bool | ast::Type::Int(_) | ast::Type::Uint(_) | ast::Type::String => {
            primitive_to_ty(ty, registry)
        }
        // resolve from the deepest element to outside
        // [[A; a: usize]; b: usize] -> Array(A_id, vec![a, b])
        ast::Type::Array(ty, dims) => {
            let mut ty = resolve_ast(ty, ns, registry);
            for d in dims {
                if let ast::ArrayLength::Fixed(d) = d {
                    let def = TypeDefArray::new(d.to_u32().unwrap(), ty.into());

                    // resolve current depth
                    ty = registry.register_type(Type::new(
                        Default::default(),
                        vec![],
                        TypeDef::Array(def),
                        Default::default(),
                    ));
                } else {
                    let def = TypeDefSequence::new(ty.into());

                    // resolve current depth
                    ty = registry.register_type(Type::new(
                        Default::default(),
                        vec![],
                        TypeDef::Sequence(def),
                        Default::default(),
                    ));
                }
            }
            ty
        }
        // substituted to [u8; len]
        ast::Type::Bytes(n) => resolve_ast(
            &ast::Type::Array(
                Box::new(ast::Type::Uint(8)),
                vec![ArrayLength::Fixed(BigInt::from(*n as i8))],
            ),
            ns,
            registry,
        ),
        // substituted to Vec<u8>
        ast::Type::DynamicBytes => resolve_ast(
            &ast::Type::Array(Box::new(ast::Type::Uint(8)), vec![ArrayLength::Dynamic]),
            ns,
            registry,
        ),
        ast::Type::Struct(s) => {
            let def = s.definition(ns);
            let fields = def
                .fields
                .iter()
                .map(|f| {
                    let f_ty = resolve_ast(&f.ty, ns, registry);

                    Field::new(Some(f.name_as_str().to_string()), f_ty.into(), None, vec![])
                })
                .collect::<Vec<Field<PortableForm>>>();
            let c = TypeDefComposite::new(fields);
            let path = path!(&def.id);
            let ty = Type::new(path, vec![], TypeDef::Composite(c), Default::default());
            registry.register_type(ty)
        }
        ast::Type::Enum(n) => {
            let decl = &ns.enums[*n];
            let variants = decl
                .values
                .iter()
                .enumerate()
                .map(|(idx, (k, _))| Variant {
                    name: k.clone(),
                    fields: Default::default(),
                    index: idx as u8,
                    docs: Default::default(),
                })
                .collect::<Vec<_>>();
            let variant = TypeDef::Variant(TypeDefVariant::new(variants));
            let path = path!(&decl.id);
            let ty = Type::new(path, vec![], variant, Default::default());
            registry.register_type(ty)
        }
        ast::Type::Ref(ty) => resolve_ast(ty, ns, registry),
        ast::Type::StorageRef(_, ty) => resolve_ast(ty, ns, registry),
        ast::Type::InternalFunction { .. } => resolve_ast(&ast::Type::Uint(8), ns, registry),
        ast::Type::ExternalFunction { .. } => {
            let fields = [ast::Type::Address(false), ast::Type::Uint(32)]
                .into_iter()
                .map(|ty| {
                    let ty = resolve_ast(&ty, ns, registry);

                    Field::new(
                        Default::default(),
                        ty.into(),
                        Default::default(),
                        Default::default(),
                    )
                })
                .collect::<Vec<_>>();
            let composite = TypeDef::Composite(TypeDefComposite::new(fields));
            let path = path!("ExternalFunction");
            let ty = Type::new(path, vec![], composite, Default::default());
            registry.register_type(ty)
        }
        ast::Type::UserType(no) => {
            let decl = &ns.user_types[*no];
            let resolved = resolve_ast(&decl.ty, ns, registry);
            match (decl.name.as_ref(), decl.loc) {
                // Builtin Hash type from ink primitives
                ("Hash", pt::Loc::Builtin) => {
                    let field = Field::new(None, resolved.into(), None, vec![]);
                    let composite = TypeDef::Composite(TypeDefComposite::new([field]));
                    let path = path!("ink_primitives", "types", "Hash");
                    registry.register_type(Type::new(path, vec![], composite, vec![]))
                }
                _ => resolved,
            }
        }
        ast::Type::Mapping(ast::Mapping { key, value, .. }) => {
            resolve_ast(key, ns, registry);
            resolve_ast(value, ns, registry)
        }
        _ => unreachable!(),
    }
}

/// Recursively build the storage layout after all types are registered
fn type_to_storage_layout(
    key: u32,
    root: LayoutKey,
    registry: &PortableRegistryBuilder,
) -> Layout<PortableForm> {
    let ty = registry.get(key).unwrap();
    match &ty.type_def {
        TypeDef::Composite(inner) => Layout::Struct(StructLayout::new(
            ty.path.ident().unwrap_or_default(),
            inner.fields.iter().map(|field| {
                FieldLayout::new(
                    field.name.clone().unwrap_or_default(),
                    type_to_storage_layout(field.ty.id, root, registry),
                )
            }),
        )),
        _ => Layout::Leaf(LeafLayout::new(root, key.into())),
    }
}

/// Generate `InkProject` from `ast::Type` and `ast::Namespace`
pub fn gen_project(contract_no: usize, ns: &ast::Namespace) -> InkProject {
    let mut registry = PortableRegistryBuilder::new();

    // This is only used by off-chain tooling. At the moment there is no such tooling available yet.
    // So it is not exactly clear yet what this should look like.
    // For now it just contains all root layouts (you get all storage keys in use).
    let fields: Vec<FieldLayout<PortableForm>> = ns.contracts[contract_no]
        .layout
        .iter()
        .filter_map(|layout| {
            let var = &ns.contracts[layout.contract_no].variables[layout.var_no];
            if let Some(slot) = layout.slot.to_u32() {
                let ty = resolve_ast(&layout.ty, ns, &mut registry);
                let layout_key = LayoutKey::new(slot);
                let root = RootLayout::new(
                    layout_key,
                    type_to_storage_layout(ty, layout_key, &registry),
                    ty.into(),
                );
                Some(FieldLayout::new(var.name.clone(), root))
            } else {
                None
            }
        })
        .collect();
    let contract_name = ns.contracts[contract_no].id.name.clone();
    let storage = Layout::Struct(StructLayout::new(contract_name, fields));

    let constructor_spec = |f: &Function| -> ConstructorSpec<PortableForm> {
        let payable = matches!(f.mutability, ast::Mutability::Payable(_));
        let args = f
            .params
            .iter()
            .map(|p| {
                let ty = resolve_ast(&p.ty, ns, &mut registry);

                let path = registry.get(ty).unwrap().path.clone();
                let spec = TypeSpec::new(ty.into(), path);

                MessageParamSpec::new(p.name_as_str().to_string())
                    .of_type(spec)
                    .done()
            })
            .collect::<Vec<MessageParamSpec<PortableForm>>>();

        ConstructorSpec::from_label(
            if f.id.name.is_empty() {
                "new"
            } else {
                &f.id.name
            }
            .into(),
        )
        .selector(f.selector(ns, &contract_no).try_into().unwrap())
        .payable(payable)
        .args(args)
        .docs(vec![render(&f.tags).as_str()])
        .returns(ReturnTypeSpec::new(TypeSpec::default()))
        .done()
    };

    let constructors = ns.contracts[contract_no]
        .functions
        .iter()
        .filter_map(|i| {
            let f = &ns.functions[*i];
            if f.is_constructor() && ns.function_externally_callable(contract_no, Some(*i)) {
                return Some(f);
            }
            None
        })
        .chain(
            // include default constructor if exists
            ns.contracts[contract_no]
                .default_constructor
                .as_ref()
                .map(|(e, _)| e),
        )
        .map(constructor_spec)
        .collect::<Vec<ConstructorSpec<PortableForm>>>();

    let message_spec = |f: &Function| -> MessageSpec<PortableForm> {
        let payable = matches!(f.mutability, ast::Mutability::Payable(_));
        let mutates = matches!(
            f.mutability,
            ast::Mutability::Payable(_) | ast::Mutability::Nonpayable(_)
        );
        let ret_spec: Option<TypeSpec<PortableForm>> = match f.returns.len() {
            0 => None,
            1 => {
                let ty = resolve_ast(&f.returns[0].ty, ns, &mut registry);
                let path = registry.get(ty).unwrap().path.clone();
                Some(TypeSpec::new(ty.into(), path))
            }
            _ => {
                let fields = f
                    .returns
                    .iter()
                    .map(|r_p| {
                        let ty = resolve_ast(&r_p.ty, ns, &mut registry);

                        ty.into()
                    })
                    .collect::<Vec<_>>();

                let t = TypeDefTuple::new_portable(fields);

                let path = path!(
                    &ns.contracts[contract_no].id.name,
                    &f.id.name,
                    "return_type"
                );

                let ty = registry.register_type(Type::new(
                    path,
                    vec![],
                    TypeDef::Tuple(t),
                    Default::default(),
                ));
                let path = registry.get(ty).unwrap().path.clone();
                Some(TypeSpec::new(ty.into(), path))
            }
        };
        let ret_type = ReturnTypeSpec::new(ret_spec.unwrap_or_default());
        let args = f
            .params
            .iter()
            .map(|p| {
                let ty = resolve_ast(&p.ty, ns, &mut registry);
                let path = registry.get(ty).unwrap().path.clone();
                let spec = TypeSpec::new(ty.into(), path);

                MessageParamSpec::new(p.name_as_str().to_string())
                    .of_type(spec)
                    .done()
            })
            .collect::<Vec<MessageParamSpec<PortableForm>>>();
        let label = if f.mangled_name_contracts.contains(&contract_no) {
            &f.mangled_name
        } else {
            &f.id.name
        };
        MessageSpec::from_label(label.into())
            .selector(f.selector(ns, &contract_no).try_into().unwrap())
            .mutates(mutates)
            .payable(payable)
            .args(args)
            .returns(ret_type)
            .docs(vec![render(&f.tags)])
            .done()
    };

    let messages = ns.contracts[contract_no]
        .all_functions
        .keys()
        .filter(|i| ns.function_externally_callable(contract_no, Some(**i)))
        .map(|i| &ns.functions[*i])
        .filter(|f| !f.is_constructor())
        .map(message_spec)
        .collect::<Vec<MessageSpec<PortableForm>>>();

    let mut event_spec = |e: &EventDecl| -> EventSpec<PortableForm> {
        let args = e
            .fields
            .iter()
            .map(|p| {
                let ty = resolve_ast(&p.ty, ns, &mut registry);
                let path = registry.get(ty).unwrap().path.clone();
                let spec = TypeSpec::new(ty.into(), path);
                EventParamSpec::new(p.name_as_str().into())
                    .of_type(spec)
                    .indexed(p.indexed)
                    .docs(vec![])
                    .done()
            })
            .collect::<Vec<_>>();
        EventSpec::new(e.id.name.clone())
            .args(args)
            .docs(vec![render(&e.tags).as_str()])
            .done()
    };

    let events = ns.contracts[contract_no]
        .emits_events
        .iter()
        .map(|event_no| {
            let event = &ns.events[*event_no];
            event_spec(event)
        })
        .collect::<Vec<EventSpec<PortableForm>>>();

    let environment: EnvironmentSpec<PortableForm> = EnvironmentSpec::new()
        .chain_extension(Default::default()) // Does not exist in Solidity
        .max_event_topics(4)
        .account_id(TypeSpec::new(
            resolve_ast(&ast::Type::Address(false), ns, &mut registry).into(),
            path!("AccountId"),
        ))
        .balance(TypeSpec::new(
            primitive_to_ty(&ast::Type::Uint(128), &mut registry).into(),
            path!("Balance"),
        ))
        .block_number(TypeSpec::new(
            primitive_to_ty(&ast::Type::Uint(64), &mut registry).into(),
            path!("BlockNumber"),
        ))
        .hash(TypeSpec::new(
            resolve_ast(
                &ast::Type::UserType(
                    ns.user_types
                        .iter()
                        .enumerate()
                        .find(|(_, t)| t.name.as_str() == "Hash")
                        .expect("this is a compiler builtin; qed")
                        .0,
                ),
                ns,
                &mut registry,
            )
            .into(),
            path!("Hash"),
        ))
        .timestamp(TypeSpec::new(
            primitive_to_ty(&ast::Type::Uint(64), &mut registry).into(),
            path!("Timestamp"),
        ))
        .static_buffer_size(SCRATCH_SIZE as usize)
        .done();

    let mut error_definitions = vec![
        ("Error".into(), ERROR_SELECTOR, vec![ast::Type::String]),
        ("Panic".into(), PANIC_SELECTOR, vec![ast::Type::Uint(256)]),
    ];
    for (error_no, err) in ns.errors.iter().enumerate() {
        let name = err.name.clone();
        let exprs = Vec::new();
        let selector = SolidityError::Custom { error_no, exprs }.selector(ns);
        let types = err.fields.iter().map(|f| f.ty.clone()).collect();
        error_definitions.push((name, selector, types));
    }

    let spec = ContractSpec::new()
        .constructors(constructors)
        .messages(messages)
        .events(events)
        .docs(vec![render(&ns.contracts[contract_no].tags)])
        .environment(environment)
        .lang_error(lang_error(ns, &mut registry, error_definitions))
        .done();

    InkProject::new_portable(storage, spec, registry.finish())
}

fn tags(contract_no: usize, tagname: &str, ns: &ast::Namespace) -> Vec<String> {
    ns.contracts[contract_no]
        .tags
        .iter()
        .filter_map(|tag| {
            if tag.tag == tagname {
                Some(tag.value.to_owned())
            } else {
                None
            }
        })
        .collect()
}

/// Generate the metadata for Substrate 4.0
pub fn metadata(
    contract_no: usize,
    code: &[u8],
    ns: &ast::Namespace,
    default_authors: &[String],
    contract_version: &str,
) -> Value {
    let hash = blake2_rfc::blake2b::blake2b(32, &[], code);
    let version = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
    let language = SourceLanguage::new(Language::Solidity, version.clone());
    let compiler = SourceCompiler::new(Compiler::Solang, version);
    let code_hash: [u8; 32] = hash.as_bytes().try_into().unwrap();
    let source_wasm = SourceWasm::new(code.to_vec());
    let source = Source::new(
        Some(source_wasm),
        CodeHash(code_hash),
        language,
        compiler,
        None,
    );

    let mut builder = Contract::builder();
    builder.name(&ns.contracts[contract_no].id.name);
    let mut description = tags(contract_no, "title", ns);
    description.extend(tags(contract_no, "notice", ns));
    if !description.is_empty() {
        builder.description(description.join("\n"));
    };
    let authors = tags(contract_no, "author", ns);
    if !authors.is_empty() {
        builder.authors(authors);
    } else {
        builder.authors(default_authors);
    }
    builder.version(Version::parse(contract_version).unwrap());
    let contract = builder.build().unwrap();

    let project_json = serde_json::to_value(gen_project(contract_no, ns)).unwrap();
    let abi = serde_json::from_value(project_json).unwrap();

    serde_json::to_value(ContractMetadata::new(source, contract, None, abi)).unwrap()
}
