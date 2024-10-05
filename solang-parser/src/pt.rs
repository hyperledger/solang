// SPDX-License-Identifier: Apache-2.0

//! Solidity parse tree data structures.
//!
//! See also the [Solidity documentation][sol].
//!
//! [sol]: https://docs.soliditylang.org/en/latest/grammar.html

// backwards compatibility re-export
#[doc(hidden)]
pub use crate::helpers::{CodeLocation, OptionalCodeLocation};

#[cfg(feature = "pt-serde")]
use serde::{Deserialize, Serialize};

/// A code location.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub enum Loc {
    /// Builtin
    Builtin,
    /// Command line
    CommandLine,
    /// Implicit
    Implicit,
    /// Codegen
    Codegen,
    /// The file number, start offset and end offset in bytes of the source file.
    File(usize, usize, usize),
}

impl Default for Loc {
    fn default() -> Self {
        Self::File(0, 0, 0)
    }
}

#[inline(never)]
#[cold]
#[track_caller]
fn not_a_file() -> ! {
    panic!("location is not a file")
}

impl Loc {
    /// Returns this location's beginning range.
    #[inline]
    pub fn begin_range(&self) -> Self {
        match self {
            Loc::File(file_no, start, _) => Loc::File(*file_no, *start, *start),
            loc => *loc,
        }
    }

    /// Returns this location's end range.
    #[inline]
    pub fn end_range(&self) -> Self {
        match self {
            Loc::File(file_no, _, end) => Loc::File(*file_no, *end, *end),
            loc => *loc,
        }
    }

    /// Returns this location's file number.
    ///
    /// # Panics
    ///
    /// If this location is not a file.
    #[track_caller]
    #[inline]
    pub fn file_no(&self) -> usize {
        match self {
            Loc::File(file_no, _, _) => *file_no,
            _ => not_a_file(),
        }
    }

    /// Returns this location's file number if it is a file, otherwise `None`.
    #[inline]
    pub fn try_file_no(&self) -> Option<usize> {
        match self {
            Loc::File(file_no, _, _) => Some(*file_no),
            _ => None,
        }
    }

    /// Returns this location's start.
    ///
    /// # Panics
    ///
    /// If this location is not a file.
    #[track_caller]
    #[inline]
    pub fn start(&self) -> usize {
        match self {
            Loc::File(_, start, _) => *start,
            _ => not_a_file(),
        }
    }

    /// Returns this location's end.
    ///
    /// # Panics
    ///
    /// If this location is not a file.
    #[track_caller]
    #[inline]
    pub fn end(&self) -> usize {
        match self {
            Loc::File(_, _, end) => *end,
            _ => not_a_file(),
        }
    }

    /// Returns this location's end.
    /// The returned endpoint is not part of the range.
    /// This is used when a half-open range is needed.
    ///
    /// # Panics
    ///
    /// If this location is not a file.
    #[track_caller]
    #[inline]
    pub fn exclusive_end(&self) -> usize {
        self.end() + 1
    }

    /// Replaces this location's start with `other`'s.
    ///
    /// # Panics
    ///
    /// If this location is not a file.
    #[track_caller]
    #[inline]
    pub fn use_start_from(&mut self, other: &Loc) {
        match (self, other) {
            (Loc::File(_, start, _), Loc::File(_, other_start, _)) => {
                *start = *other_start;
            }
            _ => not_a_file(),
        }
    }

    /// Replaces this location's end with `other`'s.
    ///
    /// # Panics
    ///
    /// If this location is not a file.
    #[track_caller]
    #[inline]
    pub fn use_end_from(&mut self, other: &Loc) {
        match (self, other) {
            (Loc::File(_, _, end), Loc::File(_, _, other_end)) => {
                *end = *other_end;
            }
            _ => not_a_file(),
        }
    }

    /// See [`Loc::use_start_from`].
    ///
    /// # Panics
    ///
    /// If this location is not a file.
    #[track_caller]
    #[inline]
    pub fn with_start_from(mut self, other: &Self) -> Self {
        self.use_start_from(other);
        self
    }

    /// See [`Loc::use_end_from`].
    ///
    /// # Panics
    ///
    /// If this location is not a file.
    #[track_caller]
    #[inline]
    pub fn with_end_from(mut self, other: &Self) -> Self {
        self.use_end_from(other);
        self
    }

    /// Creates a new `Loc::File` by replacing `start`.
    ///
    /// # Panics
    ///
    /// If this location is not a file.
    #[track_caller]
    #[inline]
    pub fn with_start(self, start: usize) -> Self {
        match self {
            Self::File(no, _, end) => Self::File(no, start, end),
            _ => not_a_file(),
        }
    }

    /// Creates a new `Loc::File` by replacing `end`.
    ///
    /// # Panics
    ///
    /// If this location is not a file.
    #[track_caller]
    #[inline]
    pub fn with_end(self, end: usize) -> Self {
        match self {
            Self::File(no, start, _) => Self::File(no, start, end),
            _ => not_a_file(),
        }
    }

    /// Returns this location's range.
    ///
    /// # Panics
    ///
    /// If this location is not a file.
    #[track_caller]
    #[inline]
    pub fn range(self) -> std::ops::Range<usize> {
        match self {
            Self::File(_, start, end) => start..end,
            _ => not_a_file(),
        }
    }

    /// Performs the union of two locations
    pub fn union(&mut self, other: &Self) {
        match (self, other) {
            (Self::File(r_file, r_start, r_end), Self::File(l_file, l_start, l_end)) => {
                assert_eq!(r_file, l_file, "cannot perform union in different files");
                *r_start = std::cmp::min(*r_start, *l_start);
                *r_end = std::cmp::max(*r_end, *l_end);
            }

            _ => unimplemented!("cannot perform union in non File Loc"),
        }
    }
}

/// An identifier.
///
/// `<name>`
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct Identifier {
    /// The code location.
    pub loc: Loc,
    /// The identifier string.
    pub name: String,
}

