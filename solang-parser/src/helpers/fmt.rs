// SPDX-License-Identifier: Apache-2.0

//! Implements `Display` for all parse tree data types, following the [Solidity style guide][ref].
//!
//! [ref]: https://docs.soliditylang.org/en/latest/style-guide.html

use crate::pt::{self, StorageType};
use std::{
    borrow::Cow,
    fmt::{Display, Formatter, Result, Write},
};

macro_rules! write_opt {
    // no sep
    ($f:expr, $opt:expr $(,)?) => {
        if let Some(t) = $opt {
            Display::fmt(t, $f)?;
        }
    };

    // sep before
    ($f:expr, $sep:literal, $opt:expr $(,)?) => {
        if let Some(t) = $opt {
            Display::fmt(&$sep, $f)?;
            Display::fmt(t, $f)?;
        }
    };

    // sep after
    ($f:expr, $opt:expr, $sep:literal $(,)?) => {
        if let Some(t) = $opt {
            Display::fmt(t, $f)?;
            Display::fmt(&$sep, $f)?;
        }
    };

    // both
    ($f:expr, $sep1:literal, $opt:expr, $sep2:literal $(,)?) => {
        if let Some(t) = $opt {
            Display::fmt(&$sep1, $f)?;
            Display::fmt(t, $f)?;
            Display::fmt(&$sep2, $f)?;
        }
    };
}

// structs
impl Display for pt::Annotation {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_char('@')?;
        self.id.fmt(f)?;
        if let Some(value) = &self.value {
            f.write_char('(')?;
            value.fmt(f)?;
            f.write_char(')')?;
        }

        Ok(())
    }
}

impl Display for pt::Base {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        self.name.fmt(f)?;
        if let Some(args) = &self.args {
            f.write_char('(')?;
            write_separated(args, f, ", ")?;
            f.write_char(')')?;
        }
        Ok(())
    }
}

impl Display for pt::ContractDefinition {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        self.ty.fmt(f)?;
        f.write_char(' ')?;

        write_opt!(f, &self.name, ' ');

        if !self.base.is_empty() {
            write_separated(&self.base, f, " ")?;
            f.write_char(' ')?;
        }

        f.write_char('{')?;
        write_separated(&self.parts, f, " ")?;
        f.write_char('}')
    }
}

impl Display for pt::EnumDefinition {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str("enum ")?;
        write_opt!(f, &self.name, ' ');

        f.write_char('{')?;
        write_separated_iter(self.values.iter().flatten(), f, ", ")?;
        f.write_char('}')
    }
}

impl Display for pt::ErrorDefinition {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        self.keyword.fmt(f)?;
        write_opt!(f, ' ', &self.name);

        f.write_char('(')?;
        write_separated(&self.fields, f, ", ")?;
        f.write_str(");")
    }
}

impl Display for pt::ErrorParameter {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        self.ty.fmt(f)?;
        write_opt!(f, ' ', &self.name);
        Ok(())
    }
}

impl Display for pt::EventDefinition {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str("event")?;
        write_opt!(f, ' ', &self.name);

        f.write_char('(')?;
        write_separated(&self.fields, f, ", ")?;
        f.write_char(')')?;

        if self.anonymous {
            f.write_str(" anonymous")?;
        }
        f.write_char(';')
    }
}

impl Display for pt::EventParameter {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        self.ty.fmt(f)?;
        if self.indexed {
            f.write_str(" indexed")?;
        }
        write_opt!(f, ' ', &self.name);
        Ok(())
    }
}

impl Display for pt::FunctionDefinition {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        self.ty.fmt(f)?;
        write_opt!(f, ' ', &self.name);

        f.write_char('(')?;
        fmt_parameter_list(&self.params, f)?;
        f.write_char(')')?;

        if !self.attributes.is_empty() {
            f.write_char(' ')?;
            write_separated(&self.attributes, f, " ")?;
        }

        if !self.returns.is_empty() {
            f.write_str(" returns (")?;
            fmt_parameter_list(&self.returns, f)?;
            f.write_char(')')?;
        }

        if let Some(body) = &self.body {
            f.write_char(' ')?;
            body.fmt(f)
        } else {
            f.write_char(';')
        }
    }
}

impl Display for pt::HexLiteral {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str(&self.hex)
    }
}

impl Display for pt::Identifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str(&self.name)
    }
}

impl Display for pt::IdentifierPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write_separated(&self.identifiers, f, ".")
    }
}

impl Display for pt::NamedArgument {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        self.name.fmt(f)?;
        f.write_str(": ")?;
        self.expr.fmt(f)
    }
}

impl Display for pt::Parameter {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write_opt!(f, &self.annotation, ' ');
        self.ty.fmt(f)?;
        write_opt!(f, ' ', &self.storage);
        write_opt!(f, ' ', &self.name);
        Ok(())
    }
}

impl Display for pt::SourceUnit {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write_separated(&self.0, f, "\n")
    }
}

impl Display for pt::StringLiteral {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        if self.unicode {
            f.write_str("unicode")?;
        }
        f.write_char('"')?;
        f.write_str(&self.string)?;
        f.write_char('"')
    }
}

impl Display for pt::StructDefinition {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str("struct ")?;
        write_opt!(f, &self.name, ' ');

        f.write_char('{')?;
        write_separated(&self.fields, f, "; ")?;
        if !self.fields.is_empty() {
            f.write_char(';')?;
        }
        f.write_char('}')
    }
}

impl Display for pt::TypeDefinition {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str("type ")?;
        self.name.fmt(f)?;
        f.write_str(" is ")?;
        self.ty.fmt(f)?;
        f.write_char(';')
    }
}

impl Display for pt::Using {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str("using ")?;

        self.list.fmt(f)?;

        f.write_str(" for ")?;

        match &self.ty {
            Some(ty) => Display::fmt(ty, f),
            None => f.write_str("*"),
        }?;

        write_opt!(f, ' ', &self.global);
        f.write_char(';')
    }
}

impl Display for pt::UsingFunction {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        self.path.fmt(f)?;
        write_opt!(f, " as ", &self.oper);
        Ok(())
    }
}

impl Display for pt::VariableDeclaration {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        self.ty.fmt(f)?;
        write_opt!(f, ' ', &self.storage);
        write_opt!(f, ' ', &self.name);
        Ok(())
    }
}

impl Display for pt::VariableDefinition {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        self.ty.fmt(f)?;
        if !self.attrs.is_empty() {
            f.write_char(' ')?;
            write_separated(&self.attrs, f, " ")?;
        }
        write_opt!(f, ' ', &self.name);
        write_opt!(f, " = ", &self.initializer);
        f.write_char(';')
    }
}

impl Display for pt::YulBlock {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_char('{')?;
        write_separated(&self.statements, f, " ")?;
        f.write_char('}')
    }
}

impl Display for pt::YulFor {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str("for ")?;
        self.init_block.fmt(f)?;
        f.write_char(' ')?;
        self.condition.fmt(f)?;
        f.write_char(' ')?;
        self.post_block.fmt(f)?;
        f.write_char(' ')?;
        self.execution_block.fmt(f)
    }
}

impl Display for pt::YulFunctionCall {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        self.id.fmt(f)?;
        f.write_char('(')?;
        write_separated(&self.arguments, f, ", ")?;
        f.write_char(')')
    }
}

impl Display for pt::YulFunctionDefinition {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str("function ")?;
        self.id.fmt(f)?;
        f.write_char('(')?;
        write_separated(&self.params, f, ", ")?;
        f.write_str(") ")?;

        if !self.returns.is_empty() {
            f.write_str("-> (")?;
            write_separated(&self.returns, f, ", ")?;
            f.write_str(") ")?;
        }

        self.body.fmt(f)
    }
}

impl Display for pt::YulSwitch {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str("switch ")?;
        self.condition.fmt(f)?;
        if !self.cases.is_empty() {
            f.write_char(' ')?;
            write_separated(&self.cases, f, " ")?;
        }
        write_opt!(f, " ", &self.default);
        Ok(())
    }
}

impl Display for pt::YulTypedIdentifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        self.id.fmt(f)?;
        write_opt!(f, ": ", &self.ty);
        Ok(())
    }
}

// enums
impl Display for pt::CatchClause {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::Simple(_, param, block) => {
                f.write_str("catch ")?;
                write_opt!(f, '(', param, ") ");
                block.fmt(f)
            }
            Self::Named(_, ident, param, block) => {
                f.write_str("catch ")?;
                ident.fmt(f)?;
                f.write_char('(')?;
                param.fmt(f)?;
                f.write_str(") ")?;
                block.fmt(f)
            }
        }
    }
}

impl Display for pt::Comment {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str(self.value())
    }
}

impl Display for pt::ContractPart {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::StructDefinition(inner) => inner.fmt(f),
            Self::EventDefinition(inner) => inner.fmt(f),
            Self::EnumDefinition(inner) => inner.fmt(f),
            Self::ErrorDefinition(inner) => inner.fmt(f),
            Self::VariableDefinition(inner) => inner.fmt(f),
            Self::FunctionDefinition(inner) => inner.fmt(f),
            Self::TypeDefinition(inner) => inner.fmt(f),
            Self::Annotation(inner) => inner.fmt(f),
            Self::Using(inner) => inner.fmt(f),
            Self::StraySemicolon(_) => f.write_char(';'),
        }
    }
}

impl Display for pt::ContractTy {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str(self.as_str())
    }
}
impl pt::ContractTy {
    /// Returns the string representation of this type.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Abstract(..) => "abstract contract",
            Self::Contract(..) => "contract",
            Self::Interface(..) => "interface",
            Self::Library(..) => "library",
        }
    }
}

impl Display for pt::Expression {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::New(_, expr) => {
                f.write_str("new ")?;
                expr.fmt(f)
            }
            Self::Delete(_, expr) => {
                f.write_str("delete ")?;
                expr.fmt(f)
            }

            Self::Type(_, ty) => ty.fmt(f),

            Self::Variable(ident) => ident.fmt(f),

            Self::ArrayLiteral(_, exprs) => {
                f.write_char('[')?;
                write_separated(exprs, f, ", ")?;
                f.write_char(']')
            }
            Self::ArraySubscript(_, expr1, expr2) => {
                expr1.fmt(f)?;
                f.write_char('[')?;
                write_opt!(f, expr2);
                f.write_char(']')
            }
            Self::ArraySlice(_, arr, l, r) => {
                arr.fmt(f)?;
                f.write_char('[')?;
                write_opt!(f, l);
                f.write_char(':')?;
                write_opt!(f, r);
                f.write_char(']')
            }

            Self::MemberAccess(_, expr, ident) => {
                expr.fmt(f)?;
                f.write_char('.')?;
                ident.fmt(f)
            }

