// SPDX-License-Identifier: Apache-2.0

use super::symtable::Symtable;
use crate::abi::anchor::function_discriminator;
use crate::codegen::cfg::{ControlFlowGraph, Instr};
use crate::diagnostics::Diagnostics;
use crate::sema::ast::ExternalCallAccounts::{AbsentArgument, NoAccount};
use crate::sema::yul::ast::{InlineAssembly, YulFunction};
use crate::sema::Recurse;
use crate::{codegen, Target};
use indexmap::IndexMap;
use num_bigint::BigInt;
use num_rational::BigRational;
use once_cell::unsync::OnceCell;
pub use solang_parser::diagnostics::*;
use solang_parser::pt;
use solang_parser::pt::{CodeLocation, FunctionTy, OptionalCodeLocation};
use std::cell::RefCell;
use std::fmt::Write;
use std::{
    collections::HashSet,
    collections::{BTreeMap, HashMap},
    fmt, hash,
    path::PathBuf,
    sync::Arc,
};
use tiny_keccak::{Hasher, Keccak};

#[derive(PartialEq, Eq, Clone, Hash, Debug)]
pub enum Type {
    Address(bool),
    Bool,
    Int(u16),
    Uint(u16),
    Rational,
    Bytes(u8),
    DynamicBytes,
    String,
    Array(Box<Type>, Vec<ArrayLength>),
    /// The usize is an index into enums in the namespace
    Enum(usize),
    /// The usize is an index into contracts in the namespace
    Struct(StructType),
    Mapping(Mapping),
    /// The usize is an index into contracts in the namespace
    Contract(usize),
    Ref(Box<Type>),
    /// Reference to storage, first bool is true for immutables
    StorageRef(bool, Box<Type>),
    InternalFunction {
        mutability: Mutability,
        params: Vec<Type>,
        returns: Vec<Type>,
    },
    ExternalFunction {
        mutability: Mutability,
        params: Vec<Type>,
        returns: Vec<Type>,
    },
    /// User type definitions, e.g. `type Foo is int128;`. The usize
    /// is an index into user_types in the namespace.
    UserType(usize),
    /// There is no way to declare value in Solidity (should there be?)
    Value,
    Void,
    Unreachable,
    /// DynamicBytes and String are lowered to a vector.
    Slice(Box<Type>),
    /// We could not resolve this type
    Unresolved,
    /// When we advance a pointer, it cannot be any of the previous types.
    /// e.g. Type::Bytes is a pointer to struct.vector. When we advance it, it is a pointer
    /// to latter's data region.
    BufferPointer,
    /// The function selector (or discriminator) type is 4 bytes on Polkadot and 8 bytes on Solana
    FunctionSelector,
}

#[derive(Eq, Clone, Debug)]
pub struct Mapping {
    pub key: Box<Type>,
    pub key_name: Option<pt::Identifier>,
    pub value: Box<Type>,
    pub value_name: Option<pt::Identifier>,
}

// Ensure the key_name and value_name is not used for comparison or hashing
impl PartialEq for Mapping {
    fn eq(&self, other: &Mapping) -> bool {
        self.key == other.key && self.value == other.value
    }
}

impl hash::Hash for Mapping {
    fn hash<H: hash::Hasher>(&self, hasher: &mut H) {
        self.key.hash(hasher);
        self.value.hash(hasher);
    }
}

#[derive(PartialEq, Eq, Clone, Hash, Debug)]
pub enum ArrayLength {
    Fixed(BigInt),
    Dynamic,
    /// Fixed length arrays, any length permitted. This is useful for when we
    /// do not want dynamic length, but want to permit any length. For example
    /// the create_program_address() call takes any number of seeds as its
    /// first argument, and we don't want to allocate a dynamic array for
    /// this parameter as this would be wasteful to allocate a vector for
    /// this argument.
    AnyFixed,
}

impl ArrayLength {
    /// Get the length, if fixed
    pub fn array_length(&self) -> Option<&BigInt> {
        match self {
            ArrayLength::Fixed(len) => Some(len),
            _ => None,
        }
    }
}

pub trait RetrieveType {
    /// Return the type for this expression. This assumes the expression has a single value,
    /// panics will occur otherwise
    fn ty(&self) -> Type;
}

impl Type {
    pub fn get_type_size(&self) -> u16 {
        match self {
            Type::Int(n) | Type::Uint(n) => *n,
            Type::Bool => 1,
            _ => unimplemented!("size of type not known"),
        }
    }

    pub fn unwrap_user_type(self, ns: &Namespace) -> Type {
        if let Type::UserType(type_no) = self {
            ns.user_types[type_no].ty.clone()
        } else {
            self
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug, Copy, Hash)]
pub enum StructType {
    UserDefined(usize),
    AccountInfo,
    AccountMeta,
    ExternalFunction,
    SolParameters,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct StructDecl {
    pub tags: Vec<Tag>,
    pub id: pt::Identifier,
    pub loc: pt::Loc,
    pub contract: Option<String>,
    pub fields: Vec<Parameter<Type>>,
    // List of offsets of the fields, last entry is the offset for the struct overall size
    pub offsets: Vec<BigInt>,
    // Same, but now in storage
    pub storage_offsets: Vec<BigInt>,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct EventDecl {
    pub tags: Vec<Tag>,
    pub id: pt::Identifier,
    pub loc: pt::Loc,
    pub contract: Option<usize>,
    pub fields: Vec<Parameter<Type>>,
    pub signature: String,
    pub anonymous: bool,
    pub used: bool,
}

impl EventDecl {
    pub fn symbol_name(&self, ns: &Namespace) -> String {
        match &self.contract {
            Some(c) => format!("{}.{}", ns.contracts[*c].id, self.id),
            None => self.id.to_string(),
        }
    }
}

#[derive(Default, PartialEq, Eq, Clone, Debug)]
pub struct ErrorDecl {
    pub tags: Vec<Tag>,
    pub name: String,
    pub loc: pt::Loc,
    pub contract: Option<usize>,
    pub fields: Vec<Parameter<Type>>,
    pub used: bool,
}

impl ErrorDecl {
    pub fn symbol_name(&self, ns: &Namespace) -> String {
        match &self.contract {
            Some(c) => format!("{}.{}", ns.contracts[*c].id, self.name),
            None => self.name.to_string(),
        }
    }
}

impl fmt::Display for StructDecl {
    /// Make the struct name into a string for printing. The struct can be declared either
    /// inside or outside a contract.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.contract {
            Some(c) => write!(f, "{}.{}", c, self.id),
            None => write!(f, "{}", self.id),
        }
    }
}

#[derive(Debug)]
pub struct EnumDecl {
    pub tags: Vec<Tag>,
    pub id: pt::Identifier,
    pub contract: Option<String>,
    pub loc: pt::Loc,
    pub ty: Type,
    pub values: IndexMap<String, pt::Loc>,
}

impl fmt::Display for EnumDecl {
    /// Make the enum name into a string for printing. The enum can be declared either
    /// inside or outside a contract.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.contract {
            Some(c) => write!(f, "{}.{}", c, self.id),
            None => write!(f, "{}", self.id),
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Parameter<Type> {
    pub loc: pt::Loc,
    /// The name can empty (e.g. in an event field or unnamed parameter/return)
    pub id: Option<pt::Identifier>,
    pub ty: Type,
    /// Yul function parameters may not have a type identifier
    pub ty_loc: Option<pt::Loc>,
    /// Event fields may indexed, which means they are sent to the log
    pub indexed: bool,
    /// Some builtin structs have readonly fields
    pub readonly: bool,
    /// A recursive struct may contain itself which make the struct infinite size in memory.
    pub infinite_size: bool,
    /// Is this struct field recursive. Recursive does not mean infinite size in all cases:
    /// `struct S { S[] s }` is recursive but not of infinite size.
    pub recursive: bool,

    pub annotation: Option<ParameterAnnotation>,
}

#[derive(Debug, Eq, Clone, PartialEq)]
pub struct ParameterAnnotation {
    pub loc: pt::Loc,
    pub id: pt::Identifier,
}

impl Parameter<Type> {
    /// Create a new instance of the given `Type`, with all other values set to their default.
    pub fn new_default(ty: Type) -> Self {
        Self {
            ty,
            loc: Default::default(),
            id: Default::default(),
            ty_loc: Default::default(),
            indexed: Default::default(),
            readonly: Default::default(),
            infinite_size: Default::default(),
            recursive: Default::default(),
            annotation: Default::default(),
        }
    }

