// Parity Substrate style ABIs/Abi
use contract_metadata::*;
use num_traits::ToPrimitive;
use parser::pt;
use sema::ast;
use sema::tags::render;
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::convert::TryInto;

#[derive(Deserialize, Serialize)]
pub struct Abi {
    storage: Storage,
    types: Vec<Type>,
    pub spec: Spec,
}

impl Abi {
    pub fn get_function(&self, name: &str) -> Option<&Message> {
        self.spec.messages.iter().find(|m| name == m.name)
    }
}

#[derive(Deserialize, Serialize, PartialEq)]
pub struct ArrayDef {
    array: Array,
}

#[derive(Deserialize, Serialize, PartialEq)]
pub struct Array {
    len: usize,
    #[serde(rename = "type")]
    ty: usize,
}

#[derive(Deserialize, Serialize, PartialEq)]
pub struct SequenceDef {
    sequence: Sequence,
}

#[derive(Deserialize, Serialize, PartialEq)]
pub struct Sequence {
    #[serde(rename = "type")]
    ty: usize,
}

#[derive(Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
enum Type {
    Builtin { def: PrimitiveDef },
    BuiltinArray { def: ArrayDef },
    BuiltinSequence { def: SequenceDef },
    Struct { path: Vec<String>, def: Composite },
    Enum { path: Vec<String>, def: EnumDef },
}

#[derive(Deserialize, Serialize, PartialEq)]
struct BuiltinType {
    id: String,
    def: String,
}

#[derive(Deserialize, Serialize, PartialEq)]
struct EnumVariant {
    name: String,
    discriminant: usize,
}

#[derive(Deserialize, Serialize, PartialEq)]
struct EnumDef {
    variant: Enum,
}

#[derive(Deserialize, Serialize, PartialEq)]
struct Enum {
    variants: Vec<EnumVariant>,
}

#[derive(Deserialize, Serialize, PartialEq)]
struct Composite {
    composite: StructFields,
}

#[derive(Deserialize, Serialize, PartialEq)]
struct StructFields {
    fields: Vec<StructField>,
}

#[derive(Deserialize, Serialize, PartialEq)]
struct PrimitiveDef {
    primitive: String,
}

#[derive(Deserialize, Serialize, PartialEq)]
struct StructField {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(rename = "type")]
    ty: usize,
}

#[derive(Deserialize, Serialize)]
pub struct Constructor {
    pub name: String,
    pub selector: String,
    pub docs: Vec<String>,
    args: Vec<Param>,
}

impl Constructor {
    /// Build byte string from
    pub fn selector(&self) -> Vec<u8> {
        parse_selector(&self.selector)
    }
}

#[derive(Deserialize, Serialize)]
pub struct Message {
    pub name: String,
    pub selector: String,
    pub docs: Vec<String>,
    mutates: bool,
    payable: bool,
    args: Vec<Param>,
    return_type: Option<ParamType>,
}

impl Message {
    /// Build byte string from
    pub fn selector(&self) -> Vec<u8> {
        parse_selector(&self.selector)
    }
}

#[derive(Deserialize, Serialize)]
pub struct Event {
    docs: Vec<String>,
    name: String,
    args: Vec<ParamIndexed>,
}

#[derive(Deserialize, Serialize)]
pub struct Spec {
    pub constructors: Vec<Constructor>,
    pub messages: Vec<Message>,
    pub events: Vec<Event>,
}

#[derive(Deserialize, Serialize)]
struct Param {
    name: String,
    #[serde(rename = "type")]
    ty: ParamType,
}

#[derive(Deserialize, Serialize)]
struct ParamIndexed {
    #[serde(flatten)]
    param: Param,
    indexed: bool,
}

#[derive(Deserialize, Serialize)]
struct ParamType {
    #[serde(rename = "type")]
    ty: usize,
    display_name: Vec<String>,
}

#[derive(Deserialize, Serialize)]
struct Storage {
    #[serde(rename = "struct")]
    structs: StorageStruct,
}

#[derive(Deserialize, Serialize)]
struct StorageStruct {
    fields: Vec<StorageLayout>,
}

#[derive(Deserialize, Serialize)]
struct StorageLayout {
    name: String,
    layout: LayoutField,
}

#[derive(Deserialize, Serialize)]
struct LayoutField {
    cell: LayoutFieldCell,
}