            Self::Parenthesis(_, expr) => {
                f.write_char('(')?;
                expr.fmt(f)?;
                f.write_char(')')
            }
            Self::List(_, list) => {
                f.write_char('(')?;
                fmt_parameter_list(list, f)?;
                f.write_char(')')
            }

            Self::AddressLiteral(_, lit) => f.write_str(lit),
            Self::StringLiteral(vals) => write_separated(vals, f, " "),
            Self::HexLiteral(vals) => write_separated(vals, f, " "),
            Self::BoolLiteral(_, bool) => {
                let s = if *bool { "true" } else { "false" };
                f.write_str(s)
            }
            Self::HexNumberLiteral(_, val, unit) => {
                // TODO: Check with and write the checksummed address when len == 42
                // ref: https://docs.soliditylang.org/en/latest/types.html#address-literals
                f.write_str(val)?;
                write_opt!(f, ' ', unit);
                Ok(())
            }
            Self::NumberLiteral(_, val, exp, unit) => {
                let val = rm_underscores(val);
                f.write_str(&val)?;
                if !exp.is_empty() {
                    f.write_char('e')?;
                    let exp = rm_underscores(exp);
                    f.write_str(&exp)?;
                }
                write_opt!(f, ' ', unit);
                Ok(())
            }
            Self::RationalNumberLiteral(_, val, fraction, exp, unit) => {
                let val = rm_underscores(val);
                f.write_str(&val)?;

                let mut fraction = fraction.trim_end_matches('0');
                if fraction.is_empty() {
                    fraction = "0"
                }
                f.write_char('.')?;
                f.write_str(fraction)?;

                if !exp.is_empty() {
                    f.write_char('e')?;
                    let exp = rm_underscores(exp);
                    f.write_str(&exp)?;
                }
                write_opt!(f, ' ', unit);
                Ok(())
            }

            Self::FunctionCall(_, expr, exprs) => {
                expr.fmt(f)?;
                f.write_char('(')?;
                write_separated(exprs, f, ", ")?;
                f.write_char(')')
            }
            Self::FunctionCallBlock(_, expr, block) => {
                expr.fmt(f)?;
                block.fmt(f)
            }
            Self::NamedFunctionCall(_, expr, args) => {
                expr.fmt(f)?;
                f.write_str("({")?;
                write_separated(args, f, ", ")?;
                f.write_str("})")
            }

            Self::ConditionalOperator(_, cond, l, r) => {
                cond.fmt(f)?;
                f.write_str(" ? ")?;
                l.fmt(f)?;
                f.write_str(" : ")?;
                r.fmt(f)
            }

            Self::PreIncrement(..)
            | Self::PostIncrement(..)
            | Self::PreDecrement(..)
            | Self::PostDecrement(..)
            | Self::Not(..)
            | Self::BitwiseNot(..)
            | Self::UnaryPlus(..)
            | Self::Add(..)
            | Self::Negate(..)
            | Self::Subtract(..)
            | Self::Power(..)
            | Self::Multiply(..)
            | Self::Divide(..)
            | Self::Modulo(..)
            | Self::ShiftLeft(..)
            | Self::ShiftRight(..)
            | Self::BitwiseAnd(..)
            | Self::BitwiseXor(..)
            | Self::BitwiseOr(..)
            | Self::Less(..)
            | Self::More(..)
            | Self::LessEqual(..)
            | Self::MoreEqual(..)
            | Self::And(..)
            | Self::Or(..)
            | Self::Equal(..)
            | Self::NotEqual(..)
            | Self::Assign(..)
            | Self::AssignOr(..)
            | Self::AssignAnd(..)
            | Self::AssignXor(..)
            | Self::AssignShiftLeft(..)
            | Self::AssignShiftRight(..)
            | Self::AssignAdd(..)
            | Self::AssignSubtract(..)
            | Self::AssignMultiply(..)
            | Self::AssignDivide(..)
            | Self::AssignModulo(..) => {
                let (left, right) = self.components();
                let has_spaces = self.has_space_around();

                if let Some(left) = left {
                    left.fmt(f)?;
                    if has_spaces {
                        f.write_char(' ')?;
                    }
                }

                let operator = self.operator().unwrap();
                f.write_str(operator)?;

                if let Some(right) = right {
                    if has_spaces {
                        f.write_char(' ')?;
                    }
                    right.fmt(f)?;
                }

                Ok(())
            }
        }
    }
}
impl pt::Expression {
    /// Returns the operator string of this expression, if any.
    #[inline]
    pub const fn operator(&self) -> Option<&'static str> {
        use pt::Expression::*;
        let operator = match self {
            New(..) => "new",
            Delete(..) => "delete",

            PreIncrement(..) | PostIncrement(..) => "++",
            PreDecrement(..) | PostDecrement(..) => "--",

            Not(..) => "!",
            BitwiseNot(..) => "~",
            UnaryPlus(..) | Add(..) => "+",
            Negate(..) | Subtract(..) => "-",
            Power(..) => "**",
            Multiply(..) => "*",
            Divide(..) => "/",
            Modulo(..) => "%",
            ShiftLeft(..) => "<<",
            ShiftRight(..) => ">>",
            BitwiseAnd(..) => "&",
            BitwiseXor(..) => "^",
            BitwiseOr(..) => "|",

            Less(..) => "<",
            More(..) => ">",
            LessEqual(..) => "<=",
            MoreEqual(..) => ">=",
            And(..) => "&&",
            Or(..) => "||",
            Equal(..) => "==",
            NotEqual(..) => "!=",

            Assign(..) => "=",
            AssignOr(..) => "|=",
            AssignAnd(..) => "&=",
            AssignXor(..) => "^=",
            AssignShiftLeft(..) => "<<=",
            AssignShiftRight(..) => ">>=",
            AssignAdd(..) => "+=",
            AssignSubtract(..) => "-=",
            AssignMultiply(..) => "*=",
            AssignDivide(..) => "/=",
            AssignModulo(..) => "%=",

            MemberAccess(..)
            | ArraySubscript(..)
            | ArraySlice(..)
            | FunctionCall(..)
            | FunctionCallBlock(..)
            | NamedFunctionCall(..)
            | ConditionalOperator(..)
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
            | ArrayLiteral(..)
            | Parenthesis(..) => return None,
        };
        Some(operator)
    }
}

impl Display for pt::FunctionAttribute {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::Mutability(mutability) => mutability.fmt(f),
            Self::Visibility(visibility) => visibility.fmt(f),
            Self::Virtual(_) => f.write_str("virtual"),
            Self::Immutable(_) => f.write_str("immutable"),
            Self::Override(_, idents) => {
                f.write_str("override")?;
                if !idents.is_empty() {
                    f.write_char('(')?;
                    write_separated(idents, f, ", ")?;
                    f.write_char(')')?;
                }
                Ok(())
            }
            Self::BaseOrModifier(_, base) => base.fmt(f),
            Self::Error(_) => Ok(()),
        }
    }
}

impl Display for pt::FunctionTy {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str(self.as_str())
    }
}
impl pt::FunctionTy {
    /// Returns the string representation of this type.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Constructor => "constructor",
            Self::Function => "function",
            Self::Fallback => "fallback",
            Self::Receive => "receive",
            Self::Modifier => "modifier",
        }
    }
}

impl Display for pt::Import {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::Plain(lit, _) => {
                f.write_str("import ")?;
                lit.fmt(f)?;
                f.write_char(';')
            }
            Self::GlobalSymbol(lit, ident, _) => {
                f.write_str("import ")?;
                lit.fmt(f)?;
                f.write_str(" as ")?;
                ident.fmt(f)?;
                f.write_char(';')
            }
            Self::Rename(lit, idents, _) => {
                f.write_str("import {")?;

                // same as `write_separated_iter`
                let mut idents = idents.iter();
                if let Some((ident, as_ident)) = idents.next() {
                    ident.fmt(f)?;
                    write_opt!(f, " as ", as_ident);
                    for (ident, as_ident) in idents {
                        f.write_str(", ")?;
                        ident.fmt(f)?;
                        write_opt!(f, " as ", as_ident);
                    }
                }
                f.write_str("} from ")?;
                lit.fmt(f)?;
                f.write_char(';')
            }
        }
    }
}

impl Display for pt::ImportPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::Filename(lit) => lit.fmt(f),
            Self::Path(path) => path.fmt(f),
        }
    }
}

impl Display for pt::Mutability {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str(self.as_str())
    }
}
impl pt::Mutability {
    /// Returns the string representation of this type.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Pure(_) => "pure",
            Self::Constant(_) | Self::View(_) => "view",
            Self::Payable(_) => "payable",
        }
    }
}

impl Display for pt::SourceUnitPart {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::ImportDirective(inner) => inner.fmt(f),
            Self::ContractDefinition(inner) => inner.fmt(f),
            Self::EnumDefinition(inner) => inner.fmt(f),
            Self::StructDefinition(inner) => inner.fmt(f),
            Self::EventDefinition(inner) => inner.fmt(f),
            Self::ErrorDefinition(inner) => inner.fmt(f),
            Self::FunctionDefinition(inner) => inner.fmt(f),
            Self::VariableDefinition(inner) => inner.fmt(f),
            Self::TypeDefinition(inner) => inner.fmt(f),
            Self::Annotation(inner) => inner.fmt(f),
            Self::Using(inner) => inner.fmt(f),
            Self::PragmaDirective(inner) => inner.fmt(f),
            Self::StraySemicolon(_) => f.write_char(';'),
        }
    }
}

impl Display for pt::PragmaDirective {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::Identifier(_, ident, val) => {
                f.write_str("pragma")?;
                write_opt!(f, ' ', ident);
                write_opt!(f, ' ', val);
                f.write_char(';')
            }
            Self::StringLiteral(_, ident, lit) => {
                f.write_str("pragma ")?;
                ident.fmt(f)?;
                f.write_char(' ')?;
                lit.fmt(f)?;
                f.write_char(';')
            }
            Self::Version(_, ident, versions) => {
                f.write_str("pragma ")?;
                ident.fmt(f)?;
                f.write_char(' ')?;
                write_separated(versions, f, " ")?;
                f.write_char(';')
            }
        }
    }
}

impl Display for pt::VersionComparator {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::Plain { version, .. } => write_separated(version, f, "."),
            Self::Operator { op, version, .. } => {
                op.fmt(f)?;
                write_separated(version, f, ".")
            }
            Self::Range { from, to, .. } => {
                write_separated(from, f, ".")?;
                f.write_str(" - ")?;
                write_separated(to, f, ".")
            }
            Self::Or { left, right, .. } => {
                left.fmt(f)?;
                f.write_str(" || ")?;
                right.fmt(f)
            }
        }
    }
}