    pub fn name_as_str(&self) -> &str {
        if let Some(name) = &self.id {
            name.name.as_str()
        } else {
            ""
        }
    }
}

#[derive(PartialEq, Eq, Clone, Hash, Debug)]
pub enum Mutability {
    Payable(pt::Loc),
    Nonpayable(pt::Loc),
    View(pt::Loc),
    Pure(pt::Loc),
}

impl Mutability {
    pub fn is_default(&self) -> bool {
        matches!(self, Mutability::Nonpayable(_))
    }
}

impl fmt::Display for Mutability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mutability::Pure(_) => write!(f, "pure"),
            Mutability::View(_) => write!(f, "view"),
            Mutability::Nonpayable(_) => write!(f, "nonpayable"),
            Mutability::Payable(_) => write!(f, "payable"),
        }
    }
}

#[derive(Debug)]
pub struct Function {
    pub tags: Vec<Tag>,
    /// The location of the prototype (not body)
    pub loc_prototype: pt::Loc,
    pub loc: pt::Loc,
    pub id: pt::Identifier,
    pub contract_no: Option<usize>,
    pub ty: pt::FunctionTy,
    pub signature: String,
    pub mutability: Mutability,
    pub visibility: pt::Visibility,
    pub params: Arc<Vec<Parameter<Type>>>,
    pub returns: Arc<Vec<Parameter<Type>>>,
    /// Constructor arguments for base contracts, only present on constructors
    pub bases: BTreeMap<usize, (pt::Loc, usize, Vec<Expression>)>,
    /// Modifiers for functions
    pub modifiers: Vec<Expression>,
    pub is_virtual: bool,
    /// Is this function an acccesor function created by a public variable
    pub is_accessor: bool,
    pub is_override: Option<(pt::Loc, Vec<usize>)>,
    /// The selector (known as discriminator on Solana/Anchor)
    pub selector: Option<(pt::Loc, Vec<u8>)>,
    /// Was the function declared with a body
    pub has_body: bool,
    /// The resolved body (if any)
    pub body: Vec<Statement>,
    pub symtable: Symtable,
    /// What events are emitted by the body of this function
    pub emits_events: Vec<usize>,
    /// For overloaded functions this is the mangled (unique) name.
    pub mangled_name: String,
    /// Solana constructors may have seeds specified using @seed tags
    pub annotations: ConstructorAnnotations,
    /// Which contracts should we use the mangled name in?
    pub mangled_name_contracts: HashSet<usize>,
    /// This indexmap stores the accounts this functions needs to be called on Solana
    /// The string is the account's name
    pub solana_accounts: RefCell<IndexMap<String, SolanaAccount>>,
    /// List of contracts this function creates
    pub creates: Vec<(pt::Loc, usize)>,
}

/// This struct represents a Solana account. There is no name field, because
/// it is stored in a IndexMap<String, SolanaAccount> (see above)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SolanaAccount {
    pub loc: pt::Loc,
    pub is_signer: bool,
    pub is_writer: bool,
    /// Has the compiler automatically generated this account entry?
    pub generated: bool,
}

#[derive(Debug, Default)]
pub struct ConstructorAnnotations {
    // (annotation location, annotation expression)
    pub seeds: Vec<(pt::Loc, Expression)>,
    pub space: Option<(pt::Loc, Expression)>,
    pub bump: Option<(pt::Loc, Expression)>,
    // (annotation location, account name)
    pub payer: Option<(pt::Loc, String)>,
}

/// This trait provides a single interface for fetching parameters, returns and the symbol table
/// for both yul and solidity functions
pub trait FunctionAttributes {
    fn get_symbol_table(&self) -> &Symtable;
    fn get_parameters(&self) -> &Vec<Parameter<Type>>;
    fn get_returns(&self) -> &Vec<Parameter<Type>>;
}

impl FunctionAttributes for Function {
    fn get_symbol_table(&self) -> &Symtable {
        &self.symtable
    }

    fn get_parameters(&self) -> &Vec<Parameter<Type>> {
        &self.params
    }

    fn get_returns(&self) -> &Vec<Parameter<Type>> {
        &self.returns
    }
}

impl Function {
    pub fn new(
        loc_prototype: pt::Loc,
        loc: pt::Loc,
        id: pt::Identifier,
        contract_no: Option<usize>,
        tags: Vec<Tag>,
        ty: pt::FunctionTy,
        mutability: Option<pt::Mutability>,
        visibility: pt::Visibility,
        params: Vec<Parameter<Type>>,
        returns: Vec<Parameter<Type>>,
        ns: &Namespace,
    ) -> Self {
        let signature = match ty {
            pt::FunctionTy::Fallback => String::from("@fallback"),
            pt::FunctionTy::Receive => String::from("@receive"),
            _ => ns.signature(&id.name, &params),
        };

        let mutability = match mutability {
            None => Mutability::Nonpayable(loc_prototype),
            Some(pt::Mutability::Payable(loc)) => Mutability::Payable(loc),
            Some(pt::Mutability::Pure(loc)) => Mutability::Pure(loc),
            Some(pt::Mutability::View(loc)) => Mutability::View(loc),
            Some(pt::Mutability::Constant(loc)) => Mutability::View(loc),
        };

        let mangled_name = signature
            .replace('(', "_")
            .replace(')', "")
            .replace(',', "_")
            .replace("[]", "Array")
            .replace('[', "Array")
            .replace(']', "");

        Function {
            tags,
            loc_prototype,
            loc,
            id,
            contract_no,
            ty,
            signature,
            mutability,
            visibility,
            params: Arc::new(params),
            returns: Arc::new(returns),
            bases: BTreeMap::new(),
            modifiers: Vec::new(),
            selector: None,
            is_virtual: false,
            is_accessor: false,
            has_body: false,
            is_override: None,
            body: Vec::new(),
            symtable: Symtable::default(),
            emits_events: Vec::new(),
            mangled_name,
            annotations: ConstructorAnnotations::default(),
            mangled_name_contracts: HashSet::new(),
            solana_accounts: IndexMap::new().into(),
            creates: Vec::new(),
        }
    }