impl Identifier {
    /// Creates a new identifier.
    pub fn new(s: impl Into<String>) -> Self {
        Self {
            loc: Loc::default(),
            name: s.into(),
        }
    }
}

/// A qualified identifier.
///
/// `<identifiers>.*`
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct IdentifierPath {
    /// The code location.
    pub loc: Loc,
    /// The list of identifiers.
    pub identifiers: Vec<Identifier>,
}

/// A comment or [doc comment][natspec].
///
/// [natspec]: https://docs.soliditylang.org/en/latest/natspec-format.html
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub enum Comment {
    /// A line comment.
    ///
    /// `// line comment`
    Line(Loc, String),

    /// A block doc comment.
    ///
    /// ` /* block comment */ `
    Block(Loc, String),

    /// A line doc comment.
    ///
    /// `/// line doc comment`
    DocLine(Loc, String),

    /// A block doc comment.
    ///
    /// ```text
    /// /**
    ///  * block doc comment
    ///  */
    /// ```
    DocBlock(Loc, String),
}

impl Comment {
    /// Returns the comment's value.
    #[inline]
    pub const fn value(&self) -> &String {
        match self {
            Self::Line(_, s) | Self::Block(_, s) | Self::DocLine(_, s) | Self::DocBlock(_, s) => s,
        }
    }

    /// Returns whether this is a doc comment.
    #[inline]
    pub const fn is_doc(&self) -> bool {
        matches!(self, Self::DocLine(..) | Self::DocBlock(..))
    }

    /// Returns whether this is a line comment.
    #[inline]
    pub const fn is_line(&self) -> bool {
        matches!(self, Self::Line(..) | Self::DocLine(..))
    }

    /// Returns whether this is a block comment.
    #[inline]
    pub const fn is_block(&self) -> bool {
        !self.is_line()
    }
}

/// The source unit of the parse tree.
///
/// Contains all of the parse tree's parts in a vector.
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct SourceUnit(pub Vec<SourceUnitPart>);

/// A parse tree part.
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub enum SourceUnitPart {
    /// A pragma directive.
    ///
    /// `pragma <1> <2>;`
    ///
    /// `1` and `2` are `None` only if an error occurred during parsing.
    PragmaDirective(Box<PragmaDirective>),

    /// An import directive.
    ImportDirective(Import),

    /// A contract definition.
    ContractDefinition(Box<ContractDefinition>),

    /// An enum definition.
    EnumDefinition(Box<EnumDefinition>),

    /// A struct definition.
    StructDefinition(Box<StructDefinition>),

    /// An event definition.
    EventDefinition(Box<EventDefinition>),

    /// An error definition.
    ErrorDefinition(Box<ErrorDefinition>),

    /// A function definition.
    FunctionDefinition(Box<FunctionDefinition>),

    /// A variable definition.
    VariableDefinition(Box<VariableDefinition>),

    /// A type definition.
    TypeDefinition(Box<TypeDefinition>),

    /// An annotation.
    Annotation(Box<Annotation>),

    /// A `using` directive.
    Using(Box<Using>),

    /// A stray semicolon.
    StraySemicolon(Loc),
}

/// An import statement.
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub enum Import {
    /// `import <0>;`
    Plain(ImportPath, Loc),

    /// `import * as <1> from <0>;`
    ///
    /// or
    ///
    /// `import <0> as <1>;`
    GlobalSymbol(ImportPath, Identifier, Loc),

    /// `import { <<1.0> [as <1.1>]>,* } from <0>;`
    Rename(ImportPath, Vec<(Identifier, Option<Identifier>)>, Loc),
}

/// An import statement.
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub enum ImportPath {
    /// "foo.sol"
    Filename(StringLiteral),
    /// std.stub (experimental Solidity feature)
    Path(IdentifierPath),
}

impl Import {
    /// Returns the import string.
    #[inline]
    pub const fn literal(&self) -> Option<&StringLiteral> {
        match self {
            Self::Plain(ImportPath::Filename(literal), _)
            | Self::GlobalSymbol(ImportPath::Filename(literal), _, _)
            | Self::Rename(ImportPath::Filename(literal), _, _) => Some(literal),
            _ => None,
        }
    }
}

/// Type alias for a list of function parameters.
pub type ParameterList = Vec<(Loc, Option<Parameter>)>;

/// A type.
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub enum Type {
    /// `address`
    Address,

    /// `address payable`
    AddressPayable,

    /// `payable`
    ///
    /// Only used as a cast.
    Payable,

    /// `bool`
    Bool,

    /// `string`
    String,

    /// `int<n>`
    Int(u16),

    /// `uint<n>`
    Uint(u16),

    /// `bytes<n>`
    Bytes(u8),

    /// `fixed`
    Rational,

    /// `bytes`
    DynamicBytes,

    /// `mapping(<key> [key_name] => <value> [value_name])`
    Mapping {
        /// The code location.
        loc: Loc,
        /// The key expression.
        ///
        /// This is only allowed to be an elementary type or a user defined type.
        key: Box<Expression>,
        /// The optional key identifier.
        key_name: Option<Identifier>,
        /// The value expression.
        value: Box<Expression>,
        /// The optional value identifier.
        value_name: Option<Identifier>,
    },

    /// `function (<params>) <attributes> [returns]`
    Function {
        /// The list of parameters.
        params: ParameterList,
        /// The list of attributes.
        attributes: Vec<FunctionAttribute>,
        /// The optional list of return parameters.
        returns: Option<(ParameterList, Vec<FunctionAttribute>)>,
    },
}

/// Dynamic type location.
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub enum StorageLocation {
    /// `memory`
    Memory(Loc),

    /// `storage`
    Storage(Loc),

    /// `calldata`
    Calldata(Loc),
}