#[derive(Deserialize, Serialize)]
struct LayoutFieldCell {
    key: String,
    ty: usize,
}

/// Create a new registry and create new entries. Note that the registry is
/// accessed by number, and the first entry is 1, not 0.
impl Abi {
    /// Add a type to the list unless already present
    fn register_ty(&mut self, ty: Type) -> usize {
        match self.types.iter().position(|t| *t == ty) {
            Some(i) => i + 1,
            None => {
                self.types.push(ty);

                self.types.len()
            }
        }
    }

    /// Returns index to builtin type in registry. Type is added if not already present
    fn builtin_type(&mut self, ty: &str) -> usize {
        self.register_ty(Type::Builtin {
            def: PrimitiveDef {
                primitive: ty.to_owned(),
            },
        })
    }

    /// Returns index to builtin type in registry. Type is added if not already present
    fn builtin_array_type(&mut self, elem: usize, array_len: usize) -> usize {
        self.register_ty(Type::BuiltinArray {
            def: ArrayDef {
                array: Array {
                    len: array_len,
                    ty: elem,
                },
            },
        })
    }

    /// Returns index to builtin type in registry. Type is added if not already present
    fn builtin_slice_type(&mut self, elem: usize) -> usize {
        self.register_ty(Type::BuiltinSequence {
            def: SequenceDef {
                sequence: Sequence { ty: elem },
            },
        })
    }

    /// Returns index to builtin type in registry. Type is added if not already present
    #[allow(dead_code)]
    fn builtin_enum_type(&mut self, e: &ast::EnumDecl) -> usize {
        self.register_ty(Type::Enum {
            path: vec![e.name.to_owned()],
            def: EnumDef {
                variant: Enum {
                    variants: e
                        .values
                        .iter()
                        .map(|(key, val)| EnumVariant {
                            name: key.to_owned(),
                            discriminant: val.1,
                        })
                        .collect(),
                },
            },
        })
    }

    /// Adds struct type to registry. Does not check for duplication (yet)
    fn struct_type(&mut self, path: Vec<String>, fields: Vec<StructField>) -> usize {
        self.register_ty(Type::Struct {
            path,
            def: Composite {
                composite: StructFields { fields },
            },
        })
    }
}