    /// Generate selector for this function
    pub fn selector(&self, ns: &Namespace, contract_no: &usize) -> Vec<u8> {
        if let Some((_, selector)) = &self.selector {
            selector.clone()
        } else if ns.target == Target::Solana {
            match self.ty {
                FunctionTy::Constructor => function_discriminator("new"),
                _ => {
                    let discriminator_image = if self.mangled_name_contracts.contains(contract_no) {
                        &self.mangled_name
                    } else {
                        &self.id.name
                    };
                    function_discriminator(discriminator_image.as_str())
                }
            }
        } else {
            let mut res = [0u8; 32];

            let mut hasher = Keccak::v256();
            hasher.update(self.signature.as_bytes());
            hasher.finalize(&mut res);

            res[..4].to_vec()
        }
    }

    /// Is this a constructor
    pub fn is_constructor(&self) -> bool {
        self.ty == pt::FunctionTy::Constructor
    }

    /// Does this function have the payable state
    pub fn is_payable(&self) -> bool {
        matches!(self.mutability, Mutability::Payable(_))
    }

    /// Does this function have an @payer annotation?
    pub fn has_payer_annotation(&self) -> bool {
        self.annotations.payer.is_some()
    }

    /// Does this function have an @seed annotation?
    pub fn has_seed_annotation(&self) -> bool {
        !self.annotations.seeds.is_empty()
    }

    /// Does this function have the pure state
    pub fn is_pure(&self) -> bool {
        matches!(self.mutability, Mutability::Pure(_))
    }

    /// Is this function visible externally, based on it's visibilty modifiers.
    ///
    /// Due to inheritance, this alone does not determine whether a function is
    /// externally callable in the final contract artifact; for that, use
    /// `Namespace::function_externally_callable()` instead.
    pub fn is_public(&self) -> bool {
        matches!(
            self.visibility,
            pt::Visibility::Public(_) | pt::Visibility::External(_)
        )
    }

    /// Is this function accessable only from same contract
    pub fn is_private(&self) -> bool {
        matches!(self.visibility, pt::Visibility::Private(_))
    }
}

impl From<&pt::Type> for Type {
    fn from(p: &pt::Type) -> Type {
        match p {
            pt::Type::Bool => Type::Bool,
            pt::Type::Address => Type::Address(false),
            pt::Type::AddressPayable => Type::Address(true),
            pt::Type::Payable => Type::Address(true),
            pt::Type::Int(n) => Type::Int(*n),
            pt::Type::Uint(n) => Type::Uint(*n),
            pt::Type::Bytes(n) => Type::Bytes(*n),
            pt::Type::String => Type::String,
            pt::Type::Rational => Type::Rational,
            pt::Type::DynamicBytes => Type::DynamicBytes,
            // needs special casing
            pt::Type::Function { .. } | pt::Type::Mapping { .. } => unimplemented!(),
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct UserTypeDecl {
    pub tags: Vec<Tag>,
    pub loc: pt::Loc,
    pub name: String,
    pub ty: Type,
    pub contract: Option<String>,
}

impl fmt::Display for UserTypeDecl {
    /// Make the user type name into a string for printing. The user type can
    /// be declared either inside or outside a contract.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.contract {
            Some(c) => write!(f, "{}.{}", c, self.name),
            None => write!(f, "{}", self.name),
        }
    }
}

#[derive(Debug)]
pub struct Variable {
    pub tags: Vec<Tag>,
    pub name: String,
    pub loc: pt::Loc,
    pub ty: Type,
    pub visibility: pt::Visibility,
    pub constant: bool,
    pub immutable: bool,
    pub initializer: Option<Expression>,
    pub assigned: bool,
    pub read: bool,
    pub storage_type: Option<pt::StorageType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Symbol {
    Enum(pt::Loc, usize),
    Function(Vec<(pt::Loc, usize)>),
    Variable(pt::Loc, Option<usize>, usize),
    Struct(pt::Loc, StructType),
    Event(Vec<(pt::Loc, usize)>),
    Error(pt::Loc, usize),
    Contract(pt::Loc, usize),
    Import(pt::Loc, usize),
    UserType(pt::Loc, usize),
}

impl CodeLocation for Symbol {
    fn loc(&self) -> pt::Loc {
        match self {
            Symbol::Enum(loc, _)
            | Symbol::Variable(loc, ..)
            | Symbol::Struct(loc, _)
            | Symbol::Contract(loc, _)
            | Symbol::Import(loc, _)
            | Symbol::Error(loc, _)
            | Symbol::UserType(loc, _) => *loc,
            Symbol::Event(items) | Symbol::Function(items) => items[0].0,
        }
    }
}

impl Symbol {
    /// Is this symbol for an event
    pub fn is_event(&self) -> bool {
        matches!(self, Symbol::Event(_))
    }

    /// Does this symbol have an accessor function
    pub fn has_accessor(&self, ns: &Namespace) -> bool {
        if let Symbol::Variable(_, Some(contract_no), var_no) = self {
            matches!(
                ns.contracts[*contract_no].variables[*var_no].visibility,
                pt::Visibility::Public(_)
            )
        } else {
            false
        }
    }