/// A variable declaration.
///
/// `<ty> [storage] <name>`
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct VariableDeclaration {
    /// The code location.
    pub loc: Loc,
    /// The type.
    pub ty: Expression,
    /// The optional memory location.
    pub storage: Option<StorageLocation>,
    /// The identifier.
    ///
    /// This field is `None` only if an error occurred during parsing.
    pub name: Option<Identifier>,
}

/// A struct definition.
///
/// `struct <name> { <fields>;* }`
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct StructDefinition {
    /// The code location.
    pub loc: Loc,
    /// The identifier.
    ///
    /// This field is `None` only if an error occurred during parsing.
    pub name: Option<Identifier>,
    /// The list of fields.
    pub fields: Vec<VariableDeclaration>,
}

/// A contract part.
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub enum ContractPart {
    /// A struct definition.
    StructDefinition(Box<StructDefinition>),

    /// An event definition.
    EventDefinition(Box<EventDefinition>),

    /// An enum definition.
    EnumDefinition(Box<EnumDefinition>),

    /// An error definition.
    ErrorDefinition(Box<ErrorDefinition>),

    /// A variable definition.
    VariableDefinition(Box<VariableDefinition>),

    /// A function definition.
    FunctionDefinition(Box<FunctionDefinition>),

    /// A type definition.
    TypeDefinition(Box<TypeDefinition>),

    /// A definition.
    Annotation(Box<Annotation>),

    /// A `using` directive.
    Using(Box<Using>),

    /// A stray semicolon.
    StraySemicolon(Loc),
}

/// A pragma directive
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub enum PragmaDirective {
    /// pragma a b;
    Identifier(Loc, Option<Identifier>, Option<Identifier>),
    /// pragma a "b";
    StringLiteral(Loc, Identifier, StringLiteral),
    /// pragma version =0.5.16;
    Version(Loc, Identifier, Vec<VersionComparator>),
}

/// A `version` list
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub enum VersionComparator {
    /// 0.8.22
    Plain {
        /// The code location.
        loc: Loc,
        /// List of versions: major, minor, patch. minor and patch are optional
        version: Vec<String>,
    },
    /// =0.5.16
    Operator {
        /// The code location.
        loc: Loc,
        /// Semver comparison operator
        op: VersionOp,
        /// version number
        version: Vec<String>,
    },
    /// foo || bar
    Or {
        /// The code location.
        loc: Loc,
        /// left part
        left: Box<VersionComparator>,
        /// right part
        right: Box<VersionComparator>,
    },
    /// 0.7.0 - 0.8.22
    Range {
        /// The code location.
        loc: Loc,
        /// start of range
        from: Vec<String>,
        /// end of range
        to: Vec<String>,
    },
}

/// Comparison operator
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub enum VersionOp {
    /// `=`
    Exact,
    /// `>`
    Greater,
    /// `>=`
    GreaterEq,
    /// `<`
    Less,
    /// `<=`
    LessEq,
    /// `~`
    Tilde,
    /// `^`
    Caret,
    /// `*`
    Wildcard,
}

/// A `using` list. See [Using].
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub enum UsingList {
    /// A single identifier path.
    Library(IdentifierPath),

    /// List of using functions.
    ///
    /// `{ <<identifier path> [ as <operator> ]>,* }`
    Functions(Vec<UsingFunction>),

    /// An error occurred during parsing.
    Error,
}

/// A `using` function. See [UsingList].
///
/// `<path> [ as <oper> ]`
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct UsingFunction {
    /// The code location.
    pub loc: Loc,
    /// The identifier path.
    pub path: IdentifierPath,
    /// The optional user-defined operator.
    pub oper: Option<UserDefinedOperator>,
}

/// A user-defined operator.
///
/// See also the [Solidity blog post][ref] on user-defined operators.
///
/// [ref]: https://blog.soliditylang.org/2023/02/22/user-defined-operators/
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub enum UserDefinedOperator {
    /// `&`
    BitwiseAnd,
    /// `~`
    ///
    BitwiseNot,
    /// `-`
    ///
    /// Note that this is the same as `Subtract`, and that it is currently not being parsed.
    Negate,
    /// `|`
    BitwiseOr,
    /// `^`
    BitwiseXor,
    /// `+`
    Add,
    /// `/`
    Divide,
    /// `%`
    Modulo,
    /// `*`
    Multiply,
    /// `-`
    Subtract,
    /// `==`
    Equal,
    /// `>`
    More,
    /// `>=`
    MoreEqual,
    /// `<`
    Less,
    /// `<=`
    LessEqual,
    /// `!=`
    NotEqual,
}

impl UserDefinedOperator {
    /// Returns the number of arguments needed for this operator's operation.
    #[inline]
    pub const fn args(&self) -> usize {
        match self {
            UserDefinedOperator::BitwiseNot | UserDefinedOperator::Negate => 1,
            _ => 2,
        }
    }

    /// Returns whether `self` is a unary operator.
    #[inline]
    pub const fn is_unary(&self) -> bool {
        matches!(self, Self::BitwiseNot | Self::Negate)
    }

    /// Returns whether `self` is a binary operator.
    #[inline]
    pub const fn is_binary(&self) -> bool {
        !self.is_unary()
    }

    /// Returns whether `self` is a bitwise operator.
    #[inline]
    pub const fn is_bitwise(&self) -> bool {
        matches!(
            self,
            Self::BitwiseAnd | Self::BitwiseOr | Self::BitwiseXor | Self::BitwiseNot
        )
    }

    /// Returns whether `self` is an arithmetic operator.
    #[inline]
    pub const fn is_arithmetic(&self) -> bool {
        matches!(
            self,
            Self::Add | Self::Subtract | Self::Multiply | Self::Divide | Self::Modulo
        )
    }

    /// Returns whether this is a comparison operator.
    #[inline]
    pub const fn is_comparison(&self) -> bool {
        matches!(
            self,
            Self::Equal
                | Self::NotEqual
                | Self::Less
                | Self::LessEqual
                | Self::More
                | Self::MoreEqual
        )
    }
}