pub fn load(bs: &str) -> Result<Abi, serde_json::error::Error> {
    serde_json::from_str(bs)
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

/// Generate the metadata for Substrate 2.0
pub fn metadata(contract_no: usize, code: &[u8], ns: &ast::Namespace) -> Value {
    let hash = blake2_rfc::blake2b::blake2b(32, &[], &code);
    let version = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
    let language = SourceLanguage::new(Language::Solidity, version.clone());
    let compiler = SourceCompiler::new(Compiler::Solang, version);
    let source = Source::new(hash.as_bytes().try_into().unwrap(), language, compiler);
    let mut builder = Contract::builder();

    // Add our name and tags
    builder.name(ns.contracts[contract_no].name.to_string());

    let mut description = tags(contract_no, "title", ns);

    description.extend(tags(contract_no, "notice", ns));

    if !description.is_empty() {
        builder.description(description.join("\n"));
    };

    let authors = tags(contract_no, "author", ns);

    if !authors.is_empty() {
        builder.authors(authors);
    } else {
        builder.authors(vec!["unknown"]);
    }

    // FIXME: contract-metadata wants us to provide a version number, but there is no version in the solidity source
    // code. Since we must provide a valid semver version, we just provide a bogus value.Abi
    builder.version(Version::new(0, 0, 1));

    let contract = builder.build().unwrap();

    // generate the abi for our contract
    let abi = gen_abi(contract_no, ns);

    let mut abi_json: Map<String, Value> = Map::new();
    abi_json.insert(
        String::from("types"),
        serde_json::to_value(&abi.types).unwrap(),
    );
    abi_json.insert(
        String::from("spec"),
        serde_json::to_value(&abi.spec).unwrap(),
    );
    abi_json.insert(
        String::from("storage"),
        serde_json::to_value(&abi.storage).unwrap(),
    );

    let metadata = ContractMetadata::new(source, contract, None, abi_json);

    // serialize to json
    serde_json::to_value(&metadata).unwrap()
}

fn gen_abi(contract_no: usize, ns: &ast::Namespace) -> Abi {
    let mut abi = Abi {
        types: Vec::new(),
        storage: Storage {
            structs: StorageStruct { fields: Vec::new() },
        },
        spec: Spec {
            constructors: Vec::new(),
            messages: Vec::new(),
            events: Vec::new(),
        },
    };

    let fields = ns.contracts[contract_no]
        .layout
        .iter()
        .filter_map(|layout| {
            let var = &ns.contracts[layout.contract_no].variables[layout.var_no];

            if !var.ty.is_mapping() {
                Some(StorageLayout {
                    name: var.name.to_string(),
                    layout: LayoutField {
                        cell: LayoutFieldCell {
                            key: format!("0x{:064X}", layout.slot),
                            ty: ty_to_abi(&var.ty, ns, &mut abi).ty,
                        },
                    },
                })
            } else {
                None
            }
        })
        .collect();

    abi.storage.structs.fields = fields;

    let mut constructors = ns.contracts[contract_no]
        .functions
        .iter()
        .filter(|f| f.is_constructor())
        .map(|f| Constructor {
            name: String::from("new"),
            selector: render_selector(f),
            args: f
                .params
                .iter()
                .map(|p| parameter_to_abi(p, ns, &mut abi))
                .collect(),
            docs: vec![render(&f.tags)],
        })
        .collect::<Vec<Constructor>>();

    if let Some((f, _)) = &ns.contracts[contract_no].default_constructor {
        constructors.push(Constructor {
            name: String::from("new"),
            selector: render_selector(f),
            args: f
                .params
                .iter()
                .map(|p| parameter_to_abi(p, ns, &mut abi))
                .collect(),
            docs: vec![render(&f.tags)],
        });
    }

    let messages = ns.contracts[contract_no]
        .all_functions
        .keys()
        .filter_map(|(base_contract_no, function_no)| {
            if ns.contracts[*base_contract_no].is_library() {
                None
            } else {
                Some(&ns.contracts[*base_contract_no].functions[*function_no])
            }
        })
        .filter(|f| match f.visibility {
            pt::Visibility::Public(_) | pt::Visibility::External(_) => {
                f.ty == pt::FunctionTy::Function
            }
            _ => false,
        })
        .map(|f| {
            let payable = matches!(f.mutability, Some(pt::StateMutability::Payable(_)));

            Message {
                name: f.name.to_owned(),
                mutates: f.mutability.is_none() || payable,
                payable,
                return_type: match f.returns.len() {
                    0 => None,
                    1 => Some(ty_to_abi(&f.returns[0].ty, ns, &mut abi)),
                    _ => {
                        let fields = f
                            .returns
                            .iter()
                            .map(|f| StructField {
                                name: if f.name.is_empty() {
                                    None
                                } else {
                                    Some(f.name.to_string())
                                },
                                ty: ty_to_abi(&f.ty, ns, &mut abi).ty,
                            })
                            .collect();

                        Some(ParamType {
                            ty: abi.struct_type(Vec::new(), fields),
                            display_name: vec![],
                        })
                    }
                },
                selector: render_selector(f),
                args: f
                    .params
                    .iter()
                    .map(|p| parameter_to_abi(p, ns, &mut abi))
                    .collect(),
                docs: vec![render(&f.tags)],
            }
        })
        .collect();

    let events = ns.contracts[contract_no]
        .sends_events
        .iter()
        .map(|event_no| {
            let event = &ns.events[*event_no];

            let name = event.name.to_owned();
            let args = event
                .fields
                .iter()
                .map(|p| ParamIndexed {
                    param: parameter_to_abi(p, ns, &mut abi),
                    indexed: p.indexed,
                })
                .collect();
            let docs = vec![render(&event.tags)];

            Event { name, args, docs }
        })
        .collect();

    abi.spec = Spec {
        constructors,
        messages,
        events,
    };

    abi
}

fn ty_to_abi(ty: &ast::Type, ns: &ast::Namespace, registry: &mut Abi) -> ParamType {
    match ty {
        ast::Type::Enum(n) => {
            /* clike_enums are broken in polkadot. Use u8 for now.
            ty: registry.builtin_enum_type(&ns.enums[*n]),
            display_name: vec![ns.enums[*n].name.to_owned()],
            */
            let mut display_name = vec![ns.enums[*n].name.to_owned()];

            if let Some(contract_name) = &ns.enums[*n].contract {
                display_name.insert(0, contract_name.to_owned());
            }

            ParamType {
                ty: registry.builtin_type("u8"),
                display_name,
            }
        }
        ast::Type::Bytes(n) => {
            let elem = registry.builtin_type("u8");
            ParamType {
                ty: registry.builtin_array_type(elem, *n as usize),
                display_name: vec![],
            }
        }
        ast::Type::Mapping(_, _) => unreachable!(),
        ast::Type::Array(ty, dims) => {
            let mut param_ty = ty_to_abi(ty, ns, registry);

            for d in dims {
                if let Some(d) = d {
                    param_ty = ParamType {
                        ty: registry.builtin_array_type(param_ty.ty, d.to_usize().unwrap()),
                        display_name: vec![],
                    }
                } else {
                    param_ty = ParamType {
                        ty: registry.builtin_slice_type(param_ty.ty),
                        display_name: vec![],
                    }
                }
            }

            param_ty
        }
        ast::Type::StorageRef(ty) => ty_to_abi(ty, ns, registry),
        ast::Type::Ref(ty) => ty_to_abi(ty, ns, registry),
        ast::Type::Bool | ast::Type::Uint(_) | ast::Type::Int(_) => {
            let scalety = match ty {
                ast::Type::Bool => "bool".into(),
                ast::Type::Uint(n) => format!("u{}", n),
                ast::Type::Int(n) => format!("i{}", n),
                _ => unreachable!(),
            };

            ParamType {
                ty: registry.builtin_type(&scalety),
                display_name: vec![scalety.to_string()],
            }
        }
        ast::Type::Address(_) | ast::Type::Contract(_) => {
            let elem = registry.builtin_type("u8");
            let ty = registry.builtin_array_type(elem, 32);

            ParamType {
                ty: registry.struct_type(
                    vec!["AccountId".to_owned()],
                    vec![StructField { name: None, ty }],
                ),
                display_name: vec!["AccountId".to_owned()],
            }
        }
        ast::Type::Struct(n) => {
            let mut display_name = vec![ns.structs[*n].name.to_owned()];

            if let Some(contract_name) = &ns.structs[*n].contract {
                display_name.insert(0, contract_name.to_owned());
            }

            let def = &ns.structs[*n];
            let fields = def
                .fields
                .iter()
                .map(|f| StructField {
                    name: Some(f.name.to_string()),
                    ty: ty_to_abi(&f.ty, ns, registry).ty,
                })
                .collect();

            ParamType {
                ty: registry.struct_type(display_name.clone(), fields),
                display_name,
            }
        }
        ast::Type::DynamicBytes => {
            let elem = registry.builtin_type("u8");

            ParamType {
                ty: registry.builtin_slice_type(elem),
                display_name: vec![String::from("Vec")],
            }
        }
        ast::Type::String => ParamType {
            ty: registry.builtin_type("str"),
            display_name: vec![String::from("String")],
        },
        ast::Type::InternalFunction { .. } => ParamType {
            ty: registry.builtin_type("u32"),
            display_name: vec![String::from("FunctionSelector")],
        },
        ast::Type::ExternalFunction { .. } => {
            let fields = vec![
                StructField {
                    name: None,
                    ty: ty_to_abi(&ast::Type::Address(false), ns, registry).ty,
                },
                StructField {
                    name: None,
                    ty: ty_to_abi(&ast::Type::Uint(32), ns, registry).ty,
                },
            ];

            let display_name = vec![String::from("ExternalFunction")];

            ParamType {
                ty: registry.struct_type(display_name.clone(), fields),
                display_name,
            }
        }
        _ => unreachable!(),
    }
}

fn parameter_to_abi(param: &ast::Parameter, ns: &ast::Namespace, registry: &mut Abi) -> Param {
    Param {
        name: param.name.to_string(),
        ty: ty_to_abi(&param.ty, ns, registry),
    }
}

/// Given an u32 selector, generate a byte string like: 0xF81E7E1A
fn render_selector(f: &ast::Function) -> String {
    format!("0x{}", hex::encode(f.selector().to_le_bytes()))
}

/// Given a selector like "0xF81E7E1A", parse the bytes. This function
/// does not validate the input.
fn parse_selector(selector: &str) -> Vec<u8> {
    hex::decode(&selector[2..]).unwrap()
}