    /// Is this a private symbol
    pub fn is_private_variable(&self, ns: &Namespace) -> bool {
        match self {
            Symbol::Variable(_, Some(contract_no), var_no) => {
                let visibility = &ns.contracts[*contract_no].variables[*var_no].visibility;

                matches!(visibility, pt::Visibility::Private(_))
            }
            _ => false,
        }
    }
}

/// Any Solidity file, either the main file or anything that was imported
#[derive(Clone, Debug)]
pub struct File {
    /// The on-disk filename
    pub path: PathBuf,
    /// Used for offset to line-column conversions
    pub line_starts: Vec<usize>,
    /// Indicates the file number in FileResolver.files
    pub cache_no: Option<usize>,
    /// Index into FileResolver.import_paths. This is `None` when this File was
    /// created not during `parse_and_resolve` (e.g., builtins)
    pub import_no: Option<usize>,
}

/// When resolving a Solidity file, this holds all the resolved items
#[derive(Debug)]
pub struct Namespace {
    pub target: Target,
    pub pragmas: Vec<Pragma>,
    pub files: Vec<File>,
    pub enums: Vec<EnumDecl>,
    pub structs: Vec<StructDecl>,
    pub events: Vec<EventDecl>,
    pub errors: Vec<ErrorDecl>,
    pub contracts: Vec<Contract>,
    /// Global using declarations
    pub using: Vec<Using>,
    /// All type declarations
    pub user_types: Vec<UserTypeDecl>,
    /// All functions
    pub functions: Vec<Function>,
    /// Yul functions
    pub yul_functions: Vec<YulFunction>,
    /// Global constants
    pub constants: Vec<Variable>,
    /// address length in bytes
    pub address_length: usize,
    /// value length in bytes
    pub value_length: usize,
    pub diagnostics: Diagnostics,
    /// There is a separate namespace for functions and non-functions
    pub function_symbols: HashMap<(usize, Option<usize>, String), Symbol>,
    /// Symbol key is file_no, contract, identifier
    pub variable_symbols: HashMap<(usize, Option<usize>, String), Symbol>,
    // each variable in the symbol table should have a unique number
    pub next_id: usize,
    /// For a variable reference at a location, give the constant value
    /// This for use by the language server to show the value of a variable at a location
    pub var_constants: HashMap<pt::Loc, codegen::Expression>,
    /// Overrides for hover in the language server
    pub hover_overrides: HashMap<pt::Loc, String>,
}

#[derive(Debug)]
pub enum Pragma {
    Identifier {
        loc: pt::Loc,
        name: pt::Identifier,
        value: pt::Identifier,
    },
    StringLiteral {
        loc: pt::Loc,
        name: pt::Identifier,
        value: pt::StringLiteral,
    },
    SolidityVersion {
        loc: pt::Loc,
        versions: Vec<VersionReq>,
    },
}

#[derive(Debug)]
pub enum VersionReq {
    Plain {
        loc: pt::Loc,
        version: Version,
    },
    Operator {
        loc: pt::Loc,
        op: pt::VersionOp,
        version: Version,
    },
    Range {
        loc: pt::Loc,
        from: Version,
        to: Version,
    },
    Or {
        loc: pt::Loc,
        left: Box<VersionReq>,
        right: Box<VersionReq>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Version {
    pub major: u32,
    pub minor: Option<u32>,
    pub patch: Option<u32>,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.major.fmt(f)?;
        if let Some(minor) = self.minor {
            f.write_char('.')?;
            minor.fmt(f)?
        }
        if let Some(patch) = self.patch {
            f.write_char('.')?;
            patch.fmt(f)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Layout {
    pub slot: BigInt,
    pub contract_no: usize,
    pub var_no: usize,
    pub ty: Type,
}

#[derive(Debug)]
pub struct Base {
    pub loc: pt::Loc,
    pub contract_no: usize,
    pub constructor: Option<(usize, Vec<Expression>)>,
}

#[derive(Debug)]
pub struct Using {
    pub list: UsingList,
    pub ty: Option<Type>,
    pub file_no: Option<usize>,
}

#[derive(Debug)]
pub enum UsingList {
    Library(usize),
    Functions(Vec<UsingFunction>),
}

/// Using binding for a function, optionally for an operator
#[derive(Debug)]
pub struct UsingFunction {
    pub loc: pt::Loc,
    pub function_no: usize,
    pub oper: Option<pt::UserDefinedOperator>,
}

#[derive(Debug)]
pub struct Contract {
    pub tags: Vec<Tag>,
    pub loc: pt::Loc,
    pub ty: pt::ContractTy,
    pub id: pt::Identifier,
    pub bases: Vec<Base>,
    pub using: Vec<Using>,
    pub layout: Vec<Layout>,
    pub fixed_layout_size: BigInt,
    pub functions: Vec<usize>,
    pub all_functions: BTreeMap<usize, usize>,
    /// maps the name of virtual functions to a vector of overriden functions.
    /// Each time a virtual function is overriden, there will be an entry pushed to the vector. The last
    /// element represents the current overriding function - there will be at least one entry in this vector.
    pub virtual_functions: HashMap<String, Vec<usize>>,
    pub yul_functions: Vec<usize>,
    pub variables: Vec<Variable>,
    /// List of contracts this contract instantiates
    pub creates: Vec<usize>,
    /// List of events this contract may emit
    pub emits_events: Vec<usize>,
    pub initializer: Option<usize>,
    pub default_constructor: Option<(Function, usize)>,
    pub cfg: Vec<ControlFlowGraph>,
    /// Compiled program. Only available after emit.
    pub code: OnceCell<Vec<u8>>,
    /// Can the contract be instantiated, i.e. not abstract, no errors, etc.
    pub instantiable: bool,
    /// Account of deployed program code on Solana
    pub program_id: Option<Vec<u8>>,
}

impl Contract {
    // Is this a concrete contract, which can be instantiated
    pub fn is_concrete(&self) -> bool {
        matches!(self.ty, pt::ContractTy::Contract(_))
    }

    // Is this an interface
    pub fn is_interface(&self) -> bool {
        matches!(self.ty, pt::ContractTy::Interface(_))
    }

    // Is this an library
    pub fn is_library(&self) -> bool {
        matches!(self.ty, pt::ContractTy::Library(_))
    }

    /// Does the constructor require arguments. Should be false is there is no constructor
    pub fn constructor_needs_arguments(&self, ns: &Namespace) -> bool {
        !self.constructors(ns).is_empty() && self.no_args_constructor(ns).is_none()
    }

    /// Does the contract have a constructor defined?
    /// Returns all the constructor function numbers if any
    pub fn constructors(&self, ns: &Namespace) -> Vec<usize> {
        self.functions
            .iter()
            .copied()
            .filter(|func_no| ns.functions[*func_no].is_constructor())
            .collect::<Vec<usize>>()
    }

    /// Return the constructor with no arguments
    pub fn no_args_constructor(&self, ns: &Namespace) -> Option<usize> {
        self.functions
            .iter()
            .find(|func_no| {
                let func = &ns.functions[**func_no];

                func.is_constructor() && func.params.is_empty()
            })
            .cloned()
    }
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum Expression {
    BoolLiteral {
        loc: pt::Loc,
        value: bool,
    },
    BytesLiteral {
        loc: pt::Loc,
        ty: Type,
        value: Vec<u8>,
    },
    NumberLiteral {
        loc: pt::Loc,
        ty: Type,
        value: BigInt,
    },
    RationalNumberLiteral {
        loc: pt::Loc,
        ty: Type,
        value: BigRational,
    },
    StructLiteral {
        loc: pt::Loc,
        id: pt::IdentifierPath,
        ty: Type,
        /// pt::Identifier represents the field name
        values: Vec<(Option<pt::Identifier>, Expression)>,
    },
    ArrayLiteral {
        loc: pt::Loc,
        ty: Type,
        dimensions: Vec<u32>,
        values: Vec<Expression>,
    },
    ConstArrayLiteral {
        loc: pt::Loc,
        ty: Type,
        dimensions: Vec<u32>,
        values: Vec<Expression>,
    },
    Add {
        loc: pt::Loc,
        ty: Type,
        /// Do not check for overflow, i.e. in `unchecked {}` block
        unchecked: bool,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Subtract {
        loc: pt::Loc,
        ty: Type,
        /// Do not check for overflow, i.e. in `unchecked {}` block
        unchecked: bool,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Multiply {
        loc: pt::Loc,
        ty: Type,
        /// Do not check for overflow, i.e. in `unchecked {}` block
        unchecked: bool,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Divide {
        loc: pt::Loc,
        ty: Type,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Modulo {
        loc: pt::Loc,
        ty: Type,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Power {
        loc: pt::Loc,
        ty: Type,
        /// Do not check for overflow, i.e. in `unchecked {}` block
        unchecked: bool,
        base: Box<Expression>,
        exp: Box<Expression>,
    },
    BitwiseOr {
        loc: pt::Loc,
        ty: Type,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    BitwiseAnd {
        loc: pt::Loc,
        ty: Type,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    BitwiseXor {
        loc: pt::Loc,
        ty: Type,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    ShiftLeft {
        loc: pt::Loc,
        ty: Type,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    ShiftRight {
        loc: pt::Loc,
        ty: Type,
        left: Box<Expression>,
        right: Box<Expression>,
        sign: bool,
    },
    Variable {
        loc: pt::Loc,
        ty: Type,
        var_no: usize,
    },
    ConstantVariable {
        loc: pt::Loc,
        ty: Type,
        contract_no: Option<usize>,
        var_no: usize,
    },
    StorageVariable {
        loc: pt::Loc,
        ty: Type,
        contract_no: usize,
        var_no: usize,
    },
    Load {
        loc: pt::Loc,
        ty: Type,
        expr: Box<Expression>,
    },
    GetRef {
        loc: pt::Loc,
        ty: Type,
        expr: Box<Expression>,
    },
    StorageLoad {
        loc: pt::Loc,
        ty: Type,
        expr: Box<Expression>,
    },
    ZeroExt {
        loc: pt::Loc,
        to: Type,
        expr: Box<Expression>,
    },
    SignExt {
        loc: pt::Loc,
        to: Type,
        expr: Box<Expression>,
    },
    Trunc {
        loc: pt::Loc,
        to: Type,
        expr: Box<Expression>,
    },
    CheckingTrunc {
        loc: pt::Loc,
        to: Type,
        expr: Box<Expression>,
    },
    Cast {
        loc: pt::Loc,
        to: Type,
        expr: Box<Expression>,
    },
    BytesCast {
        loc: pt::Loc,
        from: Type,
        to: Type,
        expr: Box<Expression>,
    },
    PreIncrement {
        loc: pt::Loc,
        ty: Type,
        /// Do not check for overflow, i.e. in `unchecked {}` block
        unchecked: bool,
        expr: Box<Expression>,
    },
    PreDecrement {
        loc: pt::Loc,
        ty: Type,
        /// Do not check for overflow, i.e. in `unchecked {}` block
        unchecked: bool,
        expr: Box<Expression>,
    },
    PostIncrement {
        loc: pt::Loc,
        ty: Type,
        /// Do not check for overflow, i.e. in `unchecked {}` block
        unchecked: bool,
        expr: Box<Expression>,
    },
    PostDecrement {
        loc: pt::Loc,
        ty: Type,
        /// Do not check for overflow, i.e. in `unchecked {}` block
        unchecked: bool,
        expr: Box<Expression>,
    },
    Assign {
        loc: pt::Loc,
        ty: Type,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    More {
        loc: pt::Loc,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Less {
        loc: pt::Loc,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    MoreEqual {
        loc: pt::Loc,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    LessEqual {
        loc: pt::Loc,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Equal {
        loc: pt::Loc,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    NotEqual {
        loc: pt::Loc,
        left: Box<Expression>,
        right: Box<Expression>,
    },

    Not {
        loc: pt::Loc,
        expr: Box<Expression>,
    },
    BitwiseNot {
        loc: pt::Loc,
        ty: Type,
        expr: Box<Expression>,
    },
    Negate {
        loc: pt::Loc,
        ty: Type,
        /// Do not check for overflow, i.e. in `unchecked {}` block
        unchecked: bool,
        expr: Box<Expression>,
    },

    ConditionalOperator {
        loc: pt::Loc,
        ty: Type,
        cond: Box<Expression>,
        true_option: Box<Expression>,
        false_option: Box<Expression>,
    },
    Subscript {
        loc: pt::Loc,
        ty: Type,
        array_ty: Type,
        array: Box<Expression>,
        index: Box<Expression>,
    },
    NamedMember {
        loc: pt::Loc,
        ty: Type,
        array: Box<Expression>,
        name: String,
    },
    StructMember {
        loc: pt::Loc,
        ty: Type,
        expr: Box<Expression>,
        field: usize,
    },

    AllocDynamicBytes {
        loc: pt::Loc,
        ty: Type,
        length: Box<Expression>,
        init: Option<Vec<u8>>,
    },
    StorageArrayLength {
        loc: pt::Loc,
        ty: Type,
        array: Box<Expression>,
        elem_ty: Type,
    },
    StringCompare {
        loc: pt::Loc,
        left: StringLocation<Expression>,
        right: StringLocation<Expression>,
    },

    Or {
        loc: pt::Loc,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    And {
        loc: pt::Loc,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    InternalFunction {
        loc: pt::Loc,
        id: pt::IdentifierPath,
        ty: Type,
        function_no: usize,
        signature: Option<String>,
    },
    ExternalFunction {
        loc: pt::Loc,
        ty: Type,
        address: Box<Expression>,
        function_no: usize,
    },
    InternalFunctionCall {
        loc: pt::Loc,
        returns: Vec<Type>,
        function: Box<Expression>,
        args: Vec<Expression>,
    },
    ExternalFunctionCall {
        loc: pt::Loc,
        returns: Vec<Type>,
        function: Box<Expression>,
        args: Vec<Expression>,
        call_args: CallArgs,
    },
    ExternalFunctionCallRaw {
        loc: pt::Loc,
        ty: CallTy,
        address: Box<Expression>,
        args: Box<Expression>,
        call_args: CallArgs,
    },
    Constructor {
        loc: pt::Loc,
        contract_no: usize,
        constructor_no: Option<usize>,
        args: Vec<Expression>,
        call_args: CallArgs,
    },
    FormatString {
        loc: pt::Loc,
        format: Vec<(FormatArg, Expression)>,
    },
    Builtin {
        loc: pt::Loc,
        tys: Vec<Type>,
        kind: Builtin,
        args: Vec<Expression>,
    },
    List {
        loc: pt::Loc,
        list: Vec<Expression>,
    },
    UserDefinedOperator {
        loc: pt::Loc,
        ty: Type,
        oper: pt::UserDefinedOperator,
        function_no: usize,
        args: Vec<Expression>,
    },
    EventSelector {
        loc: pt::Loc,
        ty: Type,
        event_no: usize,
    },
    TypeOperator {
        loc: pt::Loc,
        ty: Type,
    },
}

#[derive(PartialEq, Eq, Clone, Default, Debug)]
pub struct CallArgs {
    pub gas: Option<Box<Expression>>,
    pub salt: Option<Box<Expression>>,
    pub value: Option<Box<Expression>>,
    pub accounts: ExternalCallAccounts<Box<Expression>>,
    pub seeds: Option<Box<Expression>>,
    pub flags: Option<Box<Expression>>,
    pub program_id: Option<Box<Expression>>,
}

/// This enum manages the accounts in an external call on Solana. There can be three options:
/// 1. The developer explicitly specifies there are not accounts for the call (`NoAccount`).
/// 2. The accounts call argument is absent, in which case we attempt to generate the AccountMetas
///    vector automatically (`AbsentArgumet`).
/// 3. There are accounts specified in the accounts call argument (Present).
#[derive(PartialEq, Eq, Clone, Debug, Default)]
pub enum ExternalCallAccounts<T> {
    NoAccount,
    #[default]
    AbsentArgument,
    Present(T),
}

impl<T> ExternalCallAccounts<T> {
    /// Is the accounts call argument missing?
    pub fn is_absent(&self) -> bool {
        matches!(self, ExternalCallAccounts::AbsentArgument)
    }

    /// Returns if the accounts call argument was present in the call
    pub fn argument_provided(&self) -> bool {
        matches!(
            self,
            ExternalCallAccounts::Present(_) | ExternalCallAccounts::NoAccount
        )
    }

    /// Applies a function on the nested objects
    pub fn map<P, F>(&self, func: F) -> ExternalCallAccounts<P>
    where
        F: FnOnce(&T) -> P,
    {
        match self {
            NoAccount => NoAccount,
            AbsentArgument => AbsentArgument,
            ExternalCallAccounts::Present(value) => ExternalCallAccounts::Present(func(value)),
        }
    }

    /// Transform the nested object into a reference
    pub const fn as_ref(&self) -> ExternalCallAccounts<&T> {
        match self {
            ExternalCallAccounts::Present(value) => ExternalCallAccounts::Present(value),
            NoAccount => NoAccount,
            AbsentArgument => AbsentArgument,
        }
    }

    /// Return a reference to the nested object
    pub fn unwrap(&self) -> &T {
        match self {
            ExternalCallAccounts::Present(value) => value,
            _ => panic!("unwrap called at variant without a nested object"),
        }
    }
}

impl Recurse for CallArgs {
    type ArgType = Expression;
    fn recurse<T>(&self, cx: &mut T, f: fn(expr: &Expression, ctx: &mut T) -> bool) {
        if let Some(gas) = &self.gas {
            gas.recurse(cx, f);
        }
        if let Some(salt) = &self.salt {
            salt.recurse(cx, f);
        }
        if let Some(value) = &self.value {
            value.recurse(cx, f);
        }
        if let ExternalCallAccounts::Present(accounts) = &self.accounts {
            accounts.recurse(cx, f);
        }
        if let Some(flags) = &self.flags {
            flags.recurse(cx, f);
        }
    }
}

impl Recurse for Expression {
    type ArgType = Expression;
    fn recurse<T>(&self, cx: &mut T, f: fn(expr: &Expression, ctx: &mut T) -> bool) {
        if f(self, cx) {
            match self {
                Expression::StructLiteral { values, .. } => {
                    for (_, e) in values {
                        e.recurse(cx, f);
                    }
                }

                Expression::ArrayLiteral { values, .. }
                | Expression::ConstArrayLiteral { values, .. } => {
                    for e in values {
                        e.recurse(cx, f);
                    }
                }

                Expression::Load { expr, .. }
                | Expression::StorageLoad { expr, .. }
                | Expression::ZeroExt { expr, .. }
                | Expression::SignExt { expr, .. }
                | Expression::Trunc { expr, .. }
                | Expression::CheckingTrunc { expr, .. }
                | Expression::Cast { expr, .. }
                | Expression::BytesCast { expr, .. }
                | Expression::PreIncrement { expr, .. }
                | Expression::PreDecrement { expr, .. }
                | Expression::PostIncrement { expr, .. }
                | Expression::PostDecrement { expr, .. }
                | Expression::Not { expr, .. }
                | Expression::BitwiseNot { expr, .. }
                | Expression::Negate { expr, .. }
                | Expression::GetRef { expr, .. }
                | Expression::NamedMember { array: expr, .. }
                | Expression::StructMember { expr, .. } => expr.recurse(cx, f),

                Expression::Add { left, right, .. }
                | Expression::Subtract { left, right, .. }
                | Expression::Multiply { left, right, .. }
                | Expression::Divide { left, right, .. }
                | Expression::Modulo { left, right, .. }
                | Expression::Power {
                    base: left,
                    exp: right,
                    ..
                }
                | Expression::BitwiseOr { left, right, .. }
                | Expression::BitwiseAnd { left, right, .. }
                | Expression::BitwiseXor { left, right, .. }
                | Expression::ShiftLeft { left, right, .. }
                | Expression::ShiftRight { left, right, .. }
                | Expression::Assign { left, right, .. }
                | Expression::More { left, right, .. }
                | Expression::Less { left, right, .. }
                | Expression::MoreEqual { left, right, .. }
                | Expression::LessEqual { left, right, .. }
                | Expression::Equal { left, right, .. }
                | Expression::NotEqual { left, right, .. }
                | Expression::Or { left, right, .. }
                | Expression::And { left, right, .. } => {
                    left.recurse(cx, f);
                    right.recurse(cx, f);
                }

                Expression::ConditionalOperator {
                    cond,
                    true_option: left,
                    false_option: right,
                    ..
                } => {
                    cond.recurse(cx, f);
                    left.recurse(cx, f);
                    right.recurse(cx, f);
                }
                Expression::Subscript {
                    array: left,
                    index: right,
                    ..
                } => {
                    left.recurse(cx, f);
                    right.recurse(cx, f);
                }

                Expression::AllocDynamicBytes { length, .. } => length.recurse(cx, f),
                Expression::StorageArrayLength { array, .. } => array.recurse(cx, f),
                Expression::StringCompare { left, right, .. } => {
                    if let StringLocation::RunTime(expr) = left {
                        expr.recurse(cx, f);
                    }
                    if let StringLocation::RunTime(expr) = right {
                        expr.recurse(cx, f);
                    }
                }
                Expression::InternalFunctionCall { function, args, .. } => {
                    function.recurse(cx, f);

                    for e in args {
                        e.recurse(cx, f);
                    }
                }
                Expression::ExternalFunction { address, .. } => {
                    address.recurse(cx, f);
                }
                Expression::ExternalFunctionCall {
                    function,
                    args,
                    call_args,
                    ..
                } => {
                    for e in args {
                        e.recurse(cx, f);
                    }
                    function.recurse(cx, f);
                    call_args.recurse(cx, f);
                }
                Expression::ExternalFunctionCallRaw {
                    address,
                    args,
                    call_args,
                    ..
                } => {
                    args.recurse(cx, f);
                    address.recurse(cx, f);
                    call_args.recurse(cx, f);
                }
                Expression::Constructor {
                    args, call_args, ..
                } => {
                    for e in args {
                        e.recurse(cx, f);
                    }
                    call_args.recurse(cx, f);
                }
                Expression::UserDefinedOperator { args: exprs, .. }
                | Expression::Builtin { args: exprs, .. }
                | Expression::List { list: exprs, .. } => {
                    for e in exprs {
                        e.recurse(cx, f);
                    }
                }

                Expression::FormatString { format, .. } => {
                    for (_, arg) in format {
                        arg.recurse(cx, f);
                    }
                }

                Expression::NumberLiteral { .. }
                | Expression::InternalFunction { .. }
                | Expression::ConstantVariable { .. }
                | Expression::StorageVariable { .. }
                | Expression::Variable { .. }
                | Expression::RationalNumberLiteral { .. }
                | Expression::BytesLiteral { .. }
                | Expression::BoolLiteral { .. }
                | Expression::EventSelector { .. }
                | Expression::TypeOperator { .. } => (),
            }
        }
    }
}

impl CodeLocation for Expression {
    fn loc(&self) -> pt::Loc {
        match self {
            Expression::BoolLiteral { loc, .. }
            | Expression::BytesLiteral { loc, .. }
            | Expression::NumberLiteral { loc, .. }
            | Expression::RationalNumberLiteral { loc, .. }
            | Expression::StructLiteral { loc, .. }
            | Expression::ArrayLiteral { loc, .. }
            | Expression::ConstArrayLiteral { loc, .. }
            | Expression::Add { loc, .. }
            | Expression::Subtract { loc, .. }
            | Expression::Multiply { loc, .. }
            | Expression::Divide { loc, .. }
            | Expression::Modulo { loc, .. }
            | Expression::Power { loc, .. }
            | Expression::BitwiseOr { loc, .. }
            | Expression::BitwiseAnd { loc, .. }
            | Expression::BitwiseXor { loc, .. }
            | Expression::ShiftLeft { loc, .. }
            | Expression::ShiftRight { loc, .. }
            | Expression::Variable { loc, .. }
            | Expression::ConstantVariable { loc, .. }
            | Expression::StorageVariable { loc, .. }
            | Expression::Load { loc, .. }
            | Expression::GetRef { loc, .. }
            | Expression::StorageLoad { loc, .. }
            | Expression::ZeroExt { loc, .. }
            | Expression::SignExt { loc, .. }
            | Expression::Trunc { loc, .. }
            | Expression::CheckingTrunc { loc, .. }
            | Expression::Cast { loc, .. }
            | Expression::BytesCast { loc, .. }
            | Expression::More { loc, .. }
            | Expression::Less { loc, .. }
            | Expression::MoreEqual { loc, .. }
            | Expression::LessEqual { loc, .. }
            | Expression::Equal { loc, .. }
            | Expression::NotEqual { loc, .. }
            | Expression::Not { loc, expr: _ }
            | Expression::BitwiseNot { loc, .. }
            | Expression::Negate { loc, .. }
            | Expression::ConditionalOperator { loc, .. }
            | Expression::Subscript { loc, .. }
            | Expression::StructMember { loc, .. }
            | Expression::Or { loc, .. }
            | Expression::AllocDynamicBytes { loc, .. }
            | Expression::StorageArrayLength { loc, .. }
            | Expression::StringCompare { loc, .. }
            | Expression::InternalFunction { loc, .. }
            | Expression::ExternalFunction { loc, .. }
            | Expression::InternalFunctionCall { loc, .. }
            | Expression::ExternalFunctionCall { loc, .. }
            | Expression::ExternalFunctionCallRaw { loc, .. }
            | Expression::Constructor { loc, .. }
            | Expression::PreIncrement { loc, .. }
            | Expression::PreDecrement { loc, .. }
            | Expression::PostIncrement { loc, .. }
            | Expression::PostDecrement { loc, .. }
            | Expression::Builtin { loc, .. }
            | Expression::Assign { loc, .. }
            | Expression::List { loc, list: _ }
            | Expression::FormatString { loc, format: _ }
            | Expression::And { loc, .. }
            | Expression::NamedMember { loc, .. }
            | Expression::UserDefinedOperator { loc, .. }
            | Expression::EventSelector { loc, .. }
            | Expression::TypeOperator { loc, .. } => *loc,
        }
    }
}

impl CodeLocation for Statement {
    fn loc(&self) -> pt::Loc {
        match self {
            Statement::Block { loc, .. }
            | Statement::VariableDecl(loc, ..)
            | Statement::If(loc, ..)
            | Statement::While(loc, ..)
            | Statement::For { loc, .. }
            | Statement::DoWhile(loc, ..)
            | Statement::Expression(loc, ..)
            | Statement::Delete(loc, ..)
            | Statement::Destructure(loc, ..)
            | Statement::Continue(loc, ..)
            | Statement::Break(loc, ..)
            | Statement::Revert { loc, .. }
            | Statement::Return(loc, ..)
            | Statement::Emit { loc, .. }
            | Statement::TryCatch(loc, ..)
            | Statement::Underscore(loc, ..) => *loc,
            Statement::Assembly(ia, _) => ia.loc,
        }
    }
}

impl CodeLocation for Instr {
    fn loc(&self) -> pt::Loc {
        match self {
            Instr::Set { loc, expr, .. } => match loc {
                pt::Loc::File(_, _, _) => *loc,
                _ => expr.loc(),
            },
            Instr::Call { args, .. } if args.is_empty() => pt::Loc::Codegen,
            Instr::Return { value } if value.is_empty() => pt::Loc::Codegen,
            Instr::Call { args: arr, .. } | Instr::Return { value: arr } => arr[0].loc(),
            Instr::EmitEvent { data: expr, .. }
            | Instr::BranchCond { cond: expr, .. }
            | Instr::Store { dest: expr, .. }
            | Instr::SetStorageBytes { storage: expr, .. }
            | Instr::PushStorage { storage: expr, .. }
            | Instr::PopStorage { storage: expr, .. }
            | Instr::LoadStorage { storage: expr, .. }
            | Instr::ClearStorage { storage: expr, .. }
            | Instr::ExternalCall { value: expr, .. }
            | Instr::SetStorage { value: expr, .. }
            | Instr::Constructor { gas: expr, .. }
            | Instr::ValueTransfer { address: expr, .. }
            | Instr::SelfDestruct { recipient: expr }
            | Instr::WriteBuffer { buf: expr, .. }
            | Instr::Switch { cond: expr, .. }
            | Instr::ReturnData { data: expr, .. }
            | Instr::Print { expr } => expr.loc(),

            Instr::PushMemory { value: expr, .. } => expr.loc(),

            Instr::MemCopy {
                source,
                destination,
                ..
            } => match source.loc() {
                pt::Loc::File(_, _, _) => source.loc(),
                _ => destination.loc(),
            },
            Instr::Branch { .. }
            | Instr::ReturnCode { .. }
            | Instr::Nop
            | Instr::AssertFailure { .. }
            | Instr::PopMemory { .. }
            | Instr::Unimplemented { .. } => pt::Loc::Codegen,

            Instr::AccountAccess { loc, .. } => *loc,
        }
    }
}

#[derive(PartialEq, Clone, Copy, Debug, Eq)]
pub enum FormatArg {
    StringLiteral,
    Default,
    Binary,
    Hex,
}

impl fmt::Display for FormatArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatArg::StringLiteral => write!(f, ""),
            FormatArg::Default => write!(f, ""),
            FormatArg::Binary => write!(f, ":b"),
            FormatArg::Hex => write!(f, ":x"),
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum StringLocation<T> {
    CompileTime(Vec<u8>),
    RunTime(Box<T>),
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Builtin {
    ContractCode,
    GetAddress,
    Balance,
    PayableSend,
    PayableTransfer,
    ArrayPush,
    ArrayPop,
    ArrayLength,
    Assert,
    Print,
    Require,
    SelfDestruct,
    Keccak256,
    Ripemd160,
    Sha256,
    Blake2_128,
    Blake2_256,
    BaseFee,
    PrevRandao,
    Gasleft,
    BlockCoinbase,
    BlockDifficulty,
    GasLimit,
    BlockNumber,
    Slot,
    Timestamp,
    Calldata,
    Sender,
    Signature,
    Value,
    Gasprice,
    Origin,
    BlockHash,
    MinimumBalance,
    AbiDecode,
    AbiEncode,
    AbiEncodePacked,
    AbiEncodeWithSelector,
    AbiEncodeWithSignature,
    AbiEncodeCall,
    MulMod,
    AddMod,
    ChainId,
    ExternalFunctionAddress,
    FunctionSelector,
    SignatureVerify,
    ReadInt8,
    ReadInt16LE,
    ReadInt32LE,
    ReadInt64LE,
    ReadInt128LE,
    ReadInt256LE,
    ReadUint8,
    ReadUint16LE,
    ReadUint32LE,
    ReadUint64LE,
    ReadUint128LE,
    ReadUint256LE,
    ReadAddress,
    WriteInt8,
    WriteInt16LE,
    WriteInt32LE,
    WriteInt64LE,
    WriteInt128LE,
    WriteInt256LE,
    WriteUint8,
    WriteUint16LE,
    WriteUint32LE,
    WriteUint64LE,
    WriteUint128LE,
    WriteUint256LE,
    WriteAddress,
    WriteString,
    WriteBytes,
    Accounts,
    UserTypeWrap,
    UserTypeUnwrap,
    ECRecover,
    StringConcat,
    BytesConcat,
    TypeMin,
    TypeMax,
    TypeName,
    TypeInterfaceId,
    TypeRuntimeCode,
    TypeCreatorCode,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum CallTy {
    Regular,
    Delegate,
    Static,
}

impl fmt::Display for CallTy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CallTy::Regular => write!(f, "regular"),
            CallTy::Static => write!(f, "static"),
            CallTy::Delegate => write!(f, "delegate"),
        }
    }
}

#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Statement {
    Block {
        loc: pt::Loc,
        unchecked: bool,
        statements: Vec<Statement>,
    },
    VariableDecl(pt::Loc, usize, Parameter<Type>, Option<Arc<Expression>>),
    If(pt::Loc, bool, Expression, Vec<Statement>, Vec<Statement>),
    While(pt::Loc, bool, Expression, Vec<Statement>),
    For {
        loc: pt::Loc,
        reachable: bool,
        init: Vec<Statement>,
        cond: Option<Expression>,
        next: Option<Expression>,
        body: Vec<Statement>,
    },
    DoWhile(pt::Loc, bool, Vec<Statement>, Expression),
    Expression(pt::Loc, bool, Expression),
    Delete(pt::Loc, Type, Expression),
    Destructure(pt::Loc, Vec<DestructureField>, Expression),
    Continue(pt::Loc),
    Break(pt::Loc),
    Return(pt::Loc, Option<Expression>),
    Revert {
        loc: pt::Loc,
        error_no: Option<usize>,
        args: Vec<Expression>,
    },
    Emit {
        loc: pt::Loc,
        event_no: usize,
        event_loc: pt::Loc,
        args: Vec<Expression>,
    },
    TryCatch(pt::Loc, bool, TryCatch),
    Underscore(pt::Loc),
    Assembly(InlineAssembly, bool),
}

#[derive(Clone, Debug)]
pub struct TryCatch {
    pub expr: Expression,
    pub returns: Vec<(Option<usize>, Parameter<Type>)>,
    pub ok_stmt: Vec<Statement>,
    pub errors: Vec<CatchClause>,
    pub catch_all: Option<CatchClause>,
}

#[derive(Clone, Debug)]
pub struct CatchClause {
    pub param: Option<Parameter<Type>>,
    pub param_pos: Option<usize>,
    pub stmt: Vec<Statement>,
}

#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum DestructureField {
    None,
    Expression(Expression),
    VariableDecl(usize, Parameter<Type>),
}

impl OptionalCodeLocation for DestructureField {
    fn loc_opt(&self) -> Option<pt::Loc> {
        match self {
            DestructureField::None => None,
            DestructureField::Expression(e) => Some(e.loc()),
            DestructureField::VariableDecl(_, p) => Some(p.loc),
        }
    }
}

impl Recurse for Statement {
    type ArgType = Statement;
    fn recurse<T>(&self, cx: &mut T, f: fn(stmt: &Statement, ctx: &mut T) -> bool) {
        if f(self, cx) {
            match self {
                Statement::Block { statements, .. } => {
                    for stmt in statements {
                        stmt.recurse(cx, f);
                    }
                }
                Statement::If(_, _, _, then_stmt, else_stmt) => {
                    for stmt in then_stmt {
                        stmt.recurse(cx, f);
                    }

                    for stmt in else_stmt {
                        stmt.recurse(cx, f);
                    }
                }
                Statement::For { init, body, .. } => {
                    for stmt in init {
                        stmt.recurse(cx, f);
                    }

                    for stmt in body {
                        stmt.recurse(cx, f);
                    }
                }
                Statement::While(_, _, _, body) => {
                    for stmt in body {
                        stmt.recurse(cx, f);
                    }
                }
                Statement::DoWhile(_, _, body, _) => {
                    for stmt in body {
                        stmt.recurse(cx, f);
                    }
                }
                Statement::TryCatch(_, _, try_catch) => {
                    for stmt in &try_catch.ok_stmt {
                        stmt.recurse(cx, f);
                    }

                    for clause in &try_catch.errors {
                        for stmt in &clause.stmt {
                            stmt.recurse(cx, f);
                        }
                    }

                    if let Some(clause) = try_catch.catch_all.as_ref() {
                        for stmt in &clause.stmt {
                            stmt.recurse(cx, f);
                        }
                    }
                }
                _ => (),
            }
        }
    }
}

impl Statement {
    /// Shorthand for checking underscore
    pub fn is_underscore(&self) -> bool {
        matches!(&self, Statement::Underscore(_))
    }

    pub fn reachable(&self) -> bool {
        match self {
            Statement::Block { statements, .. } => statements.iter().all(|s| s.reachable()),
            Statement::Underscore(_)
            | Statement::Destructure(..)
            | Statement::VariableDecl(..)
            | Statement::Emit { .. }
            | Statement::Delete(..) => true,

            Statement::Continue(_)
            | Statement::Break(_)
            | Statement::Return(..)
            | Statement::Revert { .. } => false,

            Statement::If(_, reachable, ..)
            | Statement::While(_, reachable, ..)
            | Statement::DoWhile(_, reachable, ..)
            | Statement::Expression(_, reachable, _)
            | Statement::For { reachable, .. }
            | Statement::TryCatch(_, reachable, _)
            | Statement::Assembly(_, reachable) => *reachable,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Tag {
    pub loc: pt::Loc,
    pub tag: String,
    pub no: usize,
    pub value: String,
}