/// A `using` directive.
///
/// Can occur within contracts and libraries and at the file level.
///
/// `using <list> for <type | '*'> [global];`
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct Using {
    /// The code location.
    pub loc: Loc,
    /// The list of `using` functions or a single identifier path.
    pub list: UsingList,
    /// The type.
    ///
    /// This field is `None` if an error occurred or the specified type is `*`.
    pub ty: Option<Expression>,
    /// The optional `global` identifier.
    pub global: Option<Identifier>,
}

/// The contract type.
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub enum ContractTy {
    /// `abstract contract`
    Abstract(Loc),

    /// `contract`
    Contract(Loc),

    /// `interface`
    Interface(Loc),

    /// `library`
    Library(Loc),
}

/// A function modifier invocation (see [FunctionAttribute])
/// or a contract inheritance specifier (see [ContractDefinition]).
///
/// Both have the same semantics:
///
/// `<name>[(<args>,*)]`
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct Base {
    /// The code location.
    pub loc: Loc,
    /// The identifier path.
    pub name: IdentifierPath,
    /// The optional arguments.
    pub args: Option<Vec<Expression>>,
}

/// A contract definition.
///
/// `<ty> <name> [<base>,*] { <parts>,* }`
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct ContractDefinition {
    /// The code location.
    pub loc: Loc,
    /// The contract type.
    pub ty: ContractTy,
    /// The identifier.
    ///
    /// This field is `None` only if an error occurred during parsing.
    pub name: Option<Identifier>,
    /// The list of inheritance specifiers.
    pub base: Vec<Base>,
    /// The list of contract parts.
    pub parts: Vec<ContractPart>,
}

/// An event parameter.
///
/// `<ty> [indexed] [name]`
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct EventParameter {
    /// The code location.
    pub loc: Loc,
    /// The type.
    pub ty: Expression,
    /// Whether this parameter is indexed.
    pub indexed: bool,
    /// The optional identifier.
    pub name: Option<Identifier>,
}

/// An event definition.
///
/// `event <name>(<fields>,*) [anonymous];`
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct EventDefinition {
    /// The code location.
    pub loc: Loc,
    /// The identifier.
    ///
    /// This field is `None` only if an error occurred during parsing.
    pub name: Option<Identifier>,
    /// The list of event parameters.
    pub fields: Vec<EventParameter>,
    /// Whether this event is anonymous.
    pub anonymous: bool,
}

/// An error parameter.
///
/// `<ty> [name]`
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct ErrorParameter {
    /// The code location.
    pub loc: Loc,
    /// The type.
    pub ty: Expression,
    /// The optional identifier.
    pub name: Option<Identifier>,
}

/// An error definition.
///
/// `error <name> (<fields>,*);`
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct ErrorDefinition {
    /// The code location.
    pub loc: Loc,
    /// The `error` keyword.
    pub keyword: Expression,
    /// The identifier.
    ///
    /// This field is `None` only if an error occurred during parsing.
    pub name: Option<Identifier>,
    /// The list of error parameters.
    pub fields: Vec<ErrorParameter>,
}

/// An enum definition.
///
/// `enum <name> { <values>,* }`
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct EnumDefinition {
    /// The code location.
    pub loc: Loc,
    /// The identifier.
    ///
    /// This field is `None` only if an error occurred during parsing.
    pub name: Option<Identifier>,
    /// The list of values.
    ///
    /// This field contains `None` only if an error occurred during parsing.
    pub values: Vec<Option<Identifier>>,
}

/// A variable attribute.
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
#[repr(u8)] // for cmp; order of variants is important
pub enum VariableAttribute {
    /// The visibility.
    ///
    /// Only used for storage variables.
    Visibility(Visibility),

    /// `constant`
    Constant(Loc),

    /// `immutable`
    Immutable(Loc),

    /// `ovveride(<1>,*)`
    Override(Loc, Vec<IdentifierPath>),

    /// Storage type.
    StorageType(StorageType),
}

/// Soroban storage types.
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
#[repr(u8)] // for cmp; order of variants is important
pub enum StorageType {
    /// `Temporary`
    Temporary(Option<Loc>),

    /// `persistent`
    Persistent(Option<Loc>),

    /// `Instance`
    Instance(Option<Loc>),
}

/// A variable definition.
///
/// `<ty> <attrs>* <name> [= <initializer>]`
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct VariableDefinition {
    /// The code location.
    pub loc: Loc,
    /// The type.
    pub ty: Expression,
    /// The list of variable attributes.
    pub attrs: Vec<VariableAttribute>,
    /// The identifier.
    ///
    /// This field is `None` only if an error occurred during parsing.
    pub name: Option<Identifier>,
    /// The optional initializer.
    pub initializer: Option<Expression>,
}

/// A user type definition.
///
/// `type <name> is <ty>;`
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct TypeDefinition {
    /// The code location.
    pub loc: Loc,
    /// The user-defined type name.
    pub name: Identifier,
    /// The type expression.
    pub ty: Expression,
}

/// An annotation.
///
/// `@<id>(<value>)`
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct Annotation {
    /// The code location.
    pub loc: Loc,
    /// The identifier.
    pub id: Identifier,
    /// The value.
    pub value: Option<Expression>,
}

/// A string literal.
///
/// `[unicode]"<string>"`
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct StringLiteral {
    /// The code location.
    pub loc: Loc,
    /// Whether this is a unicode string.
    pub unicode: bool,
    /// The string literal.
    ///
    /// Does not contain the quotes or the `unicode` prefix.
    pub string: String,
}

/// A hex literal.
///
/// `hex"<literal>"`
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct HexLiteral {
    /// The code location.
    pub loc: Loc,
    /// The hex literal.
    ///
    /// Contains the `hex` prefix.
    pub hex: String,
}

