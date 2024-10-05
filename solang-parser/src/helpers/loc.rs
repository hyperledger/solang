// SPDX-License-Identifier: Apache-2.0

use crate::lexer::LexicalError;
use crate::pt::{self, Loc};
use std::sync::Arc;
use std::{borrow::Cow, rc::Rc};

/// Returns the optional code location.
pub trait OptionalCodeLocation {
    /// Returns the optional code location of `self`.
    fn loc_opt(&self) -> Option<Loc>;
}

impl<T: CodeLocation> OptionalCodeLocation for Option<T> {
    fn loc_opt(&self) -> Option<Loc> {
        self.as_ref().map(CodeLocation::loc)
    }
}

impl OptionalCodeLocation for pt::Visibility {
    fn loc_opt(&self) -> Option<Loc> {
        match self {
            Self::Internal(l, ..)
            | Self::External(l, ..)
            | Self::Private(l, ..)
            | Self::Public(l, ..) => *l,
        }
    }
}

impl OptionalCodeLocation for pt::StorageType {
    fn loc_opt(&self) -> Option<Loc> {
        match self {
            Self::Persistent(l) | Self::Temporary(l) | Self::Instance(l) => *l,
        }
    }
}

impl OptionalCodeLocation for pt::SourceUnit {
    #[inline]
    fn loc_opt(&self) -> Option<Loc> {
        self.0.loc_opt()
    }
}

impl<T: CodeLocation> OptionalCodeLocation for [T] {
    // TODO: Merge first with last span?
    fn loc_opt(&self) -> Option<Loc> {
        self.first().map(CodeLocation::loc)
    }
}

impl<T: CodeLocation> OptionalCodeLocation for Vec<T> {
    fn loc_opt(&self) -> Option<Loc> {
        (**self).loc_opt()
    }
}

impl<'a, T: ?Sized + OptionalCodeLocation> OptionalCodeLocation for &'a T {
    fn loc_opt(&self) -> Option<Loc> {
        (**self).loc_opt()
    }
}

impl<'a, T: ?Sized + OptionalCodeLocation> OptionalCodeLocation for &'a mut T {
    fn loc_opt(&self) -> Option<Loc> {
        (**self).loc_opt()
    }
}

impl<'a, T: ?Sized + ToOwned + OptionalCodeLocation> OptionalCodeLocation for Cow<'a, T> {
    fn loc_opt(&self) -> Option<Loc> {
        (**self).loc_opt()
    }
}

impl<T: ?Sized + OptionalCodeLocation> OptionalCodeLocation for Box<T> {
    fn loc_opt(&self) -> Option<Loc> {
        (**self).loc_opt()
    }
}

impl<T: ?Sized + OptionalCodeLocation> OptionalCodeLocation for Rc<T> {
    fn loc_opt(&self) -> Option<Loc> {
        (**self).loc_opt()
    }
}

impl<T: ?Sized + OptionalCodeLocation> OptionalCodeLocation for Arc<T> {
    fn loc_opt(&self) -> Option<Loc> {
        (**self).loc_opt()
    }
}

// would be: `impl<T: CodeLocation> OptionalCodeLocation for T { ... }`
// but then we wouldn't have the correct implementation for `Box<T>` and the other smart pointers
macro_rules! impl_optional_for_pt {
    ($($t:ty),+ $(,)?) => {
        $(
            impl OptionalCodeLocation for $t {
                #[inline]
                fn loc_opt(&self) -> Option<Loc> {
                    Some(<$t as CodeLocation>::loc(self))
                }
            }
        )+
    };
}