impl Display for pt::VersionOp {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str(self.as_str())
    }
}

impl pt::VersionOp {
    /// Returns the string representation of this type.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Exact => "=",
            Self::Greater => ">",
            Self::GreaterEq => ">=",
            Self::Less => "<",
            Self::LessEq => "<=",
            Self::Tilde => "~",
            Self::Caret => "^",
            Self::Wildcard => "*",
        }
    }
}

impl Display for pt::Statement {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::Block {
                unchecked,
                statements,
                ..
            } => {
                if *unchecked {
                    f.write_str("unchecked ")?;
                }

                f.write_char('{')?;
                write_separated(statements, f, " ")?;
                f.write_char('}')
            }
            Self::Assembly {
                dialect,
                flags,
                block,
                ..
            } => {
                f.write_str("assembly ")?;
                write_opt!(f, dialect, ' ');
                if let Some(flags) = flags {
                    if !flags.is_empty() {
                        f.write_char('(')?;
                        write_separated(flags, f, ", ")?;
                        f.write_str(") ")?;
                    }
                }
                block.fmt(f)
            }
            Self::Args(_, args) => {
                f.write_char('{')?;
                write_separated(args, f, ", ")?;
                f.write_char('}')
            }
            Self::If(_, cond, block, end_block) => {
                f.write_str("if (")?;
                cond.fmt(f)?;
                f.write_str(") ")?;
                block.fmt(f)?;
                write_opt!(f, " else ", end_block);
                Ok(())
            }
            Self::While(_, cond, block) => {
                f.write_str("while (")?;
                cond.fmt(f)?;
                f.write_str(") ")?;
                block.fmt(f)
            }
            Self::Expression(_, expr) => {
                expr.fmt(f)?;
                f.write_char(';')
            }
            Self::VariableDefinition(_, var, expr) => {
                var.fmt(f)?;
                write_opt!(f, " = ", expr);
                f.write_char(';')
            }
            Self::For(_, init, cond, expr, block) => {
                f.write_str("for (")?;
                // edge case, don't write semicolon on a variable definition
                match init.as_deref() {
                    Some(var @ pt::Statement::VariableDefinition(..)) => var.fmt(f),
                    Some(stmt) => {
                        stmt.fmt(f)?;
                        f.write_char(';')
                    }
                    None => f.write_char(';'),
                }?;
                write_opt!(f, ' ', cond);
                f.write_char(';')?;
                write_opt!(f, ' ', expr);
                f.write_str(") ")?;
                if let Some(block) = block {
                    block.fmt(f)
                } else {
                    f.write_char(';')
                }
            }
            Self::DoWhile(_, block, cond) => {
                f.write_str("do ")?;
                block.fmt(f)?;
                f.write_str(" while (")?;
                cond.fmt(f)?;
                f.write_str(");")
            }
            Self::Continue(_) => f.write_str("continue;"),
            Self::Break(_) => f.write_str("break;"),
            Self::Return(_, expr) => {
                f.write_str("return")?;
                write_opt!(f, ' ', expr);
                f.write_char(';')
            }
            Self::Revert(_, ident, exprs) => {
                f.write_str("revert")?;
                write_opt!(f, ' ', ident);
                f.write_char('(')?;
                write_separated(exprs, f, ", ")?;
                f.write_str(");")
            }
            Self::RevertNamedArgs(_, ident, args) => {
                f.write_str("revert")?;
                write_opt!(f, ' ', ident);
                f.write_char('(')?;
                if !args.is_empty() {
                    f.write_char('{')?;
                    write_separated(args, f, ", ")?;
                    f.write_char('}')?;
                }
                f.write_str(");")
            }
            Self::Emit(_, expr) => {
                f.write_str("emit ")?;
                expr.fmt(f)?;
                f.write_char(';')
            }
            Self::Try(_, expr, returns, catch) => {
                f.write_str("try ")?;
                expr.fmt(f)?;

                if let Some((list, stmt)) = returns {
                    f.write_str(" returns (")?;
                    fmt_parameter_list(list, f)?;
                    f.write_str(") ")?;
                    stmt.fmt(f)?;
                }

                if !catch.is_empty() {
                    f.write_char(' ')?;
                    write_separated(catch, f, " ")?;
                }
                Ok(())
            }
            Self::Error(_) => Ok(()),
        }
    }
}

impl Display for pt::StorageLocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str(self.as_str())
    }
}
impl pt::StorageLocation {
    /// Returns the string representation of this type.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Memory(_) => "memory",
            Self::Storage(_) => "storage",
            Self::Calldata(_) => "calldata",
        }
    }
}

impl Display for pt::Type {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::Address => f.write_str("address"),
            Self::AddressPayable => f.write_str("address payable"),
            Self::Payable => f.write_str("payable"),
            Self::Bool => f.write_str("bool"),
            Self::String => f.write_str("string"),
            Self::Rational => f.write_str("fixed"),
            Self::DynamicBytes => f.write_str("bytes"),
            Self::Bytes(n) => {
                f.write_str("bytes")?;
                n.fmt(f)
            }
            Self::Int(n) => {
                f.write_str("int")?;
                n.fmt(f)
            }
            Self::Uint(n) => {
                f.write_str("uint")?;
                n.fmt(f)
            }
            Self::Mapping {
                key,
                key_name,
                value,
                value_name,
                ..
            } => {
                f.write_str("mapping(")?;

                key.fmt(f)?;
                write_opt!(f, ' ', key_name);

                f.write_str(" => ")?;

                value.fmt(f)?;
                write_opt!(f, ' ', value_name);

                f.write_char(')')
            }
            Self::Function {
                params,
                attributes,
                returns,
            } => {
                f.write_str("function (")?;
                fmt_parameter_list(params, f)?;
                f.write_char(')')?;

                if !attributes.is_empty() {
                    f.write_char(' ')?;
                    write_separated(attributes, f, " ")?;
                }

                if let Some((returns, attrs)) = returns {
                    if !attrs.is_empty() {
                        f.write_char(' ')?;
                        write_separated(attrs, f, " ")?;
                    }

                    if !returns.is_empty() {
                        f.write_str(" returns (")?;
                        fmt_parameter_list(returns, f)?;
                        f.write_char(')')?;
                    }
                }
                Ok(())
            }
        }
    }
}

impl Display for pt::UserDefinedOperator {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str(self.as_str())
    }
}

impl pt::UserDefinedOperator {
    /// Returns the string representation of this type.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::BitwiseAnd => "&",
            Self::BitwiseNot => "~",
            Self::Negate => "-",
            Self::BitwiseOr => "|",
            Self::BitwiseXor => "^",
            Self::Add => "+",
            Self::Divide => "/",
            Self::Modulo => "%",
            Self::Multiply => "*",
            Self::Subtract => "-",
            Self::Equal => "==",
            Self::More => ">",
            Self::MoreEqual => ">=",
            Self::Less => "<",
            Self::LessEqual => "<=",
            Self::NotEqual => "!=",
        }
    }
}

impl Display for pt::UsingList {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::Library(ident) => ident.fmt(f),
            Self::Functions(list) => {
                f.write_char('{')?;
                write_separated(list, f, ", ")?;
                f.write_char('}')
            }
            Self::Error => Ok(()),
        }
    }
}

impl Display for pt::VariableAttribute {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::Visibility(vis) => vis.fmt(f),
            Self::Constant(_) => f.write_str("constant"),
            Self::Immutable(_) => f.write_str("immutable"),
            Self::Override(_, idents) => {
                f.write_str("override")?;
                if !idents.is_empty() {
                    f.write_char('(')?;
                    write_separated(idents, f, ", ")?;
                    f.write_char(')')?;
                }
                Ok(())
            }
            Self::StorageType(storage) => match storage {
                StorageType::Instance(_) => f.write_str("instance"),
                StorageType::Temporary(_) => f.write_str("temporary"),
                StorageType::Persistent(_) => f.write_str("persistent"),
            },
        }
    }
}

impl Display for pt::Visibility {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.write_str(self.as_str())
    }
}
impl pt::Visibility {
    /// Returns the string representation of this type.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Public(_) => "public",
            Self::Internal(_) => "internal",
            Self::Private(_) => "private",
            Self::External(_) => "external",
        }
    }
}

impl Display for pt::YulExpression {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::BoolLiteral(_, value, ident) => {
                let value = if *value { "true" } else { "false" };
                f.write_str(value)?;
                write_opt!(f, ": ", ident);
                Ok(())
            }
            Self::NumberLiteral(_, value, exponent, ident) => {
                f.write_str(value)?;
                if !exponent.is_empty() {
                    f.write_char('e')?;
                    f.write_str(exponent)?;
                }
                write_opt!(f, ": ", ident);
                Ok(())
            }
            Self::HexNumberLiteral(_, value, ident) => {
                f.write_str(value)?;
                write_opt!(f, ": ", ident);
                Ok(())
            }
            Self::HexStringLiteral(value, ident) => {
                value.fmt(f)?;
                write_opt!(f, ": ", ident);
                Ok(())
            }
            Self::StringLiteral(value, ident) => {
                value.fmt(f)?;
                write_opt!(f, ": ", ident);
                Ok(())
            }
            Self::Variable(ident) => ident.fmt(f),
            Self::FunctionCall(call) => call.fmt(f),
            Self::SuffixAccess(_, l, r) => {
                l.fmt(f)?;
                f.write_char('.')?;
                r.fmt(f)
            }
        }
    }
}

impl Display for pt::YulStatement {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::Block(inner) => inner.fmt(f),
            Self::FunctionDefinition(inner) => inner.fmt(f),
            Self::FunctionCall(inner) => inner.fmt(f),
            Self::For(inner) => inner.fmt(f),
            Self::Switch(inner) => inner.fmt(f),

            Self::Assign(_, exprs, eq_expr) => {
                write_separated(exprs, f, ", ")?;
                f.write_str(" := ")?;
                eq_expr.fmt(f)
            }
            Self::VariableDeclaration(_, vars, eq_expr) => {
                f.write_str("let")?;
                if !vars.is_empty() {
                    f.write_char(' ')?;
                    write_separated(vars, f, ", ")?;
                }
                write_opt!(f, " := ", eq_expr);
                Ok(())
            }

            Self::If(_, expr, block) => {
                f.write_str("if ")?;
                expr.fmt(f)?;
                f.write_char(' ')?;
                block.fmt(f)
            }

            Self::Leave(_) => f.write_str("leave"),
            Self::Break(_) => f.write_str("break"),
            Self::Continue(_) => f.write_str("continue"),

            Self::Error(_) => Ok(()),
        }
    }
}