/// A named argument.
///
/// `<name>: <expr>`
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct NamedArgument {
    /// The code location.
    pub loc: Loc,
    /// The identifier.
    pub name: Identifier,
    /// The value.
    pub expr: Expression,
}

/// An expression.
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub enum Expression {
    /// `<1>++`
    PostIncrement(Loc, Box<Expression>),
    /// `<1>--`
    PostDecrement(Loc, Box<Expression>),
    /// `new <1>`
    New(Loc, Box<Expression>),
    /// `<1>\[ [2] \]`
    ArraySubscript(Loc, Box<Expression>, Option<Box<Expression>>),
    /// `<1>\[ [2] : [3] \]`
    ArraySlice(
        Loc,
        Box<Expression>,
        Option<Box<Expression>>,
        Option<Box<Expression>>,
    ),
    /// `(<1>)`
    Parenthesis(Loc, Box<Expression>),
    /// `<1>.<2>`
    MemberAccess(Loc, Box<Expression>, Identifier),
    /// `<1>(<2>,*)`
    FunctionCall(Loc, Box<Expression>, Vec<Expression>),
    /// `<1><2>` where <2> is a block.
    FunctionCallBlock(Loc, Box<Expression>, Box<Statement>),
    /// `<1>({ <2>,* })`
    NamedFunctionCall(Loc, Box<Expression>, Vec<NamedArgument>),
    /// `!<1>`
    Not(Loc, Box<Expression>),
    /// `~<1>`
    BitwiseNot(Loc, Box<Expression>),
    /// `delete <1>`
    Delete(Loc, Box<Expression>),
    /// `++<1>`
    PreIncrement(Loc, Box<Expression>),
    /// `--<1>`
    PreDecrement(Loc, Box<Expression>),
    /// `+<1>`
    ///
    /// Note that this isn't actually supported by Solidity.
    UnaryPlus(Loc, Box<Expression>),
    /// `-<1>`
    Negate(Loc, Box<Expression>),

    /// `<1> ** <2>`
    Power(Loc, Box<Expression>, Box<Expression>),
    /// `<1> * <2>`
    Multiply(Loc, Box<Expression>, Box<Expression>),
    /// `<1> / <2>`
    Divide(Loc, Box<Expression>, Box<Expression>),
    /// `<1> % <2>`
    Modulo(Loc, Box<Expression>, Box<Expression>),
    /// `<1> + <2>`
    Add(Loc, Box<Expression>, Box<Expression>),
    /// `<1> - <2>`
    Subtract(Loc, Box<Expression>, Box<Expression>),
    /// `<1> << <2>`
    ShiftLeft(Loc, Box<Expression>, Box<Expression>),
    /// `<1> >> <2>`
    ShiftRight(Loc, Box<Expression>, Box<Expression>),
    /// `<1> & <2>`
    BitwiseAnd(Loc, Box<Expression>, Box<Expression>),
    /// `<1> ^ <2>`
    BitwiseXor(Loc, Box<Expression>, Box<Expression>),
    /// `<1> | <2>`
    BitwiseOr(Loc, Box<Expression>, Box<Expression>),
    /// `<1> < <2>`
    Less(Loc, Box<Expression>, Box<Expression>),
    /// `<1> > <2>`
    More(Loc, Box<Expression>, Box<Expression>),
    /// `<1> <= <2>`
    LessEqual(Loc, Box<Expression>, Box<Expression>),
    /// `<1> >= <2>`
    MoreEqual(Loc, Box<Expression>, Box<Expression>),
    /// `<1> == <2>`
    Equal(Loc, Box<Expression>, Box<Expression>),
    /// `<1> != <2>`
    NotEqual(Loc, Box<Expression>, Box<Expression>),
    /// `<1> && <2>`
    And(Loc, Box<Expression>, Box<Expression>),
    /// `<1> || <2>`
    Or(Loc, Box<Expression>, Box<Expression>),
    /// `<1> ? <2> : <3>`
    ///
    /// AKA ternary operator.
    ConditionalOperator(Loc, Box<Expression>, Box<Expression>, Box<Expression>),
    /// `<1> = <2>`
    Assign(Loc, Box<Expression>, Box<Expression>),
    /// `<1> |= <2>`
    AssignOr(Loc, Box<Expression>, Box<Expression>),
    /// `<1> &= <2>`
    AssignAnd(Loc, Box<Expression>, Box<Expression>),
    /// `<1> ^= <2>`
    AssignXor(Loc, Box<Expression>, Box<Expression>),
    /// `<1> <<= <2>`
    AssignShiftLeft(Loc, Box<Expression>, Box<Expression>),
    /// `<1> >>= <2>`
    AssignShiftRight(Loc, Box<Expression>, Box<Expression>),
    /// `<1> += <2>`
    AssignAdd(Loc, Box<Expression>, Box<Expression>),
    /// `<1> -= <2>`
    AssignSubtract(Loc, Box<Expression>, Box<Expression>),
    /// `<1> *= <2>`
    AssignMultiply(Loc, Box<Expression>, Box<Expression>),
    /// `<1> /= <2>`
    AssignDivide(Loc, Box<Expression>, Box<Expression>),
    /// `<1> %= <2>`
    AssignModulo(Loc, Box<Expression>, Box<Expression>),

    /// `true` or `false`
    BoolLiteral(Loc, bool),
    /// ``
    NumberLiteral(Loc, String, String, Option<Identifier>),
    /// ``
    RationalNumberLiteral(Loc, String, String, String, Option<Identifier>),
    /// ``
    HexNumberLiteral(Loc, String, Option<Identifier>),
    /// `<1>+`. See [StringLiteral].
    StringLiteral(Vec<StringLiteral>),
    /// See [Type].
    Type(Loc, Type),
    /// `<1>+`. See [HexLiteral].
    HexLiteral(Vec<HexLiteral>),
    /// `0x[a-fA-F0-9]{40}`
    ///
    /// This [should be correctly checksummed][ref], but it currently isn't being enforced in the parser.
    ///
    /// [ref]: https://docs.soliditylang.org/en/latest/types.html#address-literals
    AddressLiteral(Loc, String),
    /// Any valid [Identifier].
    Variable(Identifier),
    /// `(<1>,*)`
    List(Loc, ParameterList),
    /// `\[ <1>.* \]`
    ArrayLiteral(Loc, Vec<Expression>),
}