impl_optional_for_pt!(
    // structs
    pt::Annotation,
    pt::Base,
    pt::ContractDefinition,
    pt::EnumDefinition,
    pt::ErrorDefinition,
    pt::ErrorParameter,
    pt::EventDefinition,
    pt::EventParameter,
    pt::FunctionDefinition,
    pt::HexLiteral,
    pt::Identifier,
    pt::IdentifierPath,
    pt::NamedArgument,
    pt::Parameter,
    pt::StringLiteral,
    pt::StructDefinition,
    pt::TypeDefinition,
    pt::Using,
    pt::UsingFunction,
    pt::VariableDeclaration,
    pt::VariableDefinition,
    pt::YulBlock,
    pt::YulFor,
    pt::YulFunctionCall,
    pt::YulFunctionDefinition,
    pt::YulSwitch,
    pt::YulTypedIdentifier,
    // enums
    pt::CatchClause,
    pt::Comment,
    pt::ContractPart,
    pt::ContractTy,
    pt::Expression,
    pt::FunctionAttribute,
    pt::Import,
    pt::Loc,
    pt::Mutability,
    pt::SourceUnitPart,
    pt::Statement,
    pt::StorageLocation,
    pt::UsingList,
    pt::VariableAttribute,
    pt::YulExpression,
    pt::YulStatement,
    pt::YulSwitchOptions,
    // other
    LexicalError,
);

/// Returns the code location.
pub trait CodeLocation {
    /// Returns the code location of `self`.
    fn loc(&self) -> Loc;
}

impl CodeLocation for Loc {
    #[inline]
    fn loc(&self) -> Loc {
        *self
    }
}

impl<'a, T: ?Sized + CodeLocation> CodeLocation for &'a T {
    fn loc(&self) -> Loc {
        (**self).loc()
    }
}

impl<'a, T: ?Sized + CodeLocation> CodeLocation for &'a mut T {
    fn loc(&self) -> Loc {
        (**self).loc()
    }
}

impl<'a, T: ?Sized + ToOwned + CodeLocation> CodeLocation for Cow<'a, T> {
    fn loc(&self) -> Loc {
        (**self).loc()
    }
}

impl<T: ?Sized + CodeLocation> CodeLocation for Box<T> {
    fn loc(&self) -> Loc {
        (**self).loc()
    }
}

impl<T: ?Sized + CodeLocation> CodeLocation for Rc<T> {
    fn loc(&self) -> Loc {
        (**self).loc()
    }
}

impl<T: ?Sized + CodeLocation> CodeLocation for Arc<T> {
    fn loc(&self) -> Loc {
        (**self).loc()
    }
}

macro_rules! impl_for_structs {
    ($($t:ty),+ $(,)?) => {
        $(
            impl CodeLocation for $t {
                #[inline]
                fn loc(&self) -> Loc {
                    self.loc
                }
            }
        )+
    };
}

// all structs except for SourceUnit
impl_for_structs!(
    pt::Annotation,
    pt::Base,
    pt::ContractDefinition,
    pt::EnumDefinition,
    pt::ErrorDefinition,
    pt::ErrorParameter,
    pt::EventDefinition,
    pt::EventParameter,
    pt::FunctionDefinition,
    pt::HexLiteral,
    pt::Identifier,
    pt::IdentifierPath,
    pt::NamedArgument,
    pt::Parameter,
    pt::StringLiteral,
    pt::StructDefinition,
    pt::TypeDefinition,
    pt::Using,
    pt::UsingFunction,
    pt::VariableDeclaration,
    pt::VariableDefinition,
    pt::YulBlock,
    pt::YulFor,
    pt::YulFunctionCall,
    pt::YulFunctionDefinition,
    pt::YulSwitch,
    pt::YulTypedIdentifier,
);

macro_rules! impl_for_enums {
    ($(
        $t:ty: match $s:ident {
            $($m:tt)*
        }
    )+) => {
        $(
            impl CodeLocation for $t {
                fn loc(&$s) -> Loc {
                    match *$s {
                        $($m)*
                    }
                }
            }
        )+
    };
}