impl Display for pt::YulSwitchOptions {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::Case(_, expr, block) => {
                f.write_str("case ")?;
                expr.fmt(f)?;
                f.write_str(" ")?;
                block.fmt(f)
            }
            Self::Default(_, block) => {
                f.write_str("default ")?;
                block.fmt(f)
            }
        }
    }
}

// These functions are private so they should be inlined by the compiler.
// We provided these `#[inline]` hints regardless because we don't expect compile time penalties
// or other negative impacts from them.
// See: <https://github.com/hyperledger-solang/solang/pull/1237#discussion_r1151557453>
#[inline]
fn fmt_parameter_list(list: &pt::ParameterList, f: &mut Formatter<'_>) -> Result {
    let iter = list.iter().flat_map(|(_, param)| param);
    write_separated_iter(iter, f, ", ")
}

#[inline]
fn write_separated<T: Display>(slice: &[T], f: &mut Formatter<'_>, sep: &str) -> Result {
    write_separated_iter(slice.iter(), f, sep)
}

fn write_separated_iter<T, I>(mut iter: I, f: &mut Formatter<'_>, sep: &str) -> Result
where
    I: Iterator<Item = T>,
    T: Display,
{
    if let Some(first) = iter.next() {
        first.fmt(f)?;
        for item in iter {
            f.write_str(sep)?;
            item.fmt(f)?;
        }
    }
    Ok(())
}

fn rm_underscores(s: &str) -> Cow<'_, str> {
    if s.is_empty() {
        Cow::Borrowed("0")
    } else if s.contains('_') {
        let mut s = s.to_string();
        s.retain(|c| c != '_');
        Cow::Owned(s)
    } else {
        Cow::Borrowed(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pt::{Annotation, Loc};

    macro_rules! struct_tests {
        ($(pt::$t:ident { $( $f:ident: $e:expr ),* $(,)? } => $expected:expr),* $(,)?) => {
            $(
                assert_eq_display(
                    pt::$t {
                        loc: loc!(),
                        $( $f: $e, )*
                    },
                    $expected,
                );
            )*
        };
    }

    macro_rules! enum_tests {
        ($(
            $t:ty: {
                $($p:expr => $expected:expr,)+
            }
        )+) => {
            $(
                $(
                    assert_eq_display($p, $expected);
                )+
            )+
        };
    }

    /// Expression
    macro_rules! expr {
        (this) => {
            pt::Expression::This(loc!())
        };

        ($i:ident) => {
            pt::Expression::Variable(id(stringify!($i)))
        };

        ($l:literal) => {
            pt::Expression::Variable(id(stringify!($l)))
        };

        (++ $($t:tt)+) => {
            pt::Expression::PreIncrement(loc!(), Box::new(expr!($($t)+)))
        };

        ($($t:tt)+ ++) => {
            pt::Expression::PostIncrement(loc!(), Box::new(expr!($($t)+)))
        };
    }
    macro_rules! yexpr {
        ($i:ident) => {
            pt::YulExpression::Variable(id(stringify!($i)))
        };
        ($l:literal) => {
            pt::YulExpression::Variable(id(stringify!($l)))
        };
    }

    /// Type
    macro_rules! ty {
        (uint256) => {
            pt::Type::Uint(256)
        };
        (string) => {
            pt::Type::String
        };
        (bytes) => {
            pt::Type::DynamicBytes
        };
        (address) => {
            pt::Type::Address
        };
    }
    macro_rules! expr_ty {
        ($($t:tt)+) => {
            pt::Expression::Type(loc!(), ty!($($t)+))
        };
    }

    /// Literals
    macro_rules! lit {
        // prefixes are not allowed in rust strings
        (unicode $($l:literal)+) => {
            pt::StringLiteral {
                loc: loc!(),
                unicode: true,
                string: concat!( $($l),+ ).to_string(),
            }
        };

        (hex $($l:literal)+) => {
            pt::HexLiteral {
                loc: loc!(),
                hex: concat!( "hex\"", $($l),+ , "\"" ).to_string(),
            }
        };

        ($($l:literal)+) => {
            pt::StringLiteral {
                loc: loc!(),
                unicode: false,
                string: concat!( $($l),+ ).to_string(),
            }
        };
    }

    /// VersionComparsion
    macro_rules! version {
        ($($l:literal),+) => {
            <[_]>::into_vec(Box::new([ $( $l.into() ),+ ]))
        }
    }

    macro_rules! plain_version {
        ($($l:literal),+) => {
            pt::VersionComparator::Plain {
                loc: loc!(),
                version: <[_]>::into_vec(Box::new([ $( $l.into() ),+ ])),
            }
        };
    }

    macro_rules! op_version {
        ($op:expr, $($l:literal),+) => {
            pt::VersionComparator::Operator {
                loc: loc!(),
                op: $op,
                version: <[_]>::into_vec(Box::new([ $( $l.into() ),+ ])),
            }
        };
    }

    macro_rules! range_version {
        ($from:expr, $to:expr) => {
            pt::VersionComparator::Range {
                loc: loc!(),
                from: $from,
                to: $to,
            }
        };
    }

    macro_rules! or_version {
        ($left:expr, $right:expr) => {
            pt::VersionComparator::Or {
                loc: loc!(),
                left: $left.into(),
                right: $right.into(),
            }
        };
    }

    /// Statement
    macro_rules! stmt {
        ( {} ) => {
            pt::Statement::Block {
                loc: loc!(),
                unchecked: false,
                statements: vec![],
            }
        };

        ( unchecked { $($t:tt)* } ) => {
            pt::Statement::Block {
                loc: loc!(),
                unchecked: true,
                statements: vec![stmt!($(t)*)],
            }
        };
        ( { $($t:tt)* } ) => {
            pt::Statement::Block {
                loc: loc!(),
                unchecked: false,
                statements: vec![stmt!($(t)*)],
            }
        };
    }

    /// IdentifierPath
    macro_rules! idp {
        ($($e:expr),* $(,)?) => {
            pt::IdentifierPath {
                loc: loc!(),
                identifiers: vec![$(id($e)),*],
            }
        };
    }

    macro_rules! loc {
        () => {
            pt::Loc::File(0, 0, 0)
        };
    }

    /// Param
    macro_rules! param {
        ($i:ident) => {
            pt::Parameter {
                loc: loc!(),
                ty: expr_ty!($i),
                storage: None,
                name: None,
                annotation: None,
            }
        };

        ($i:ident $n:ident) => {
            pt::Parameter {
                loc: loc!(),
                ty: expr_ty!($i),
                storage: None,
                name: Some(id(stringify!($n))),
                annotation: None,
            }
        };

        ($i:ident $s:ident $n:ident) => {
            pt::Parameter {
                loc: loc!(),
                ty: expr_ty!($i),
                storage: Some(storage!($s)),
                name: Some(id(stringify!($n))),
                annotation: None,
            }
        };
    }

    macro_rules! storage {
        (memory) => {
            pt::StorageLocation::Memory(loc!())
        };
        (storage) => {
            pt::StorageLocation::Storage(loc!())
        };
        (calldata) => {
            pt::StorageLocation::Calldata(loc!())
        };
    }

    /// Identifier
    fn id(s: &str) -> pt::Identifier {
        pt::Identifier {
            loc: loc!(),
            name: s.to_string(),
        }
    }

    macro_rules! yid {
        ($i:ident) => {
            pt::YulTypedIdentifier {
                loc: loc!(),
                id: id(stringify!($i)),
                ty: None,
            }
        };

        ($i:ident : $t:ident) => {
            pt::YulTypedIdentifier {
                loc: loc!(),
                id: id(stringify!($i)),
                ty: Some(id(stringify!($t))),
            }
        };
    }

    fn var(s: &str) -> Box<pt::Expression> {
        Box::new(pt::Expression::Variable(id(s)))
    }

    fn yul_block() -> pt::YulBlock {
        pt::YulBlock {
            loc: loc!(),
            statements: vec![],
        }
    }

    fn assert_eq_display<T: Display + std::fmt::Debug>(item: T, expected: &str) {
        let ty = std::any::type_name::<T>();
        let actual = item.to_string();
        assert_eq!(actual, expected, "\"{ty}\": {item:?}");
        // TODO: Test parsing back into an item
        // let parsed = ;
        // assert_eq!(parsed, item, "failed to parse display back into an item: {expected}");
    }

    #[test]
    fn display_structs_simple() {
        struct_tests![
            pt::Annotation {
                id: id("name"),
                value: Some(expr!(value)),
            } => "@name(value)",

            pt::Base {
                name: idp!("id", "path"),
                args: None,
            } => "id.path",
            pt::Base {
                name: idp!("id", "path"),
                args: Some(vec![expr!(value)]),
            } => "id.path(value)",
            pt::Base {
                name: idp!("id", "path"),
                args: Some(vec![expr!(value1), expr!(value2)]),
            } => "id.path(value1, value2)",

            pt::ErrorParameter {
                ty: expr_ty!(uint256),
                name: None,
            } => "uint256",
            pt::ErrorParameter {
                ty: expr_ty!(uint256),
                name: Some(id("name")),
            } => "uint256 name",

            pt::EventParameter {
                ty: expr_ty!(uint256),
                indexed: false,
                name: None,
            } => "uint256",
            pt::EventParameter {
                ty: expr_ty!(uint256),
                indexed: true,
                name: None,
            } => "uint256 indexed",
            pt::EventParameter {
                ty: expr_ty!(uint256),
                indexed: false,
                name: Some(id("name")),
            } => "uint256 name",
            pt::EventParameter {
                ty: expr_ty!(uint256),
                indexed: true,
                name: Some(id("name")),
            } => "uint256 indexed name",

            pt::HexLiteral {
                hex: "hex\"1234\"".into(),
            } => "hex\"1234\"",
            pt::HexLiteral {
                hex: "hex\"455318975130845\"".into(),
            } => "hex\"455318975130845\"",

            pt::Identifier {
                name: "name".to_string(),
            } => "name",

            pt::IdentifierPath {
                identifiers: vec![id("id")],
            } => "id",
            pt::IdentifierPath {
                identifiers: vec![id("id"), id("path")],
            } => "id.path",
            pt::IdentifierPath {
                identifiers: vec![id("long"), id("id"), id("path")],
            } => "long.id.path",

            pt::NamedArgument {
                name: id("name"),
                expr: expr!(expr),
            } => "name: expr",

            pt::Parameter {
                ty: expr_ty!(uint256),
                storage: None,
                name: None,
                annotation: None,
            } => "uint256",
            pt::Parameter {
                ty: expr_ty!(uint256),
                storage: None,
                name: Some(id("name")),
                annotation: None,
            } => "uint256 name",
            pt::Parameter {
                ty: expr_ty!(uint256),
                storage: Some(pt::StorageLocation::Calldata(Default::default())),
                name: Some(id("name")),
                annotation: None,
            } => "uint256 calldata name",
            pt::Parameter {
                ty: expr_ty!(uint256),
                storage: Some(pt::StorageLocation::Calldata(Default::default())),
                name: None,
                annotation: None,
            } => "uint256 calldata",
            pt::Parameter {
                ty: expr_ty!(bytes),
                storage: None,
                name: Some(id("my_seed")),
                annotation: Some(Annotation {
                    loc: Loc::Builtin,
                    id: id("name"),
                    value: None,
                }),
            } => "@name bytes my_seed",

            pt::StringLiteral {
                unicode: false,
                string: "string".into(),
            } => "\"string\"",
            pt::StringLiteral {
                unicode: true,
                string: "string".into(),
            } => "unicode\"string\"",

            pt::UsingFunction {
                path: idp!["id", "path"],
                oper: None,
            } => "id.path",
            pt::UsingFunction {
                path: idp!["id", "path"],
                oper: Some(pt::UserDefinedOperator::Add),
            } => "id.path as +",

            pt::VariableDeclaration {
                ty: expr_ty!(uint256),
                storage: None,
                name: None,
            } => "uint256",
            pt::VariableDeclaration {
                ty: expr_ty!(uint256),
                storage: None,
                name: Some(id("name")),
            } => "uint256 name",
            pt::VariableDeclaration {
                ty: expr_ty!(uint256),
                storage: Some(pt::StorageLocation::Calldata(Default::default())),
                name: Some(id("name")),
            } => "uint256 calldata name",
            pt::VariableDeclaration {
                ty: expr_ty!(uint256),
                storage: Some(pt::StorageLocation::Calldata(Default::default())),
                name: None,
            } => "uint256 calldata",

            pt::VariableDefinition {
                ty: expr_ty!(uint256),
                attrs: vec![],
                name: None,
                initializer: None,
            } => "uint256;",
            pt::VariableDefinition {
                ty: expr_ty!(uint256),
                attrs: vec![],
                name: Some(id("name")),
                initializer: None,
            } => "uint256 name;",
            pt::VariableDefinition {
                ty: expr_ty!(uint256),
                attrs: vec![],
                name: Some(id("name")),
                initializer: Some(expr!(value)),
            } => "uint256 name = value;",
            pt::VariableDefinition {
                ty: expr_ty!(uint256),
                attrs: vec![pt::VariableAttribute::Constant(loc!())],
                name: Some(id("name")),
                initializer: Some(expr!(value)),
            } => "uint256 constant name = value;",
            pt::VariableDefinition {
                ty: expr_ty!(uint256),
                attrs: vec![
                    pt::VariableAttribute::Visibility(pt::Visibility::Public(None)),
                    pt::VariableAttribute::Constant(loc!())
                ],
                name: Some(id("name")),
                initializer: Some(expr!(value)),
            } => "uint256 public constant name = value;",

            pt::YulTypedIdentifier {
                id: id("name"),
                ty: None,
            } => "name",
            pt::YulTypedIdentifier {
                id: id("name"),
                ty: Some(id("uint256")),
            } => "name: uint256",
        ];
    }

    #[test]
    fn display_structs_complex() {
        struct_tests![
            pt::ContractDefinition {
                ty: pt::ContractTy::Contract(loc!()),
                name: Some(id("name")),
                base: vec![],
                parts: vec![],
            } => "contract name {}",
            pt::ContractDefinition {
                ty: pt::ContractTy::Contract(loc!()),
                name: Some(id("name")),
                base: vec![pt::Base {
                    loc: loc!(),
                    name: idp!("base"),
                    args: None
                }],
                parts: vec![],
            } => "contract name base {}",
            pt::ContractDefinition {
                ty: pt::ContractTy::Contract(loc!()),
                name: Some(id("name")),
                base: vec![pt::Base {
                    loc: loc!(),
                    name: idp!("base"),
                    args: Some(vec![])
                }],
                parts: vec![],
            } => "contract name base() {}",
            pt::ContractDefinition {
                ty: pt::ContractTy::Contract(loc!()),
                name: Some(id("name")),
                base: vec![pt::Base {
                    loc: loc!(),
                    name: idp!("base"),
                    args: Some(vec![expr!(expr)])
                }],
                parts: vec![],
            } => "contract name base(expr) {}",
            pt::ContractDefinition {
                ty: pt::ContractTy::Contract(loc!()),
                name: Some(id("name")),
                base: vec![
                    pt::Base {
                        loc: loc!(),
                        name: idp!("base1"),
                        args: None
                    },
                    pt::Base {
                        loc: loc!(),
                        name: idp!("base2"),
                        args: None
                    },
                ],
                parts: vec![],
            } => "contract name base1 base2 {}",

            pt::EnumDefinition {
                name: Some(id("name")),
                values: vec![]
            } => "enum name {}",
            pt::EnumDefinition {
                name: Some(id("name")),
                values: vec![Some(id("variant"))]
            } => "enum name {variant}",
            pt::EnumDefinition {
                name: Some(id("name")),
                values: vec![
                    Some(id("variant1")),
                    Some(id("variant2")),
                ]
            } => "enum name {variant1, variant2}",

            pt::ErrorDefinition {
                keyword: expr!(error),
                name: Some(id("name")),
                fields: vec![],
            } => "error name();",
            pt::ErrorDefinition {
                keyword: expr!(error),
                name: Some(id("name")),
                fields: vec![pt::ErrorParameter {
                    loc: loc!(),
                    ty: expr_ty!(uint256),
                    name: None,
                }],
            } => "error name(uint256);",

            pt::EventDefinition {
                name: Some(id("name")),
                fields: vec![],
                anonymous: false,
            } => "event name();",
            pt::EventDefinition {
                name: Some(id("name")),
                fields: vec![pt::EventParameter {
                    loc: loc!(),
                    ty: expr_ty!(uint256),
                    indexed: false,
                    name: None,
                }],
                anonymous: false,
            } => "event name(uint256);",
            pt::EventDefinition {
                name: Some(id("name")),
                fields: vec![pt::EventParameter {
                    loc: loc!(),
                    ty: expr_ty!(uint256),
                    indexed: true,
                    name: None,
                }],
                anonymous: false,
            } => "event name(uint256 indexed);",
            pt::EventDefinition {
                name: Some(id("name")),
                fields: vec![],
                anonymous: true,
            } => "event name() anonymous;",

            pt::FunctionDefinition {
                loc_prototype: loc!(),
                ty: pt::FunctionTy::Function,
                name: Some(id("name")),
                name_loc: loc!(),
                params: vec![],
                attributes: vec![],
                return_not_returns: None,
                returns: vec![],
                body: None,
            } => "function name();",
            pt::FunctionDefinition {
                loc_prototype: loc!(),
                ty: pt::FunctionTy::Function,
                name: Some(id("name")),
                name_loc: loc!(),
                params: vec![],
                attributes: vec![],
                return_not_returns: None,
                returns: vec![],
                body: Some(stmt!({})),
            } => "function name() {}",
            pt::FunctionDefinition {
                loc_prototype: loc!(),
                ty: pt::FunctionTy::Function,
                name: Some(id("name")),
                name_loc: loc!(),
                params: vec![],
                attributes: vec![],
                return_not_returns: None,
                returns: vec![(loc!(), Some(param!(uint256)))],
                body: Some(stmt!({})),
            } => "function name() returns (uint256) {}",
            pt::FunctionDefinition {
                loc_prototype: loc!(),
                ty: pt::FunctionTy::Function,
                name: Some(id("name")),
                name_loc: loc!(),
                params: vec![],
                attributes: vec![pt::FunctionAttribute::Virtual(loc!())],
                return_not_returns: None,
                returns: vec![(loc!(), Some(param!(uint256)))],
                body: Some(stmt!({})),
            } => "function name() virtual returns (uint256) {}",

            pt::StructDefinition {
                name: Some(id("name")),
                fields: vec![],
            } => "struct name {}",
            pt::StructDefinition {
                name: Some(id("name")),
                fields: vec![pt::VariableDeclaration {
                    loc: loc!(),
                    ty: expr_ty!(uint256),
                    storage: None,
                    name: Some(id("a")),
                }],
            } => "struct name {uint256 a;}",
            pt::StructDefinition {
                name: Some(id("name")),
                fields: vec![
                    pt::VariableDeclaration {
                        loc: loc!(),
                        ty: expr_ty!(uint256),
                        storage: None,
                        name: Some(id("a")),
                    },
                    pt::VariableDeclaration {
                        loc: loc!(),
                        ty: expr_ty!(uint256),
                        storage: None,
                        name: Some(id("b")),
                    }
                ],
            } => "struct name {uint256 a; uint256 b;}",

            pt::TypeDefinition {
                name: id("MyType"),
                ty: expr_ty!(uint256),
            } => "type MyType is uint256;",

            pt::Using {
                list: pt::UsingList::Library(idp!["id", "path"]),
                ty: None,
                global: None,
            } => "using id.path for *;",
            pt::Using {
                list: pt::UsingList::Library(idp!["id", "path"]),
                ty: Some(expr_ty!(uint256)),
                global: None,
            } => "using id.path for uint256;",
            pt::Using {
                list: pt::UsingList::Library(idp!["id", "path"]),
                ty: Some(expr_ty!(uint256)),
                global: Some(id("global")),
            } => "using id.path for uint256 global;",
            pt::Using {
                list: pt::UsingList::Functions(vec![]),
                ty: None,
                global: None,
            } => "using {} for *;",
            pt::Using {
                list: pt::UsingList::Functions(vec![
                    pt::UsingFunction {
                        loc: loc!(),
                        path: idp!("id", "path"),
                        oper: None,
                    }
                ]),
                ty: None,
                global: None,
            } => "using {id.path} for *;",
            pt::Using {
                list: pt::UsingList::Functions(vec![
                    pt::UsingFunction {
                        loc: loc!(),
                        path: idp!("id", "path"),
                        oper: Some(pt::UserDefinedOperator::Add),
                    }
                ]),
                ty: Some(expr_ty!(uint256)),
                global: None,
            } => "using {id.path as +} for uint256;",
            pt::Using {
                list: pt::UsingList::Functions(vec![
                    pt::UsingFunction {
                        loc: loc!(),
                        path: idp!("id", "path1"),
                        oper: None,
                    },
                    pt::UsingFunction {
                        loc: loc!(),
                        path: idp!("id", "path2"),
                        oper: None,
                    }
                ]),
                ty: Some(expr_ty!(uint256)),
                global: Some(id("global")),
            } => "using {id.path1, id.path2} for uint256 global;",

            pt::YulBlock {
                statements: vec![]
            } => "{}",

            pt::YulFor {
                init_block: yul_block(),
                condition: yexpr!(cond),
                post_block: yul_block(),
                execution_block: yul_block(),
            } => "for {} cond {} {}",

            pt::YulFunctionCall {
                id: id("name"),
                arguments: vec![],
            } => "name()",
            pt::YulFunctionCall {
                id: id("name"),
                arguments: vec![yexpr!(arg)],
            } => "name(arg)",
            pt::YulFunctionCall {
                id: id("name"),
                arguments: vec![yexpr!(arg1), yexpr!(arg2)],
            } => "name(arg1, arg2)",

            pt::YulFunctionDefinition {
                id: id("name"),
                params: vec![],
                returns: vec![],
                body: yul_block(),
            } => "function name() {}",
            pt::YulFunctionDefinition {
                id: id("name"),
                params: vec![yid!(param1: a), yid!(param2: b)],
                returns: vec![],
                body: yul_block(),
            } => "function name(param1: a, param2: b) {}",
            pt::YulFunctionDefinition {
                id: id("name"),
                params: vec![yid!(param1: a), yid!(param2: b)],
                returns: vec![yid!(ret1: c), yid!(ret2: d)],
                body: yul_block(),
            } => "function name(param1: a, param2: b) -> (ret1: c, ret2: d) {}",

            pt::YulSwitch {
                condition: yexpr!(cond),
                cases: vec![pt::YulSwitchOptions::Case(loc!(), yexpr!(expr), yul_block())],
                default: None,
            } => "switch cond case expr {}",
            pt::YulSwitch {
                condition: yexpr!(cond),
                cases: vec![
                    pt::YulSwitchOptions::Case(loc!(), yexpr!(0), yul_block()),
                    pt::YulSwitchOptions::Case(loc!(), yexpr!(1), yul_block()),
                ],
                default: None,
            } => "switch cond case 0 {} case 1 {}",
            pt::YulSwitch {
                condition: yexpr!(cond),
                cases: vec![pt::YulSwitchOptions::Case(loc!(), yexpr!(0), yul_block())],
                default: Some(pt::YulSwitchOptions::Default(loc!(), yul_block())),
            } => "switch cond case 0 {} default {}",
        ];
    }

    #[test]
    fn display_enums() {
        enum_tests![
            // https://docs.soliditylang.org/en/latest/control-structures.html#try-catch
            pt::CatchClause: {
                pt::CatchClause::Named(loc!(), id("Error"), param!(string memory reason), stmt!({}))
                    => "catch Error(string memory reason) {}",
                pt::CatchClause::Named(loc!(), id("Panic"), param!(uint256 errorCode), stmt!({}))
                    => "catch Panic(uint256 errorCode) {}",

                pt::CatchClause::Simple(loc!(), None, stmt!({})) => "catch {}",
                pt::CatchClause::Simple(loc!(), Some(param!(uint256)), stmt!({}))
                    => "catch (uint256) {}",
                pt::CatchClause::Simple(loc!(), Some(param!(bytes memory data)), stmt!({}))
                    => "catch (bytes memory data) {}",
            }

            pt::Comment: {
                pt::Comment::Line(loc!(), "// line".into()) => "// line",
                pt::Comment::Block(loc!(), "/* \nblock\n*/".into()) => "/* \nblock\n*/",
                pt::Comment::DocLine(loc!(), "/// doc line".into()) => "/// doc line",
                pt::Comment::DocBlock(loc!(), "/**\n * doc block\n */".into()) => "/**\n * doc block\n */",
            }

            // tested individually
            pt::ContractPart: {
                pt::ContractPart::StraySemicolon(loc!()) => ";",
            }

            pt::ContractTy: {
                pt::ContractTy::Abstract(loc!()) => "abstract contract",
                pt::ContractTy::Contract(loc!()) => "contract",
                pt::ContractTy::Interface(loc!()) => "interface",
                pt::ContractTy::Library(loc!()) => "library",
            }

            pt::Expression: {
                pt::Expression::New(loc!(), Box::new(expr_ty!(uint256))) => "new uint256",
                pt::Expression::Delete(loc!(), Box::new(expr_ty!(uint256))) => "delete uint256",

                pt::Expression::Type(loc!(), ty!(uint256)) => "uint256",
                pt::Expression::Variable(id("myVar")) => "myVar",

                pt::Expression::ArrayLiteral(loc!(), vec![expr!(1), expr!(2)]) => "[1, 2]",

                pt::Expression::ArraySubscript(loc!(), Box::new(expr!(arr)), None) => "arr[]",
                pt::Expression::ArraySubscript(loc!(), Box::new(expr!(arr)), Some(Box::new(expr!(0)))) => "arr[0]",
                pt::Expression::ArraySlice(loc!(), Box::new(expr!(arr)), None, None) => "arr[:]",
                pt::Expression::ArraySlice(loc!(), Box::new(expr!(arr)), Some(Box::new(expr!(left))), None)
                    => "arr[left:]",
                pt::Expression::ArraySlice(loc!(), Box::new(expr!(arr)), None, Some(Box::new(expr!(right))))
                    => "arr[:right]",
                pt::Expression::ArraySlice(loc!(), Box::new(expr!(arr)), Some(Box::new(expr!(left))), Some(Box::new(expr!(right))))
                    => "arr[left:right]",

                pt::Expression::MemberAccess(loc!(), Box::new(expr!(struct)), id("access")) => "struct.access",

                pt::Expression::Parenthesis(loc!(), Box::new(expr!(var))) => "(var)",
                pt::Expression::List(loc!(), vec![]) => "()",
                pt::Expression::List(loc!(), vec![(loc!(), Some(param!(address)))])
                    => "(address)",
                pt::Expression::List(loc!(), vec![(loc!(), Some(param!(address))), (loc!(), Some(param!(uint256)))])
                    => "(address, uint256)",

                pt::Expression::AddressLiteral(loc!(), "0x1234".into()) => "0x1234",
                pt::Expression::StringLiteral(vec![lit!(unicode "¹²³")]) => "unicode\"¹²³\"",
                pt::Expression::HexLiteral(vec![lit!(hex "00112233")]) => "hex\"00112233\"",
                pt::Expression::BoolLiteral(loc!(), true) => "true",
                pt::Expression::BoolLiteral(loc!(), false) => "false",

                pt::Expression::HexNumberLiteral(loc!(), "0x1234".into(), None) => "0x1234",
                pt::Expression::HexNumberLiteral(loc!(), "0x1234".into(), Some(id("gwei"))) => "0x1234 gwei",
                pt::Expression::NumberLiteral(loc!(), "_123_4_".into(), "".into(), None)
                    => "1234",
                pt::Expression::NumberLiteral(loc!(), "_1_234_".into(), "_2".into(), None)
                    => "1234e2",
                pt::Expression::NumberLiteral(loc!(), "_1_23_4".into(), "".into(), Some(id("gwei")))
                    => "1234 gwei",
                pt::Expression::NumberLiteral(loc!(), "1_23_4_".into(), "2_".into(), Some(id("gwei")))
                    => "1234e2 gwei",
                pt::Expression::RationalNumberLiteral(loc!(), "1_23_4_".into(), "".into(), "".into(), None)
                    => "1234.0",
                pt::Expression::RationalNumberLiteral(loc!(), "_1_23_4".into(), "0".into(), "_2".into(), None)
                    => "1234.0e2",
                pt::Expression::RationalNumberLiteral(loc!(), "_1_234_".into(), "09".into(), "".into(), Some(id("gwei")))
                    => "1234.09 gwei",
                pt::Expression::RationalNumberLiteral(loc!(), "_123_4_".into(), "90".into(), "2_".into(), Some(id("gwei")))
                    => "1234.9e2 gwei",

                pt::Expression::FunctionCall(loc!(), Box::new(expr!(func)), vec![]) => "func()",
                pt::Expression::FunctionCall(loc!(), Box::new(expr!(func)), vec![expr!(arg)])
                    => "func(arg)",
                pt::Expression::FunctionCall(loc!(), Box::new(expr!(func)), vec![expr!(arg1), expr!(arg2)])
                    => "func(arg1, arg2)",
                pt::Expression::FunctionCallBlock(loc!(), Box::new(expr!(func)), Box::new(stmt!({})))
                    => "func{}",
                pt::Expression::NamedFunctionCall(loc!(), Box::new(expr!(func)), vec![])
                    => "func({})",
                pt::Expression::NamedFunctionCall(loc!(), Box::new(expr!(func)), vec![pt::NamedArgument {
                    loc: loc!(),
                    name: id("arg"),
                    expr: expr!(value),
                }]) => "func({arg: value})",
                pt::Expression::NamedFunctionCall(loc!(), Box::new(expr!(func)), vec![
                    pt::NamedArgument {
                        loc: loc!(),
                        name: id("arg1"),
                        expr: expr!(value1),
                    },
                    pt::NamedArgument {
                        loc: loc!(),
                        name: id("arg2"),
                        expr: expr!(value2),
                    }
                ]) => "func({arg1: value1, arg2: value2})",

                pt::Expression::PreIncrement(loc!(), var("a")) => "++a",
                pt::Expression::PostIncrement(loc!(), var("a")) => "a++",
                pt::Expression::PreDecrement(loc!(), var("a")) => "--a",
                pt::Expression::PostDecrement(loc!(), var("a")) => "a--",
                pt::Expression::Not(loc!(), var("a")) => "!a",
                pt::Expression::BitwiseNot(loc!(), var("a")) => "~a",
                pt::Expression::UnaryPlus(loc!(), var("a")) => "+a",
                pt::Expression::Negate(loc!(), var("a")) => "-a",

                pt::Expression::Add(loc!(), var("a"), var("b")) => "a + b",
                pt::Expression::Subtract(loc!(), var("a"), var("b")) => "a - b",
                pt::Expression::Power(loc!(), var("a"), var("b")) => "a ** b",
                pt::Expression::Multiply(loc!(), var("a"), var("b")) => "a * b",
                pt::Expression::Divide(loc!(), var("a"), var("b")) => "a / b",
                pt::Expression::Modulo(loc!(), var("a"), var("b")) => "a % b",
                pt::Expression::ShiftLeft(loc!(), var("a"), var("b")) => "a << b",
                pt::Expression::ShiftRight(loc!(), var("a"), var("b")) => "a >> b",
                pt::Expression::BitwiseAnd(loc!(), var("a"), var("b")) => "a & b",
                pt::Expression::BitwiseXor(loc!(), var("a"), var("b")) => "a ^ b",
                pt::Expression::BitwiseOr(loc!(), var("a"), var("b")) => "a | b",
                pt::Expression::Less(loc!(), var("a"), var("b")) => "a < b",
                pt::Expression::More(loc!(), var("a"), var("b")) => "a > b",
                pt::Expression::LessEqual(loc!(), var("a"), var("b")) => "a <= b",
                pt::Expression::MoreEqual(loc!(), var("a"), var("b")) => "a >= b",
                pt::Expression::And(loc!(), var("a"), var("b")) => "a && b",
                pt::Expression::Or(loc!(), var("a"), var("b")) => "a || b",
                pt::Expression::Equal(loc!(), var("a"), var("b")) => "a == b",
                pt::Expression::NotEqual(loc!(), var("a"), var("b")) => "a != b",

                pt::Expression::Assign(loc!(), var("a"), var("b")) => "a = b",
                pt::Expression::AssignOr(loc!(), var("a"), var("b")) => "a |= b",
                pt::Expression::AssignAnd(loc!(), var("a"), var("b")) => "a &= b",
                pt::Expression::AssignXor(loc!(), var("a"), var("b")) => "a ^= b",
                pt::Expression::AssignShiftLeft(loc!(), var("a"), var("b")) => "a <<= b",
                pt::Expression::AssignShiftRight(loc!(), var("a"), var("b")) => "a >>= b",
                pt::Expression::AssignAdd(loc!(), var("a"), var("b")) => "a += b",
                pt::Expression::AssignSubtract(loc!(), var("a"), var("b")) => "a -= b",
                pt::Expression::AssignMultiply(loc!(), var("a"), var("b")) => "a *= b",
                pt::Expression::AssignDivide(loc!(), var("a"), var("b")) => "a /= b",
                pt::Expression::AssignModulo(loc!(), var("a"), var("b")) => "a %= b",
            }

            pt::FunctionAttribute: {
                pt::FunctionAttribute::Virtual(loc!()) => "virtual",
                pt::FunctionAttribute::Immutable(loc!()) => "immutable",

                pt::FunctionAttribute::Override(loc!(), vec![]) => "override",
                pt::FunctionAttribute::Override(loc!(), vec![idp!["a", "b"]]) => "override(a.b)",
                pt::FunctionAttribute::Override(loc!(), vec![idp!["a", "b"], idp!["c", "d"]])
                    => "override(a.b, c.d)",
            }

            pt::FunctionTy: {
                pt::FunctionTy::Constructor => "constructor",
                pt::FunctionTy::Function => "function",
                pt::FunctionTy::Fallback => "fallback",
                pt::FunctionTy::Receive => "receive",
                pt::FunctionTy::Modifier => "modifier",
            }

            pt::Import: {
                pt::Import::Plain(pt::ImportPath::Filename(lit!("path/to/import")), loc!()) => "import \"path/to/import\";",

                pt::Import::GlobalSymbol(pt::ImportPath::Filename(lit!("path-to-import")), id("ImportedContract"), loc!())
                    => "import \"path-to-import\" as ImportedContract;",

                pt::Import::Rename(pt::ImportPath::Filename(lit!("import\\to\\path")), vec![], loc!())
                    => "import {} from \"import\\to\\path\";",
                pt::Import::Rename(pt::ImportPath::Filename(lit!("import\\to\\path")), vec![(id("A"), None), (id("B"), Some(id("C")))], loc!())
                    => "import {A, B as C} from \"import\\to\\path\";",

                pt::Import::Plain(pt::ImportPath::Path(idp!("std", "stub")), loc!()) => "import std.stub;",

                pt::Import::GlobalSymbol(pt::ImportPath::Path(idp!("a", "b", "c")), id("ImportedContract"), loc!())
                    => "import a.b.c as ImportedContract;",

                pt::Import::Rename(pt::ImportPath::Path(idp!("std", "stub")), vec![], loc!())
                    => "import {} from std.stub;",
                pt::Import::Rename(pt::ImportPath::Path(idp!("std", "stub")), vec![(id("A"), None), (id("B"), Some(id("C")))], loc!())
                    => "import {A, B as C} from std.stub;",
            }

            pt::Mutability: {
                pt::Mutability::Pure(loc!()) => "pure",
                pt::Mutability::View(loc!()) => "view",
                pt::Mutability::Constant(loc!()) => "view",
                pt::Mutability::Payable(loc!()) => "payable",
            }

            pt::SourceUnitPart: {
                // rest tested individually

                pt::SourceUnitPart::PragmaDirective(pt::PragmaDirective::Identifier(loc!(), None, None).into()) => "pragma;",
                pt::SourceUnitPart::PragmaDirective(pt::PragmaDirective::Identifier(loc!(), Some(id("solidity")), None).into())
                    => "pragma solidity;",
                pt::SourceUnitPart::PragmaDirective(pt::PragmaDirective::StringLiteral(loc!(), id("abi"), lit!("v2")).into())
                    => "pragma abi \"v2\";",
                pt::SourceUnitPart::PragmaDirective(pt::PragmaDirective::Version(loc!(), id("solidity"), vec![plain_version!("0", "8", "0")]).into())
                    => "pragma solidity 0.8.0;",
                pt::SourceUnitPart::PragmaDirective(pt::PragmaDirective::Version(loc!(), id("solidity"), vec![
                    op_version!(pt::VersionOp::Exact, "0", "5", "16"),
                    op_version!(pt::VersionOp::GreaterEq, "0", "5"),
                    op_version!(pt::VersionOp::Greater, "0"),
                    op_version!(pt::VersionOp::Less, "1"),
                    op_version!(pt::VersionOp::LessEq, "1"),
                    op_version!(pt::VersionOp::Caret, "0", "5", "16"),
                    op_version!(pt::VersionOp::Wildcard, "5", "5")]
                ).into())
                    => "pragma solidity =0.5.16 >=0.5 >0 <1 <=1 ^0.5.16 *5.5;",
                pt::SourceUnitPart::PragmaDirective(pt::PragmaDirective::Version(loc!(), id("solidity"), vec![or_version!(plain_version!("0"), op_version!(pt::VersionOp::Caret, "1", "0"))]).into())
                    => "pragma solidity 0 || ^1.0;",
                pt::SourceUnitPart::PragmaDirective(pt::PragmaDirective::Version(loc!(), id("solidity"), vec![range_version!(version!["0"], version!["1", "0"])]).into())
                    => "pragma solidity 0 - 1.0;",
                pt::SourceUnitPart::StraySemicolon(loc!()) => ";",
            }

            pt::Statement: {
                pt::Statement::Assembly {
                    loc: loc!(),
                    dialect: None,
                    flags: None,
                    block: yul_block(),
                } => "assembly {}",
                pt::Statement::Assembly {
                    loc: loc!(),
                    dialect: None,
                    flags: Some(vec![lit!("memory-safe")]),
                    block: yul_block(),
                } => "assembly (\"memory-safe\") {}",
                pt::Statement::Assembly {
                    loc: loc!(),
                    dialect: None,
                    flags: Some(vec![lit!("memory-safe"), lit!("second-flag")]),
                    block: yul_block(),
                } => "assembly (\"memory-safe\", \"second-flag\") {}",

                pt::Statement::Args(loc!(), vec![]) => "{}",
                pt::Statement::Args(loc!(), vec![
                    pt::NamedArgument {
                        loc: loc!(),
                        name: id("name"),
                        expr: expr!(value),
                    },
                ]) => "{name: value}",
                pt::Statement::Args(loc!(), vec![
                    pt::NamedArgument {
                        loc: loc!(),
                        name: id("name1"),
                        expr: expr!(value1),
                    },
                    pt::NamedArgument {
                        loc: loc!(),
                        name: id("name2"),
                        expr: expr!(value2),
                    },
                ]) => "{name1: value1, name2: value2}",

                pt::Statement::If(loc!(), expr!(true), Box::new(stmt!({})), None) => "if (true) {}",
                pt::Statement::If(loc!(), expr!(true), Box::new(stmt!({})), Some(Box::new(stmt!({}))))
                    => "if (true) {} else {}",

                pt::Statement::While(loc!(), expr!(true), Box::new(stmt!({}))) => "while (true) {}",

                pt::Statement::Expression(loc!(), expr!(true)) => "true;",

                pt::Statement::VariableDefinition(loc!(), pt::VariableDeclaration {
                    loc: loc!(),
                    ty: expr_ty!(uint256),
                    storage: None,
                    name: Some(id("a")),
                }, None) => "uint256 a;",
                pt::Statement::VariableDefinition(loc!(), pt::VariableDeclaration {
                    loc: loc!(),
                    ty: expr_ty!(uint256),
                    storage: None,
                    name: Some(id("a")),
                }, Some(expr!(0))) => "uint256 a = 0;",

                pt::Statement::For(loc!(), None, None, None, Some(Box::new(stmt!({}))))
                    => "for (;;) {}",
                pt::Statement::For(loc!(), Some(Box::new(pt::Statement::VariableDefinition(
                    loc!(),
                    pt::VariableDeclaration {
                        loc: loc!(),
                        ty: expr_ty!(uint256),
                        storage: None,
                        name: Some(id("a")),
                    },
                    None
                ))), None, None, Some(Box::new(stmt!({}))))
                    => "for (uint256 a;;) {}",
                pt::Statement::For(loc!(), None, Some(Box::new(expr!(true))), None, Some(Box::new(stmt!({}))))
                    => "for (; true;) {}",
                pt::Statement::For(
                    loc!(),
                    None,
                    Some(Box::new(expr!(true))),
                    Some(Box::new(expr!(++i))),
                    Some(Box::new(stmt!({})))
                ) => "for (; true; ++i) {}",

                pt::Statement::DoWhile(loc!(), Box::new(stmt!({})), expr!(true))
                    => "do {} while (true);",

                pt::Statement::Continue(loc!()) => "continue;",
                pt::Statement::Break(loc!()) => "break;",

                pt::Statement::Return(loc!(), None) => "return;",
                pt::Statement::Return(loc!(), Some(expr!(true))) => "return true;",

                pt::Statement::Revert(loc!(), None, vec![]) => "revert();",
                pt::Statement::Revert(loc!(), None, vec![expr!("error")])
                    => "revert(\"error\");",
                pt::Statement::Revert(loc!(), Some(idp!("my", "error")), vec![expr!("error")])
                    => "revert my.error(\"error\");",

                pt::Statement::RevertNamedArgs(loc!(), None, vec![]) => "revert();",
                pt::Statement::RevertNamedArgs(loc!(), None, vec![pt::NamedArgument {
                    loc: loc!(),
                    name: id("name"),
                    expr: expr!(value),
                }]) => "revert({name: value});",
                pt::Statement::RevertNamedArgs(loc!(), Some(idp!("my", "error")), vec![pt::NamedArgument {
                    loc: loc!(),
                    name: id("name"),
                    expr: expr!(value),
                }]) => "revert my.error({name: value});",

                pt::Statement::Emit(loc!(), expr!(true)) => "emit true;",

                pt::Statement::Try(loc!(), expr!(true), None, vec![]) => "try true",
                pt::Statement::Try(loc!(), expr!(true), None, vec![pt::CatchClause::Simple(loc!(), None, stmt!({}))])
                    => "try true catch {}",
                pt::Statement::Try(loc!(), expr!(true), Some((vec![], Box::new(stmt!({})))), vec![])
                    => "try true returns () {}",
                pt::Statement::Try(
                    loc!(),
                    expr!(true),
                    Some((vec![], Box::new(stmt!({})))),
                    vec![pt::CatchClause::Simple(loc!(), None, stmt!({}))]
                ) => "try true returns () {} catch {}",
                pt::Statement::Try(
                    loc!(),
                    expr!(true),
                    Some((vec![(loc!(), Some(param!(uint256 a)))], Box::new(stmt!({})))),
                    vec![pt::CatchClause::Simple(loc!(), None, stmt!({}))]
                ) => "try true returns (uint256 a) {} catch {}",
            }

            pt::StorageLocation: {
                pt::StorageLocation::Memory(loc!()) => "memory",
                pt::StorageLocation::Storage(loc!()) => "storage",
                pt::StorageLocation::Calldata(loc!()) => "calldata",
            }

            pt::Type: {
                pt::Type::Address => "address",
                pt::Type::AddressPayable => "address payable",
                pt::Type::Payable => "payable",
                pt::Type::Bool => "bool",
                pt::Type::String => "string",
                pt::Type::Int(256) => "int256",
                pt::Type::Uint(256) => "uint256",
                pt::Type::Bytes(32) => "bytes32",
                pt::Type::Rational => "fixed",
                pt::Type::DynamicBytes => "bytes",

                pt::Type::Mapping {
                    loc: loc!(),
                    key: Box::new(expr_ty!(uint256)),
                    key_name: None,
                    value: Box::new(expr_ty!(uint256)),
                    value_name: None,
                } => "mapping(uint256 => uint256)",
                pt::Type::Mapping {
                    loc: loc!(),
                    key: Box::new(expr_ty!(uint256)),
                    key_name: Some(id("key")),
                    value: Box::new(expr_ty!(uint256)),
                    value_name: None,
                } => "mapping(uint256 key => uint256)",
                pt::Type::Mapping {
                    loc: loc!(),
                    key: Box::new(expr_ty!(uint256)),
                    key_name: Some(id("key")),
                    value: Box::new(expr_ty!(uint256)),
                    value_name: Some(id("value")),
                } => "mapping(uint256 key => uint256 value)",

                pt::Type::Function {
                    params: vec![],
                    attributes: vec![],
                    returns: None
                } => "function ()",
                pt::Type::Function {
                    params: vec![(loc!(), Some(param!(uint256)))],
                    attributes: vec![],
                    returns: None
                } => "function (uint256)",
                pt::Type::Function {
                    params: vec![(loc!(), Some(param!(uint256))), (loc!(), Some(param!(address)))],
                    attributes: vec![],
                    returns: None
                } => "function (uint256, address)",
                pt::Type::Function {
                    params: vec![(loc!(), Some(param!(uint256)))],
                    attributes: vec![pt::FunctionAttribute::Virtual(loc!())],
                    returns: None
                } => "function (uint256) virtual",
                pt::Type::Function {
                    params: vec![(loc!(), Some(param!(uint256)))],
                    attributes: vec![pt::FunctionAttribute::Virtual(loc!()), pt::FunctionAttribute::Override(loc!(), vec![])],
                    returns: None
                } => "function (uint256) virtual override",
                pt::Type::Function {
                    params: vec![(loc!(), Some(param!(uint256)))],
                    attributes: vec![pt::FunctionAttribute::Virtual(loc!()), pt::FunctionAttribute::Override(loc!(), vec![idp!["a", "b"]])],
                    returns: None
                } => "function (uint256) virtual override(a.b)",
                pt::Type::Function {
                    params: vec![(loc!(), Some(param!(uint256)))],
                    attributes: vec![],
                    returns: Some((vec![], vec![])),
                } => "function (uint256)",
                pt::Type::Function {
                    params: vec![(loc!(), Some(param!(uint256)))],
                    attributes: vec![],
                    returns: Some((vec![(loc!(), Some(param!(uint256)))], vec![])),
                } => "function (uint256) returns (uint256)",
                pt::Type::Function {
                    params: vec![(loc!(), Some(param!(uint256)))],
                    attributes: vec![],
                    returns: Some((vec![(loc!(), Some(param!(uint256))), (loc!(), Some(param!(address)))], vec![])),
                } => "function (uint256) returns (uint256, address)",
            }

            pt::UserDefinedOperator: {
                pt::UserDefinedOperator::BitwiseAnd => "&",
                pt::UserDefinedOperator::BitwiseNot => "~",
                pt::UserDefinedOperator::Negate => "-",
                pt::UserDefinedOperator::BitwiseOr => "|",
                pt::UserDefinedOperator::BitwiseXor => "^",
                pt::UserDefinedOperator::Add => "+",
                pt::UserDefinedOperator::Divide => "/",
                pt::UserDefinedOperator::Modulo => "%",
                pt::UserDefinedOperator::Multiply => "*",
                pt::UserDefinedOperator::Subtract => "-",
                pt::UserDefinedOperator::Equal => "==",
                pt::UserDefinedOperator::More => ">",
                pt::UserDefinedOperator::MoreEqual => ">=",
                pt::UserDefinedOperator::Less => "<",
                pt::UserDefinedOperator::LessEqual => "<=",
                pt::UserDefinedOperator::NotEqual => "!=",
            }

            pt::UsingList: {
                pt::UsingList::Library(idp!("id", "path")) => "id.path",

                pt::UsingList::Functions(vec![]) => "{}",
                pt::UsingList::Functions(vec![
                    pt::UsingFunction {
                        loc: loc!(),
                        path: idp!["id", "path"],
                        oper: None,
                    },
                    pt::UsingFunction {
                        loc: loc!(),
                        path: idp!["id", "path"],
                        oper: Some(pt::UserDefinedOperator::Add),
                }]) => "{id.path, id.path as +}",
            }

            pt::VariableAttribute: {
                pt::VariableAttribute::Constant(loc!()) => "constant",
                pt::VariableAttribute::Immutable(loc!()) => "immutable",

                pt::VariableAttribute::Override(loc!(), vec![]) => "override",
                pt::VariableAttribute::Override(loc!(), vec![idp!["a", "b"]]) => "override(a.b)",
                pt::VariableAttribute::Override(loc!(), vec![idp!["a", "b"], idp!["c", "d"]])
                    => "override(a.b, c.d)",
            }

            pt::Visibility: {
                pt::Visibility::Public(Some(loc!())) => "public",
                pt::Visibility::Internal(Some(loc!())) => "internal",
                pt::Visibility::Private(Some(loc!())) => "private",
                pt::Visibility::External(Some(loc!())) => "external",
            }

            pt::YulExpression: {
                pt::YulExpression::BoolLiteral(loc!(), false, None) => "false",
                pt::YulExpression::BoolLiteral(loc!(), true, None) => "true",
                pt::YulExpression::BoolLiteral(loc!(), false, Some(id("name"))) => "false: name",
                pt::YulExpression::BoolLiteral(loc!(), true, Some(id("name"))) => "true: name",

                pt::YulExpression::NumberLiteral(loc!(), "1234".into(), "".into(), None) => "1234",
                pt::YulExpression::NumberLiteral(loc!(), "1234".into(), "9".into(), None) => "1234e9",
                pt::YulExpression::NumberLiteral(loc!(), "1234".into(), "".into(), Some(id("name"))) => "1234: name",
                pt::YulExpression::NumberLiteral(loc!(), "1234".into(), "9".into(), Some(id("name"))) => "1234e9: name",

                pt::YulExpression::HexNumberLiteral(loc!(), "0x1234".into(), None) => "0x1234",
                pt::YulExpression::HexNumberLiteral(loc!(), "0x1234".into(), Some(id("name"))) => "0x1234: name",

                pt::YulExpression::HexStringLiteral(lit!(hex "1234"), None) => "hex\"1234\"",
                pt::YulExpression::HexStringLiteral(lit!(hex "1234"), Some(id("name"))) => "hex\"1234\": name",

                pt::YulExpression::StringLiteral(lit!("1234"), None) => "\"1234\"",
                pt::YulExpression::StringLiteral(lit!("1234"), Some(id("name"))) => "\"1234\": name",

                pt::YulExpression::Variable(id("name")) => "name",

                pt::YulExpression::FunctionCall(Box::new(pt::YulFunctionCall {
                    loc: loc!(),
                    id: id("name"),
                    arguments: vec![],
                })) => "name()",

                pt::YulExpression::SuffixAccess(loc!(), Box::new(yexpr!(struct)), id("access"))
                    => "struct.access",
            }

            pt::YulStatement: {
                // rest tested individually

                pt::YulStatement::Assign(loc!(), vec![yexpr!(var)], yexpr!(eq))
                    => "var := eq",
                pt::YulStatement::Assign(loc!(), vec![yexpr!(a), yexpr!(b)], yexpr!(eq))
                    => "a, b := eq",

                pt::YulStatement::VariableDeclaration(loc!(), vec![yid!(var)], None)
                    => "let var",
                pt::YulStatement::VariableDeclaration(loc!(), vec![yid!(a), yid!(b)], None)
                    => "let a, b",
                pt::YulStatement::VariableDeclaration(loc!(), vec![yid!(var)], Some(yexpr!(eq)))
                    => "let var := eq",
                pt::YulStatement::VariableDeclaration(loc!(), vec![yid!(a), yid!(b)], Some(yexpr!(eq)))
                    => "let a, b := eq",

                pt::YulStatement::If(loc!(), yexpr!(expr), yul_block()) => "if expr {}",

                pt::YulStatement::Leave(loc!()) => "leave",
                pt::YulStatement::Break(loc!()) => "break",
                pt::YulStatement::Continue(loc!()) => "continue",
            }

            pt::YulSwitchOptions: {
                pt::YulSwitchOptions::Case(loc!(), yexpr!(expr), yul_block()) => "case expr {}",
                pt::YulSwitchOptions::Default(loc!(), yul_block()) => "default {}",
            }
        ];
    }
}