/// See `Expression::components`.
macro_rules! expr_components {
    ($s:ident) => {
        match $s {
            // (Some, None)
            PostDecrement(_, expr) | PostIncrement(_, expr) => (Some(expr), None),

            // (None, Some)
            Not(_, expr)
            | BitwiseNot(_, expr)
            | New(_, expr)
            | Delete(_, expr)
            | UnaryPlus(_, expr)
            | Negate(_, expr)
            | PreDecrement(_, expr)
            | Parenthesis(_, expr)
            | PreIncrement(_, expr) => (None, Some(expr)),

            // (Some, Some)
            Power(_, left, right)
            | Multiply(_, left, right)
            | Divide(_, left, right)
            | Modulo(_, left, right)
            | Add(_, left, right)
            | Subtract(_, left, right)
            | ShiftLeft(_, left, right)
            | ShiftRight(_, left, right)
            | BitwiseAnd(_, left, right)
            | BitwiseXor(_, left, right)
            | BitwiseOr(_, left, right)
            | Less(_, left, right)
            | More(_, left, right)
            | LessEqual(_, left, right)
            | MoreEqual(_, left, right)
            | Equal(_, left, right)
            | NotEqual(_, left, right)
            | And(_, left, right)
            | Or(_, left, right)
            | Assign(_, left, right)
            | AssignOr(_, left, right)
            | AssignAnd(_, left, right)
            | AssignXor(_, left, right)
            | AssignShiftLeft(_, left, right)
            | AssignShiftRight(_, left, right)
            | AssignAdd(_, left, right)
            | AssignSubtract(_, left, right)
            | AssignMultiply(_, left, right)
            | AssignDivide(_, left, right)
            | AssignModulo(_, left, right) => (Some(left), Some(right)),

            // (None, None)
            MemberAccess(..)
            | ConditionalOperator(..)
            | ArraySubscript(..)
            | ArraySlice(..)
            | FunctionCall(..)
            | FunctionCallBlock(..)
            | NamedFunctionCall(..)
            | BoolLiteral(..)
            | NumberLiteral(..)
            | RationalNumberLiteral(..)
            | HexNumberLiteral(..)
            | StringLiteral(..)
            | Type(..)
            | HexLiteral(..)
            | AddressLiteral(..)
            | Variable(..)
            | List(..)
            | ArrayLiteral(..) => (None, None),
        }
    };
}

impl Expression {
    /// Removes one layer of parentheses.
    #[inline]
    pub fn remove_parenthesis(&self) -> &Expression {
        if let Expression::Parenthesis(_, expr) = self {
            expr
        } else {
            self
        }
    }

    /// Strips all parentheses recursively.
    pub fn strip_parentheses(&self) -> &Expression {
        match self {
            Expression::Parenthesis(_, expr) => expr.strip_parentheses(),
            _ => self,
        }
    }

    /// Returns shared references to the components of this expression.
    ///
    /// `(left_component, right_component)`
    ///
    /// # Examples
    ///
    /// ```
    /// use solang_parser::pt::{Expression, Identifier, Loc};
    ///
    /// // `a++`
    /// let var = Expression::Variable(Identifier::new("a"));
    /// let post_increment = Expression::PostIncrement(Loc::default(), Box::new(var.clone()));
    /// assert_eq!(post_increment.components(), (Some(&var), None));
    ///
    /// // `++a`
    /// let var = Expression::Variable(Identifier::new("a"));
    /// let pre_increment = Expression::PreIncrement(Loc::default(), Box::new(var.clone()));
    /// assert_eq!(pre_increment.components(), (None, Some(&var)));
    ///
    /// // `a + b`
    /// let var_a = Expression::Variable(Identifier::new("a"));
    /// let var_b = Expression::Variable(Identifier::new("b"));
    /// let pre_increment = Expression::Add(Loc::default(), Box::new(var_a.clone()), Box::new(var_b.clone()));
    /// assert_eq!(pre_increment.components(), (Some(&var_a), Some(&var_b)));
    /// ```
    #[inline]
    pub fn components(&self) -> (Option<&Self>, Option<&Self>) {
        use Expression::*;
        expr_components!(self)
    }

    /// Returns mutable references to the components of this expression.
    ///
    /// See also [`Expression::components`].
    #[inline]
    pub fn components_mut(&mut self) -> (Option<&mut Self>, Option<&mut Self>) {
        use Expression::*;
        expr_components!(self)
    }

    /// Returns whether this expression can be split across multiple lines.
    #[inline]
    pub const fn is_unsplittable(&self) -> bool {
        use Expression::*;
        matches!(
            self,
            BoolLiteral(..)
                | NumberLiteral(..)
                | RationalNumberLiteral(..)
                | HexNumberLiteral(..)
                | StringLiteral(..)
                | HexLiteral(..)
                | AddressLiteral(..)
                | Variable(..)
        )
    }

    /// Returns whether this expression has spaces around it.
    #[inline]
    pub const fn has_space_around(&self) -> bool {
        use Expression::*;
        !matches!(
            self,
            PostIncrement(..)
                | PreIncrement(..)
                | PostDecrement(..)
                | PreDecrement(..)
                | Not(..)
                | BitwiseNot(..)
                | UnaryPlus(..)
                | Negate(..)
        )
    }