// all enums except for Type, UserDefinedOperator and FunctionTy
impl_for_enums! {
    pt::CatchClause: match self {
        Self::Simple(l, ..)
        | Self::Named(l, ..) => l,
    }

    pt::Comment: match self {
        Self::Line(l, ..)
        | Self::Block(l, ..)
        | Self::DocLine(l, ..)
        | Self::DocBlock(l, ..) => l,
    }

    pt::ContractPart: match self {
        Self::StructDefinition(ref l, ..) => l.loc(),
        Self::EventDefinition(ref l, ..) => l.loc(),
        Self::EnumDefinition(ref l, ..) => l.loc(),
        Self::ErrorDefinition(ref l, ..) => l.loc(),
        Self::VariableDefinition(ref l, ..) => l.loc(),
        Self::FunctionDefinition(ref l, ..) => l.loc(),
        Self::TypeDefinition(ref l, ..) => l.loc(),
        Self::Annotation(ref l, ..) => l.loc(),
        Self::Using(ref l, ..) => l.loc(),
        Self::StraySemicolon(l, ..) => l,
    }

    pt::ContractTy: match self {
        Self::Abstract(l, ..)
        | Self::Contract(l, ..)
        | Self::Library(l, ..)
        | Self::Interface(l, ..) => l,
    }

    pt::Expression: match self {
        // literals have at least one item
        Self::StringLiteral(ref l, ..) => l.loc_opt().unwrap(),
        Self::HexLiteral(ref l, ..) => l.loc_opt().unwrap(),
        Self::Variable(ref l, ..) => l.loc(),
        Self::PostIncrement(l, ..)
        | Self::PostDecrement(l, ..)
        | Self::New(l, ..)
        | Self::Parenthesis(l, ..)
        | Self::ArraySubscript(l, ..)
        | Self::ArraySlice(l, ..)
        | Self::MemberAccess(l, ..)
        | Self::FunctionCall(l, ..)
        | Self::FunctionCallBlock(l, ..)
        | Self::NamedFunctionCall(l, ..)
        | Self::Not(l, ..)
        | Self::BitwiseNot(l, ..)
        | Self::Delete(l, ..)
        | Self::PreIncrement(l, ..)
        | Self::PreDecrement(l, ..)
        | Self::UnaryPlus(l, ..)
        | Self::Negate(l, ..)
        | Self::Power(l, ..)
        | Self::Multiply(l, ..)
        | Self::Divide(l, ..)
        | Self::Modulo(l, ..)
        | Self::Add(l, ..)
        | Self::Subtract(l, ..)
        | Self::ShiftLeft(l, ..)
        | Self::ShiftRight(l, ..)
        | Self::BitwiseAnd(l, ..)
        | Self::BitwiseXor(l, ..)
        | Self::BitwiseOr(l, ..)
        | Self::Less(l, ..)
        | Self::More(l, ..)
        | Self::LessEqual(l, ..)
        | Self::MoreEqual(l, ..)
        | Self::Equal(l, ..)
        | Self::NotEqual(l, ..)
        | Self::And(l, ..)
        | Self::Or(l, ..)
        | Self::ConditionalOperator(l, ..)
        | Self::Assign(l, ..)
        | Self::AssignOr(l, ..)
        | Self::AssignAnd(l, ..)
        | Self::AssignXor(l, ..)
        | Self::AssignShiftLeft(l, ..)
        | Self::AssignShiftRight(l, ..)
        | Self::AssignAdd(l, ..)
        | Self::AssignSubtract(l, ..)
        | Self::AssignMultiply(l, ..)
        | Self::AssignDivide(l, ..)
        | Self::AssignModulo(l, ..)
        | Self::BoolLiteral(l, ..)
        | Self::NumberLiteral(l, ..)
        | Self::RationalNumberLiteral(l, ..)
        | Self::HexNumberLiteral(l, ..)
        | Self::ArrayLiteral(l, ..)
        | Self::List(l, ..)
        | Self::Type(l, ..)
        | Self::AddressLiteral(l, ..) => l,
    }

    pt::FunctionAttribute: match self {
        Self::Mutability(ref l) => l.loc(),
        Self::Visibility(ref l) => l.loc_opt().unwrap_or_default(),
        Self::Virtual(l, ..)
        | Self::Immutable(l, ..)
        | Self::Override(l, ..,)
        | Self::BaseOrModifier(l, ..)
        | Self::Error(l, ..) => l,
    }

    pt::Import: match self {
        Self::GlobalSymbol(.., l)
        | Self::Plain(.., l)
        | Self::Rename(.., l) => l,
    }

    pt::Mutability: match self {
        Self::Constant(l, ..)
        | Self::Payable(l, ..)
        | Self::Pure(l, ..)
        | Self::View(l, ..) => l,
    }

    pt::SourceUnitPart: match self {
        Self::ImportDirective(ref l, ..) => l.loc(),
        Self::ContractDefinition(ref l, ..) => l.loc(),
        Self::EnumDefinition(ref l, ..) => l.loc(),
        Self::StructDefinition(ref l, ..) => l.loc(),
        Self::EventDefinition(ref l, ..) => l.loc(),
        Self::ErrorDefinition(ref l, ..) => l.loc(),
        Self::FunctionDefinition(ref l, ..) => l.loc(),
        Self::VariableDefinition(ref l, ..) => l.loc(),
        Self::TypeDefinition(ref l, ..) => l.loc(),
        Self::Annotation(ref l, ..) => l.loc(),
        Self::Using(ref l, ..) => l.loc(),
        Self::PragmaDirective(ref l, ..) => l.loc(),
        Self::StraySemicolon(l, ..) => l,
    }

    pt::Statement: match self {
        Self::Block { loc: l, .. }
        | Self::Assembly { loc: l, .. }
        | Self::Args(l, ..)
        | Self::If(l, ..)
        | Self::While(l, ..)
        | Self::Expression(l, ..)
        | Self::VariableDefinition(l, ..)
        | Self::For(l, ..)
        | Self::DoWhile(l, ..)
        | Self::Continue(l, ..)
        | Self::Break(l, ..)
        | Self::Return(l, ..)
        | Self::Revert(l, ..)
        | Self::RevertNamedArgs(l, ..)
        | Self::Emit(l, ..)
        | Self::Try(l, ..)
        | Self::Error(l, ..) => l,
    }

    pt::StorageLocation: match self {
        Self::Calldata(l, ..)
        | Self::Memory(l, ..)
        | Self::Storage(l, ..) => l,
    }

    pt::UsingList: match self {
        Self::Library(ref l, ..) => l.loc(),
        Self::Functions(ref l, ..) => l.loc_opt().unwrap_or_default(),
        Self::Error => panic!("an error occurred"),
    }

    pt::VariableAttribute: match self {
        Self::Visibility(ref l, ..) => l.loc_opt().unwrap_or_default(),
        Self::StorageType(ref l, ..) => l.loc_opt().unwrap_or_default(),
        Self::Constant(l, ..)
        | Self::Immutable(l, ..)
        | Self::Override(l, ..) => l,
    }

    pt::YulExpression: match self {
        Self::HexStringLiteral(ref l, ..) => l.loc(),
        Self::StringLiteral(ref l, ..) => l.loc(),
        Self::Variable(ref l, ..) => l.loc(),
        Self::FunctionCall(ref l, ..) => l.loc(),
        Self::BoolLiteral(l, ..)
        | Self::NumberLiteral(l, ..)
        | Self::HexNumberLiteral(l, ..)
        | Self::SuffixAccess(l, ..) => l,
    }

    pt::YulStatement: match self {
        Self::Block(ref l, ..) => l.loc(),
        Self::FunctionDefinition(ref l, ..) => l.loc(),
        Self::FunctionCall(ref l, ..) => l.loc(),
        Self::For(ref l, ..) => l.loc(),
        Self::Switch(ref l, ..) => l.loc(),
        Self::Assign(l, ..)
        | Self::VariableDeclaration(l, ..)
        | Self::If(l, ..)
        | Self::Leave(l, ..)
        | Self::Break(l, ..)
        | Self::Continue(l, ..)
        | Self::Error(l, ..) => l,
    }

    pt::YulSwitchOptions: match self {
        Self::Case(l, ..)
        | Self::Default(l, ..) => l,
    }

    pt::PragmaDirective: match self {
        Self::Identifier(l, ..)
        | Self::StringLiteral(l, ..)
        | Self::Version(l, ..) => l,
    }

    // other
    LexicalError: match self {
        Self::EndOfFileInComment(l)
        | Self::EndOfFileInString(l)
        | Self::EndofFileInHex(l)
        | Self::MissingNumber(l)
        | Self::InvalidCharacterInHexLiteral(l, _)
        | Self::UnrecognisedToken(l, _)
        | Self::ExpectedFrom(l, _)
        | Self::MissingExponent(l) => l,
    }
}