    /// Returns if the expression is a literal
    pub fn is_literal(&self) -> bool {
        matches!(
            self,
            Expression::AddressLiteral(..)
                | Expression::HexLiteral(..)
                | Expression::BoolLiteral(..)
                | Expression::NumberLiteral(..)
                | Expression::ArrayLiteral(..)
                | Expression::HexNumberLiteral(..)
                | Expression::RationalNumberLiteral(..)
                | Expression::StringLiteral(..)
        )
    }
}

/// A parameter.
///
/// `<ty> [storage] <name>`
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct Parameter {
    /// The code location.
    pub loc: Loc,
    /// An optional annotation '@annotation'.
    pub annotation: Option<Annotation>,
    /// The type.
    pub ty: Expression,
    /// The optional memory location.
    pub storage: Option<StorageLocation>,
    /// The optional identifier.
    pub name: Option<Identifier>,
}

/// Function mutability.
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub enum Mutability {
    /// `pure`
    Pure(Loc),

    /// `view`
    View(Loc),

    /// `constant`
    Constant(Loc),

    /// `payable`
    Payable(Loc),
}

/// Function visibility.
///
/// Deprecated for [FunctionTy] other than `Function`.
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
#[repr(u8)] // for cmp; order of variants is important
pub enum Visibility {
    /// `external`
    External(Option<Loc>),

    /// `public`
    Public(Option<Loc>),

    /// `internal`
    Internal(Option<Loc>),

    /// `private`
    Private(Option<Loc>),
}

/// A function attribute.
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
#[repr(u8)] // for cmp; order of variants is important
pub enum FunctionAttribute {
    /// Visibility attribute.
    Visibility(Visibility),

    /// Mutability attribute.
    Mutability(Mutability),

    /// `virtual`
    Virtual(Loc),

    /// `immutable`
    Immutable(Loc),

    /// `override[(<identifier path>,*)]`
    Override(Loc, Vec<IdentifierPath>),

    /// A modifier or constructor invocation.
    BaseOrModifier(Loc, Base),

    /// An error occurred during parsing.
    Error(Loc),
}

/// A function's type.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub enum FunctionTy {
    /// `constructor`
    Constructor,

    /// `function`
    Function,

    /// `fallback`
    Fallback,

    /// `receive`
    Receive,

    /// `modifier`
    Modifier,
}

/// A function definition.
///
/// `<ty> [name](<params>,*) [attributes] [returns] [body]`
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct FunctionDefinition {
    /// The function prototype location.
    pub loc_prototype: Loc,
    /// The code location.
    pub loc: Loc,
    /// The function type.
    pub ty: FunctionTy,
    /// The optional identifier.
    ///
    /// This can be `None` for old style fallback functions.
    pub name: Option<Identifier>,
    /// The identifier's code location.
    pub name_loc: Loc,
    /// The parameter list.
    pub params: ParameterList,
    /// The function attributes.
    pub attributes: Vec<FunctionAttribute>,
    /// The `returns` keyword's location. `Some` if this was `return`, not `returns`.
    pub return_not_returns: Option<Loc>,
    /// The return parameter list.
    pub returns: ParameterList,
    /// The function body.
    ///
    /// If `None`, the declaration ended with a semicolon.
    pub body: Option<Statement>,
}

impl FunctionDefinition {
    /// Returns `true` if the function has no return parameters.
    #[inline]
    pub fn is_void(&self) -> bool {
        self.returns.is_empty()
    }

    /// Returns `true` if the function body is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.body.as_ref().map_or(true, Statement::is_empty)
    }

    /// Sorts the function attributes.
    #[inline]
    pub fn sort_attributes(&mut self) {
        // we don't use unstable sort since there may be more that one `BaseOrModifier` attributes
        // which we want to preserve the order of
        self.attributes.sort();
    }
}

/// A statement.
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
#[allow(clippy::large_enum_variant, clippy::type_complexity)]
pub enum Statement {
    /// `[unchecked] { <statements>* }`
    Block {
        /// The code location.
        loc: Loc,
        /// Whether this block is `unchecked`.
        unchecked: bool,
        /// The statements.
        statements: Vec<Statement>,
    },
    /// `assembly [dialect] [(<flags>,*)] <block>`
    Assembly {
        /// The code location.
        loc: Loc,
        /// The assembly dialect.
        dialect: Option<StringLiteral>,
        /// The assembly flags.
        flags: Option<Vec<StringLiteral>>,
        /// The assembly block.
        block: YulBlock,
    },
    /// `{ <1>,* }`
    Args(Loc, Vec<NamedArgument>),
    /// `if ({1}) <2> [else <3>]`
    ///
    /// Note that the `<1>` expression does not contain the parentheses.
    If(Loc, Expression, Box<Statement>, Option<Box<Statement>>),
    /// `while ({1}) <2>`
    ///
    /// Note that the `<1>` expression does not contain the parentheses.
    While(Loc, Expression, Box<Statement>),
    /// An [Expression].
    Expression(Loc, Expression),
    /// `<1> [= <2>];`
    VariableDefinition(Loc, VariableDeclaration, Option<Expression>),
    /// `for ([1]; [2]; [3]) [4]`
    ///
    /// The `[4]` block statement is `None` when the `for` statement ends with a semicolon.
    For(
        Loc,
        Option<Box<Statement>>,
        Option<Box<Expression>>,
        Option<Box<Expression>>,
        Option<Box<Statement>>,
    ),
    /// `do <1> while ({2});`
    ///
    /// Note that the `<2>` expression does not contain the parentheses.
    DoWhile(Loc, Box<Statement>, Expression),
    /// `continue;`
    Continue(Loc),
    /// `break;`
    Break(Loc),
    /// `return [1];`
    Return(Loc, Option<Expression>),
    /// `revert [1] (<2>,*);`
    Revert(Loc, Option<IdentifierPath>, Vec<Expression>),
    /// `revert [1] ({ <2>,* });`
    RevertNamedArgs(Loc, Option<IdentifierPath>, Vec<NamedArgument>),
    /// `emit <1>;`
    ///
    /// `<1>` is `FunctionCall`.
    Emit(Loc, Expression),
    /// `try <1> [returns (<2.1>,*) <2.2>] <3>*`
    ///
    /// `<1>` is either `New(FunctionCall)` or `FunctionCall`.
    Try(
        Loc,
        Expression,
        Option<(ParameterList, Box<Statement>)>,
        Vec<CatchClause>,
    ),
    /// An error occurred during parsing.
    Error(Loc),
}

impl Statement {
    /// Returns `true` if the block statement contains no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Block { statements, .. } => statements.is_empty(),
            Self::Assembly { block, .. } => block.is_empty(),
            Self::Args(_, args) => args.is_empty(),
            _ => false,
        }
    }
}

/// A catch clause. See [Statement].
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub enum CatchClause {
    /// `catch [(<1>)] <2>`
    Simple(Loc, Option<Parameter>, Statement),

    /// `catch <1> (<2>) <3>`
    Named(Loc, Identifier, Parameter, Statement),
}

/// A Yul statement.
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub enum YulStatement {
    /// `<1>,+ = <2>`
    Assign(Loc, Vec<YulExpression>, YulExpression),
    /// `let <1>,+ [:= <2>]`
    VariableDeclaration(Loc, Vec<YulTypedIdentifier>, Option<YulExpression>),
    /// `if <1> <2>`
    If(Loc, YulExpression, YulBlock),
    /// A [YulFor] statement.
    For(YulFor),
    /// A [YulSwitch] statement.
    Switch(YulSwitch),
    /// `leave`
    Leave(Loc),
    /// `break`
    Break(Loc),
    /// `continue`
    Continue(Loc),
    /// A [YulBlock] statement.
    Block(YulBlock),
    /// A [YulFunctionDefinition] statement.
    FunctionDefinition(Box<YulFunctionDefinition>),
    /// A [YulFunctionCall] statement.
    FunctionCall(Box<YulFunctionCall>),
    /// An error occurred during parsing.
    Error(Loc),
}

/// A Yul switch statement.
///
/// `switch <condition> <cases>* [default <default>]`
///
/// Enforced by the parser:
///
/// - `cases` is guaranteed to be a `Vec` of `YulSwitchOptions::Case`.
/// - `default` is guaranteed to be `YulSwitchOptions::Default`.
/// - At least one of `cases` or `default` must be non-empty/`Some` respectively.
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct YulSwitch {
    /// The code location.
    pub loc: Loc,
    /// The switch condition.
    pub condition: YulExpression,
    /// The switch cases.
    pub cases: Vec<YulSwitchOptions>,
    /// The optional default case.
    pub default: Option<YulSwitchOptions>,
}

/// A Yul for statement.
///
/// `for <init_block> <condition> <post_block> <execution_block>`
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct YulFor {
    /// The code location.
    pub loc: Loc,
    /// The for statement init block.
    pub init_block: YulBlock,
    /// The for statement condition.
    pub condition: YulExpression,
    /// The for statement post block.
    pub post_block: YulBlock,
    /// The for statement execution block.
    pub execution_block: YulBlock,
}

/// A Yul block statement.
///
/// `{ <statements>* }`
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct YulBlock {
    /// The code location.
    pub loc: Loc,
    /// The block statements.
    pub statements: Vec<YulStatement>,
}

impl YulBlock {
    /// Returns `true` if the block contains no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.statements.is_empty()
    }
}

/// A Yul expression.
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub enum YulExpression {
    /// `<1> [: <2>]`
    BoolLiteral(Loc, bool, Option<Identifier>),
    /// `<1>[e<2>] [: <2>]`
    NumberLiteral(Loc, String, String, Option<Identifier>),
    /// `<1> [: <2>]`
    HexNumberLiteral(Loc, String, Option<Identifier>),
    /// `<0> [: <1>]`
    HexStringLiteral(HexLiteral, Option<Identifier>),
    /// `<0> [: <1>]`
    StringLiteral(StringLiteral, Option<Identifier>),
    /// Any valid [Identifier].
    Variable(Identifier),
    /// [YulFunctionCall].
    FunctionCall(Box<YulFunctionCall>),
    /// `<1>.<2>`
    SuffixAccess(Loc, Box<YulExpression>, Identifier),
}

/// A Yul typed identifier.
///
/// `<id> [: <ty>]`
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct YulTypedIdentifier {
    /// The code location.
    pub loc: Loc,
    /// The identifier.
    pub id: Identifier,
    /// The optional type.
    pub ty: Option<Identifier>,
}

/// A Yul function definition.
///
/// `function <name> (<params>,*) [-> (<returns>,*)] <body>`
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct YulFunctionDefinition {
    /// The code location.
    pub loc: Loc,
    /// The identifier.
    pub id: Identifier,
    /// The parameters.
    pub params: Vec<YulTypedIdentifier>,
    /// The return parameters.
    pub returns: Vec<YulTypedIdentifier>,
    /// The function body.
    pub body: YulBlock,
}

impl YulFunctionDefinition {
    /// Returns `true` if the function has no return parameters.
    #[inline]
    pub fn is_void(&self) -> bool {
        self.returns.is_empty()
    }

    /// Returns `true` if the function body is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.body.is_empty()
    }
}

/// A Yul function call.
///
/// `<id>(<arguments>,*)`
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub struct YulFunctionCall {
    /// The code location.
    pub loc: Loc,
    /// The identifier.
    pub id: Identifier,
    /// The function call arguments.
    pub arguments: Vec<YulExpression>,
}

/// A Yul switch case or default statement. See [YulSwitch].
#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "pt-serde", derive(Serialize, Deserialize))]
pub enum YulSwitchOptions {
    /// `case <1> <2>`
    Case(Loc, YulExpression, YulBlock),
    /// `default <1>`
    Default(Loc, YulBlock),
}
