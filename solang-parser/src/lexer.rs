// SPDX-License-Identifier: Apache-2.0

//! Custom Solidity lexer.
//!
//! Solidity needs a custom lexer for two reasons:
//!  - comments and doc comments
//!  - pragma value is [^;]+

use crate::pt::{Comment, Loc};
use itertools::{peek_nth, PeekNth};
use phf::phf_map;
use std::{fmt, str::CharIndices};
use thiserror::Error;
use unicode_xid::UnicodeXID;

/// A spanned [Token].
pub type Spanned<'a> = (usize, Token<'a>, usize);

/// [Lexer]'s Result type.
pub type Result<'a, T = Spanned<'a>, E = LexicalError> = std::result::Result<T, E>;

/// A Solidity lexical token. Produced by [Lexer].
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[allow(missing_docs)]
pub enum Token<'input> {
    Identifier(&'input str),
    /// `(unicode, literal)`
    StringLiteral(bool, &'input str),
    AddressLiteral(&'input str),
    HexLiteral(&'input str),
    /// `(number, exponent)`
    Number(&'input str, &'input str),
    /// `(number, fraction, exponent)`
    RationalNumber(&'input str, &'input str, &'input str),
    HexNumber(&'input str),
    Divide,
    Contract,
    Library,
    Interface,
    Function,
    Pragma,
    Import,

    Struct,
    Event,
    Enum,
    Type,

    Memory,
    Storage,
    Calldata,

    Public,
    Private,
    Internal,
    External,

    Constant,

    New,
    Delete,

    Pure,
    View,
    Payable,

    Do,
    Continue,
    Break,

    Throw,
    Emit,
    Return,
    Returns,
    Revert,

    Uint(u16),
    Int(u16),
    Bytes(u8),
    // prior to 0.8.0 `byte` used to be an alias for `bytes1`
    Byte,
    DynamicBytes,
    Bool,
    Address,
    String,

    Semicolon,
    Comma,
    OpenParenthesis,
    CloseParenthesis,
    OpenCurlyBrace,
    CloseCurlyBrace,

    BitwiseOr,
    BitwiseOrAssign,
    Or,

    BitwiseXor,
    BitwiseXorAssign,

    BitwiseAnd,
    BitwiseAndAssign,
    And,

    AddAssign,
    Increment,
    Add,

    SubtractAssign,
    Decrement,
    Subtract,

    MulAssign,
    Mul,
    Power,
    DivideAssign,
    ModuloAssign,
    Modulo,

    Equal,
    Assign,
    ColonAssign,

    NotEqual,
    Not,

    True,
    False,
    Else,
    Anonymous,
    For,
    While,
    If,

    ShiftRight,
    ShiftRightAssign,
    Less,
    LessEqual,

    ShiftLeft,
    ShiftLeftAssign,
    More,
    MoreEqual,

    Constructor,
    Indexed,

    Member,
    Colon,
    OpenBracket,
    CloseBracket,
    BitwiseNot,
    Question,

    Mapping,
    Arrow,

    Try,
    Catch,

    Receive,
    Fallback,

    As,
    Is,
    Abstract,
    Virtual,
    Override,
    Using,
    Modifier,
    Immutable,
    Unchecked,

    Assembly,
    Let,
    Leave,
    Switch,
    Case,
    Default,
    YulArrow,

    // Storage types for Soroban
    Persistent,
    Temporary,
    Instance,

    Annotation(&'input str),
}

impl<'input> fmt::Display for Token<'input> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Identifier(id) => write!(f, "{id}"),
            Token::StringLiteral(false, s) => write!(f, "\"{s}\""),
            Token::StringLiteral(true, s) => write!(f, "unicode\"{s}\""),
            Token::HexLiteral(hex) => write!(f, "{hex}"),
            Token::AddressLiteral(address) => write!(f, "{address}"),
            Token::Number(integer, "") => write!(f, "{integer}"),
            Token::Number(integer, exp) => write!(f, "{integer}e{exp}"),
            Token::RationalNumber(integer, fraction, "") => {
                write!(f, "{integer}.{fraction}")
            }
            Token::RationalNumber(integer, fraction, exp) => {
                write!(f, "{integer}.{fraction}e{exp}")
            }
            Token::HexNumber(n) => write!(f, "{n}"),
            Token::Uint(w) => write!(f, "uint{w}"),
            Token::Int(w) => write!(f, "int{w}"),
            Token::Bytes(w) => write!(f, "bytes{w}"),
            Token::Byte => write!(f, "byte"),
            Token::DynamicBytes => write!(f, "bytes"),
            Token::Semicolon => write!(f, ";"),
            Token::Comma => write!(f, ","),
            Token::OpenParenthesis => write!(f, "("),
            Token::CloseParenthesis => write!(f, ")"),
            Token::OpenCurlyBrace => write!(f, "{{"),
            Token::CloseCurlyBrace => write!(f, "}}"),
            Token::BitwiseOr => write!(f, "|"),
            Token::BitwiseOrAssign => write!(f, "|="),
            Token::Or => write!(f, "||"),
            Token::BitwiseXor => write!(f, "^"),
            Token::BitwiseXorAssign => write!(f, "^="),
            Token::BitwiseAnd => write!(f, "&"),
            Token::BitwiseAndAssign => write!(f, "&="),
            Token::And => write!(f, "&&"),
            Token::AddAssign => write!(f, "+="),
            Token::Increment => write!(f, "++"),
            Token::Add => write!(f, "+"),
            Token::SubtractAssign => write!(f, "-="),
            Token::Decrement => write!(f, "--"),
            Token::Subtract => write!(f, "-"),
            Token::MulAssign => write!(f, "*="),
            Token::Mul => write!(f, "*"),
            Token::Power => write!(f, "**"),
            Token::Divide => write!(f, "/"),
            Token::DivideAssign => write!(f, "/="),
            Token::ModuloAssign => write!(f, "%="),
            Token::Modulo => write!(f, "%"),
            Token::Equal => write!(f, "=="),
            Token::Assign => write!(f, "="),
            Token::ColonAssign => write!(f, ":="),
            Token::NotEqual => write!(f, "!="),
            Token::Not => write!(f, "!"),
            Token::ShiftLeft => write!(f, "<<"),
            Token::ShiftLeftAssign => write!(f, "<<="),
            Token::More => write!(f, ">"),
            Token::MoreEqual => write!(f, ">="),
            Token::Member => write!(f, "."),
            Token::Colon => write!(f, ":"),
            Token::OpenBracket => write!(f, "["),
            Token::CloseBracket => write!(f, "]"),
            Token::BitwiseNot => write!(f, "~"),
            Token::Question => write!(f, "?"),
            Token::ShiftRightAssign => write!(f, ">>="),
            Token::ShiftRight => write!(f, ">>"),
            Token::Less => write!(f, "<"),
            Token::LessEqual => write!(f, "<="),
            Token::Bool => write!(f, "bool"),
            Token::Address => write!(f, "address"),
            Token::String => write!(f, "string"),
            Token::Contract => write!(f, "contract"),
            Token::Library => write!(f, "library"),
            Token::Interface => write!(f, "interface"),
            Token::Function => write!(f, "function"),
            Token::Pragma => write!(f, "pragma"),
            Token::Import => write!(f, "import"),
            Token::Struct => write!(f, "struct"),
            Token::Event => write!(f, "event"),
            Token::Enum => write!(f, "enum"),
            Token::Type => write!(f, "type"),
            Token::Memory => write!(f, "memory"),
            Token::Storage => write!(f, "storage"),
            Token::Calldata => write!(f, "calldata"),
            Token::Public => write!(f, "public"),
            Token::Private => write!(f, "private"),
            Token::Internal => write!(f, "internal"),
            Token::External => write!(f, "external"),
            Token::Constant => write!(f, "constant"),
            Token::New => write!(f, "new"),
            Token::Delete => write!(f, "delete"),
            Token::Pure => write!(f, "pure"),
            Token::View => write!(f, "view"),
            Token::Payable => write!(f, "payable"),
            Token::Do => write!(f, "do"),
            Token::Continue => write!(f, "continue"),
            Token::Break => write!(f, "break"),
            Token::Throw => write!(f, "throw"),
            Token::Emit => write!(f, "emit"),
            Token::Return => write!(f, "return"),
            Token::Returns => write!(f, "returns"),
            Token::Revert => write!(f, "revert"),
            Token::True => write!(f, "true"),
            Token::False => write!(f, "false"),
            Token::Else => write!(f, "else"),
            Token::Anonymous => write!(f, "anonymous"),
            Token::For => write!(f, "for"),
            Token::While => write!(f, "while"),
            Token::If => write!(f, "if"),
            Token::Constructor => write!(f, "constructor"),
            Token::Indexed => write!(f, "indexed"),
            Token::Mapping => write!(f, "mapping"),
            Token::Arrow => write!(f, "=>"),
            Token::Try => write!(f, "try"),
            Token::Catch => write!(f, "catch"),
            Token::Receive => write!(f, "receive"),
            Token::Fallback => write!(f, "fallback"),
            Token::As => write!(f, "as"),
            Token::Is => write!(f, "is"),
            Token::Abstract => write!(f, "abstract"),
            Token::Virtual => write!(f, "virtual"),
            Token::Override => write!(f, "override"),
            Token::Using => write!(f, "using"),
            Token::Modifier => write!(f, "modifier"),
            Token::Immutable => write!(f, "immutable"),
            Token::Unchecked => write!(f, "unchecked"),
            Token::Assembly => write!(f, "assembly"),
            Token::Let => write!(f, "let"),
            Token::Leave => write!(f, "leave"),
            Token::Switch => write!(f, "switch"),
            Token::Case => write!(f, "case"),
            Token::Default => write!(f, "default"),
            Token::YulArrow => write!(f, "->"),
            Token::Annotation(name) => write!(f, "@{name}"),
            Token::Persistent => write!(f, "persistent"),
            Token::Temporary => write!(f, "temporary"),
            Token::Instance => write!(f, "instance"),
        }
    }
}

/// Custom Solidity lexer.
///
/// # Examples
///
/// ```
/// use solang_parser::lexer::{Lexer, Token};
///
/// let source = "uint256 number = 0;";
/// let mut comments = Vec::new();
/// let mut errors = Vec::new();
/// let mut lexer = Lexer::new(source, 0, &mut comments, &mut errors);
///
/// let mut next_token = || lexer.next().map(|(_, token, _)| token);
/// assert_eq!(next_token(), Some(Token::Uint(256)));
/// assert_eq!(next_token(), Some(Token::Identifier("number")));
/// assert_eq!(next_token(), Some(Token::Assign));
/// assert_eq!(next_token(), Some(Token::Number("0", "")));
/// assert_eq!(next_token(), Some(Token::Semicolon));
/// assert_eq!(next_token(), None);
/// assert!(errors.is_empty());
/// assert!(comments.is_empty());
/// ```
#[derive(Debug)]
pub struct Lexer<'input> {
    input: &'input str,
    chars: PeekNth<CharIndices<'input>>,
    comments: &'input mut Vec<Comment>,
    file_no: usize,
    /// While parsing version semver, do not parse rational numbers
    parse_semver: bool,
    last_tokens: [Option<Token<'input>>; 2],
    /// The mutable reference to the error vector.
    pub errors: &'input mut Vec<LexicalError>,
}

/// An error thrown by [Lexer].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[allow(missing_docs)]
pub enum LexicalError {
    #[error("end of file found in comment")]
    EndOfFileInComment(Loc),

    #[error("end of file found in string literal")]
    EndOfFileInString(Loc),

    #[error("end of file found in hex literal string")]
    EndofFileInHex(Loc),

    #[error("missing number")]
    MissingNumber(Loc),

    #[error("invalid character '{1}' in hex literal string")]
    InvalidCharacterInHexLiteral(Loc, char),

    #[error("unrecognised token '{1}'")]
    UnrecognisedToken(Loc, String),

    #[error("missing exponent")]
    MissingExponent(Loc),

    #[error("'{1}' found where 'from' expected")]
    ExpectedFrom(Loc, String),
}

/// Returns whether `word` is a keyword in Solidity.
pub fn is_keyword(word: &str) -> bool {
    KEYWORDS.contains_key(word)
}

static KEYWORDS: phf::Map<&'static str, Token> = phf_map! {
    "address" => Token::Address,
    "anonymous" => Token::Anonymous,
    "bool" => Token::Bool,
    "break" => Token::Break,
    "bytes1" => Token::Bytes(1),
    "bytes2" => Token::Bytes(2),
    "bytes3" => Token::Bytes(3),
    "bytes4" => Token::Bytes(4),
    "bytes5" => Token::Bytes(5),
    "bytes6" => Token::Bytes(6),
    "bytes7" => Token::Bytes(7),
    "bytes8" => Token::Bytes(8),
    "bytes9" => Token::Bytes(9),
    "bytes10" => Token::Bytes(10),
    "bytes11" => Token::Bytes(11),
    "bytes12" => Token::Bytes(12),
    "bytes13" => Token::Bytes(13),
    "bytes14" => Token::Bytes(14),
    "bytes15" => Token::Bytes(15),
    "bytes16" => Token::Bytes(16),
    "bytes17" => Token::Bytes(17),
    "bytes18" => Token::Bytes(18),
    "bytes19" => Token::Bytes(19),
    "bytes20" => Token::Bytes(20),
    "bytes21" => Token::Bytes(21),
    "bytes22" => Token::Bytes(22),
    "bytes23" => Token::Bytes(23),
    "bytes24" => Token::Bytes(24),
    "bytes25" => Token::Bytes(25),
    "bytes26" => Token::Bytes(26),
    "bytes27" => Token::Bytes(27),
    "bytes28" => Token::Bytes(28),
    "bytes29" => Token::Bytes(29),
    "bytes30" => Token::Bytes(30),
    "bytes31" => Token::Bytes(31),
    "bytes32" => Token::Bytes(32),
    "bytes" => Token::DynamicBytes,
    "byte" => Token::Byte,
    "calldata" => Token::Calldata,
    "case" => Token::Case,
    "constant" => Token::Constant,
    "constructor" => Token::Constructor,
    "continue" => Token::Continue,
    "contract" => Token::Contract,
    "default" => Token::Default,
    "delete" => Token::Delete,
    "do" => Token::Do,
    "else" => Token::Else,
    "emit" => Token::Emit,
    "enum" => Token::Enum,
    "event" => Token::Event,
    "external" => Token::External,
    "false" => Token::False,
    "for" => Token::For,
    "function" => Token::Function,
    "if" => Token::If,
    "import" => Token::Import,
    "indexed" => Token::Indexed,
    "int8" => Token::Int(8),
    "int16" => Token::Int(16),
    "int24" => Token::Int(24),
    "int32" => Token::Int(32),
    "int40" => Token::Int(40),
    "int48" => Token::Int(48),
    "int56" => Token::Int(56),
    "int64" => Token::Int(64),
    "int72" => Token::Int(72),
    "int80" => Token::Int(80),
    "int88" => Token::Int(88),
    "int96" => Token::Int(96),
    "int104" => Token::Int(104),
    "int112" => Token::Int(112),
    "int120" => Token::Int(120),
    "int128" => Token::Int(128),
    "int136" => Token::Int(136),
    "int144" => Token::Int(144),
    "int152" => Token::Int(152),
    "int160" => Token::Int(160),
    "int168" => Token::Int(168),
    "int176" => Token::Int(176),
    "int184" => Token::Int(184),
    "int192" => Token::Int(192),
    "int200" => Token::Int(200),
    "int208" => Token::Int(208),
    "int216" => Token::Int(216),
    "int224" => Token::Int(224),
    "int232" => Token::Int(232),
    "int240" => Token::Int(240),
    "int248" => Token::Int(248),
    "int256" => Token::Int(256),
    "interface" => Token::Interface,
    "internal" => Token::Internal,
    "int" => Token::Int(256),
    "leave" => Token::Leave,
    "library" => Token::Library,
    "mapping" => Token::Mapping,
    "memory" => Token::Memory,
    "new" => Token::New,
    "payable" => Token::Payable,
    "pragma" => Token::Pragma,
    "private" => Token::Private,
    "public" => Token::Public,
    "pure" => Token::Pure,
    "returns" => Token::Returns,
    "return" => Token::Return,
    "revert" => Token::Revert,
    "storage" => Token::Storage,
    "string" => Token::String,
    "struct" => Token::Struct,
    "switch" => Token::Switch,
    "throw" => Token::Throw,
    "true" => Token::True,
    "type" => Token::Type,
    "uint8" => Token::Uint(8),
    "uint16" => Token::Uint(16),
    "uint24" => Token::Uint(24),
    "uint32" => Token::Uint(32),
    "uint40" => Token::Uint(40),
    "uint48" => Token::Uint(48),
    "uint56" => Token::Uint(56),
    "uint64" => Token::Uint(64),
    "uint72" => Token::Uint(72),
    "uint80" => Token::Uint(80),
    "uint88" => Token::Uint(88),
    "uint96" => Token::Uint(96),
    "uint104" => Token::Uint(104),
    "uint112" => Token::Uint(112),
    "uint120" => Token::Uint(120),
    "uint128" => Token::Uint(128),
    "uint136" => Token::Uint(136),
    "uint144" => Token::Uint(144),
    "uint152" => Token::Uint(152),
    "uint160" => Token::Uint(160),
    "uint168" => Token::Uint(168),
    "uint176" => Token::Uint(176),
    "uint184" => Token::Uint(184),
    "uint192" => Token::Uint(192),
    "uint200" => Token::Uint(200),
    "uint208" => Token::Uint(208),
    "uint216" => Token::Uint(216),
    "uint224" => Token::Uint(224),
    "uint232" => Token::Uint(232),
    "uint240" => Token::Uint(240),
    "uint248" => Token::Uint(248),
    "uint256" => Token::Uint(256),
    "uint" => Token::Uint(256),
    "view" => Token::View,
    "while" => Token::While,
    "try" => Token::Try,
    "catch" => Token::Catch,
    "receive" => Token::Receive,
    "fallback" => Token::Fallback,
    "as" => Token::As,
    "is" => Token::Is,
    "abstract" => Token::Abstract,
    "virtual" => Token::Virtual,
    "override" => Token::Override,
    "using" => Token::Using,
    "modifier" => Token::Modifier,
    "immutable" => Token::Immutable,
    "unchecked" => Token::Unchecked,
    "assembly" => Token::Assembly,
    "let" => Token::Let,
    "persistent" => Token::Persistent,
    "temporary" => Token::Temporary,
    "instance" => Token::Instance,
};

impl<'input> Lexer<'input> {
    /// Instantiates a new Lexer.
    ///
    /// # Examples
    ///
    /// ```
    /// use solang_parser::lexer::Lexer;
    ///
    /// let source = "uint256 number = 0;";
    /// let mut comments = Vec::new();
    /// let mut errors = Vec::new();
    /// let mut lexer = Lexer::new(source, 0, &mut comments, &mut errors);
    /// ```
    pub fn new(
        input: &'input str,
        file_no: usize,
        comments: &'input mut Vec<Comment>,
        errors: &'input mut Vec<LexicalError>,
    ) -> Self {
        Lexer {
            input,
            chars: peek_nth(input.char_indices()),
            comments,
            file_no,
            parse_semver: false,
            last_tokens: [None, None],
            errors,
        }
    }

    fn parse_number(&mut self, mut start: usize, ch: char) -> Result<'input> {
        let mut is_rational = false;
        if ch == '0' {
            if let Some((_, 'x')) = self.chars.peek() {
                // hex number
                self.chars.next();

                let mut end = match self.chars.next() {
                    Some((end, ch)) if ch.is_ascii_hexdigit() => end,
                    Some((..)) => {
                        return Err(LexicalError::MissingNumber(Loc::File(
                            self.file_no,
                            start,
                            start + 1,
                        )));
                    }
                    None => {
                        return Err(LexicalError::EndofFileInHex(Loc::File(
                            self.file_no,
                            start,
                            self.input.len(),
                        )));
                    }
                };

                while let Some((i, ch)) = self.chars.peek() {
                    if !ch.is_ascii_hexdigit() && *ch != '_' {
                        break;
                    }
                    end = *i;
                    self.chars.next();
                }

                return Ok((start, Token::HexNumber(&self.input[start..=end]), end + 1));
            }
        }

        if ch == '.' {
            is_rational = true;
            start -= 1;
        }

        let mut end = start;
        while let Some((i, ch)) = self.chars.peek() {
            if !ch.is_ascii_digit() && *ch != '_' {
                break;
            }
            end = *i;
            self.chars.next();
        }

        if self.parse_semver {
            let integer = &self.input[start..=end];
            let exp = &self.input[0..0];

            return Ok((start, Token::Number(integer, exp), end + 1));
        }

        let mut rational_end = end;
        let mut end_before_rational = end + 1;
        let mut rational_start = end;
        if is_rational {
            end_before_rational = start;
            rational_start = start + 1;
        }

        if let Some((_, '.')) = self.chars.peek() {
            if let Some((i, ch)) = self.chars.peek_nth(1) {
                if ch.is_ascii_digit() && !is_rational {
                    rational_start = *i;
                    rational_end = *i;
                    is_rational = true;
                    self.chars.next(); // advance over '.'
                    while let Some((i, ch)) = self.chars.peek() {
                        if !ch.is_ascii_digit() && *ch != '_' {
                            break;
                        }
                        rational_end = *i;
                        end = *i;
                        self.chars.next();
                    }
                }
            }
        }

        let old_end = end;
        let mut exp_start = end + 1;

        if let Some((i, 'e' | 'E')) = self.chars.peek() {
            exp_start = *i + 1;
            self.chars.next();
            // Negative exponent
            while matches!(self.chars.peek(), Some((_, '-'))) {
                self.chars.next();
            }
            while let Some((i, ch)) = self.chars.peek() {
                if !ch.is_ascii_digit() && *ch != '_' {
                    break;
                }
                end = *i;
                self.chars.next();
            }

            if exp_start > end {
                return Err(LexicalError::MissingExponent(Loc::File(
                    self.file_no,
                    start,
                    self.input.len(),
                )));
            }
        }

        if is_rational {
            let integer = &self.input[start..end_before_rational];
            let fraction = &self.input[rational_start..=rational_end];
            let exp = &self.input[exp_start..=end];

            return Ok((
                start,
                Token::RationalNumber(integer, fraction, exp),
                end + 1,
            ));
        }

        let integer = &self.input[start..=old_end];
        let exp = &self.input[exp_start..=end];

        Ok((start, Token::Number(integer, exp), end + 1))
    }

    fn string(
        &mut self,
        unicode: bool,
        token_start: usize,
        string_start: usize,
        quote_char: char,
    ) -> Result<'input> {
        let mut end;

        let mut last_was_escape = false;

        loop {
            if let Some((i, ch)) = self.chars.next() {
                end = i;
                if !last_was_escape {
                    if ch == quote_char {
                        break;
                    }
                    last_was_escape = ch == '\\';
                } else {
                    last_was_escape = false;
                }
            } else {
                return Err(LexicalError::EndOfFileInString(Loc::File(
                    self.file_no,
                    token_start,
                    self.input.len(),
                )));
            }
        }

        Ok((
            token_start,
            Token::StringLiteral(unicode, &self.input[string_start..end]),
            end + 1,
        ))
    }

    fn next(&mut self) -> Option<Spanned<'input>> {
        'toplevel: loop {
            match self.chars.next() {
                Some((start, ch)) if ch == '_' || ch == '$' || UnicodeXID::is_xid_start(ch) => {
                    let (id, end) = self.match_identifier(start);

                    if id == "unicode" {
                        match self.chars.peek() {
                            Some((_, quote_char @ '"')) | Some((_, quote_char @ '\'')) => {
                                let quote_char = *quote_char;

                                self.chars.next();
                                let str_res = self.string(true, start, start + 8, quote_char);
                                match str_res {
                                    Err(lex_err) => self.errors.push(lex_err),
                                    Ok(val) => return Some(val),
                                }
                            }
                            _ => (),
                        }
                    }

                    if id == "hex" {
                        match self.chars.peek() {
                            Some((_, quote_char @ '"')) | Some((_, quote_char @ '\'')) => {
                                let quote_char = *quote_char;

                                self.chars.next();

                                for (i, ch) in &mut self.chars {
                                    if ch == quote_char {
                                        return Some((
                                            start,
                                            Token::HexLiteral(&self.input[start..=i]),
                                            i + 1,
                                        ));
                                    }

                                    if !ch.is_ascii_hexdigit() && ch != '_' {
                                        // Eat up the remainer of the string
                                        for (_, ch) in &mut self.chars {
                                            if ch == quote_char {
                                                break;
                                            }
                                        }

                                        self.errors.push(
                                            LexicalError::InvalidCharacterInHexLiteral(
                                                Loc::File(self.file_no, i, i + 1),
                                                ch,
                                            ),
                                        );
                                        continue 'toplevel;
                                    }
                                }

                                self.errors.push(LexicalError::EndOfFileInString(Loc::File(
                                    self.file_no,
                                    start,
                                    self.input.len(),
                                )));
                                return None;
                            }
                            _ => (),
                        }
                    }

                    if id == "address" {
                        match self.chars.peek() {
                            Some((_, quote_char @ '"')) | Some((_, quote_char @ '\'')) => {
                                let quote_char = *quote_char;

                                self.chars.next();

                                for (i, ch) in &mut self.chars {
                                    if ch == quote_char {
                                        return Some((
                                            start,
                                            Token::AddressLiteral(&self.input[start..=i]),
                                            i + 1,
                                        ));
                                    }
                                }

                                self.errors.push(LexicalError::EndOfFileInString(Loc::File(
                                    self.file_no,
                                    start,
                                    self.input.len(),
                                )));
                                return None;
                            }
                            _ => (),
                        }
                    }

                    return if let Some(w) = KEYWORDS.get(id) {
                        Some((start, *w, end))
                    } else {
                        Some((start, Token::Identifier(id), end))
                    };
                }
                Some((start, quote_char @ '"')) | Some((start, quote_char @ '\'')) => {
                    let str_res = self.string(false, start, start + 1, quote_char);
                    match str_res {
                        Err(lex_err) => self.errors.push(lex_err),
                        Ok(val) => return Some(val),
                    }
                }
                Some((start, '/')) => {
                    match self.chars.peek() {
                        Some((_, '=')) => {
                            self.chars.next();
                            return Some((start, Token::DivideAssign, start + 2));
                        }
                        Some((_, '/')) => {
                            // line comment
                            self.chars.next();

                            let mut newline = false;

                            let doc_comment = match self.chars.next() {
                                Some((_, '/')) => {
                                    // ///(/)+ is still a line comment
                                    !matches!(self.chars.peek(), Some((_, '/')))
                                }
                                Some((_, ch)) if ch == '\n' || ch == '\r' => {
                                    newline = true;
                                    false
                                }
                                _ => false,
                            };

                            let mut last = start + 3;

                            if !newline {
                                loop {
                                    match self.chars.next() {
                                        None => {
                                            last = self.input.len();
                                            break;
                                        }
                                        Some((offset, '\n' | '\r')) => {
                                            last = offset;
                                            break;
                                        }
                                        Some(_) => (),
                                    }
                                }
                            }

                            if doc_comment {
                                self.comments.push(Comment::DocLine(
                                    Loc::File(self.file_no, start, last),
                                    self.input[start..last].to_owned(),
                                ));
                            } else {
                                self.comments.push(Comment::Line(
                                    Loc::File(self.file_no, start, last),
                                    self.input[start..last].to_owned(),
                                ));
                            }
                        }
                        Some((_, '*')) => {
                            // multiline comment
                            self.chars.next();

                            let doc_comment_start = matches!(self.chars.peek(), Some((_, '*')));

                            let mut last = start + 3;
                            let mut seen_star = false;

                            loop {
                                if let Some((i, ch)) = self.chars.next() {
                                    if seen_star && ch == '/' {
                                        break;
                                    }
                                    seen_star = ch == '*';
                                    last = i;
                                } else {
                                    self.errors.push(LexicalError::EndOfFileInComment(Loc::File(
                                        self.file_no,
                                        start,
                                        self.input.len(),
                                    )));
                                    return None;
                                }
                            }

                            // `/**/` is not a doc comment
                            if doc_comment_start && last > start + 2 {
                                self.comments.push(Comment::DocBlock(
                                    Loc::File(self.file_no, start, last + 2),
                                    self.input[start..last + 2].to_owned(),
                                ));
                            } else {
                                self.comments.push(Comment::Block(
                                    Loc::File(self.file_no, start, last + 2),
                                    self.input[start..last + 2].to_owned(),
                                ));
                            }
                        }
                        _ => {
                            return Some((start, Token::Divide, start + 1));
                        }
                    }
                }
                Some((start, ch)) if ch.is_ascii_digit() => {
                    let parse_result = self.parse_number(start, ch);
                    match parse_result {
                        Err(lex_err) => {
                            self.errors.push(lex_err.clone());
                            if matches!(lex_err, LexicalError::EndofFileInHex(_)) {
                                return None;
                            }
                        }
                        Ok(parse_result) => return Some(parse_result),
                    }
                }
                Some((start, '@')) => {
                    let (id, end) = self.match_identifier(start);
                    if id.len() == 1 {
                        self.errors.push(LexicalError::UnrecognisedToken(
                            Loc::File(self.file_no, start, start + 1),
                            id.to_owned(),
                        ));
                    } else {
                        return Some((start, Token::Annotation(&id[1..]), end));
                    };
                }
                Some((i, ';')) => {
                    self.parse_semver = false;
                    return Some((i, Token::Semicolon, i + 1));
                }
                Some((i, ',')) => return Some((i, Token::Comma, i + 1)),
                Some((i, '(')) => return Some((i, Token::OpenParenthesis, i + 1)),
                Some((i, ')')) => return Some((i, Token::CloseParenthesis, i + 1)),
                Some((i, '{')) => return Some((i, Token::OpenCurlyBrace, i + 1)),
                Some((i, '}')) => return Some((i, Token::CloseCurlyBrace, i + 1)),
                Some((i, '~')) => return Some((i, Token::BitwiseNot, i + 1)),
                Some((i, '=')) => {
                    return match self.chars.peek() {
                        Some((_, '=')) => {
                            self.chars.next();
                            Some((i, Token::Equal, i + 2))
                        }
                        Some((_, '>')) => {
                            self.chars.next();
                            Some((i, Token::Arrow, i + 2))
                        }
                        _ => Some((i, Token::Assign, i + 1)),
                    }
                }
                Some((i, '!')) => {
                    return if let Some((_, '=')) = self.chars.peek() {
                        self.chars.next();
                        Some((i, Token::NotEqual, i + 2))
                    } else {
                        Some((i, Token::Not, i + 1))
                    }
                }
                Some((i, '|')) => {
                    return match self.chars.peek() {
                        Some((_, '=')) => {
                            self.chars.next();
                            Some((i, Token::BitwiseOrAssign, i + 2))
                        }
                        Some((_, '|')) => {
                            self.chars.next();
                            Some((i, Token::Or, i + 2))
                        }
                        _ => Some((i, Token::BitwiseOr, i + 1)),
                    };
                }
                Some((i, '&')) => {
                    return match self.chars.peek() {
                        Some((_, '=')) => {
                            self.chars.next();
                            Some((i, Token::BitwiseAndAssign, i + 2))
                        }
                        Some((_, '&')) => {
                            self.chars.next();
                            Some((i, Token::And, i + 2))
                        }
                        _ => Some((i, Token::BitwiseAnd, i + 1)),
                    };
                }
                Some((i, '^')) => {
                    return match self.chars.peek() {
                        Some((_, '=')) => {
                            self.chars.next();
                            Some((i, Token::BitwiseXorAssign, i + 2))
                        }
                        _ => Some((i, Token::BitwiseXor, i + 1)),
                    };
                }
                Some((i, '+')) => {
                    return match self.chars.peek() {
                        Some((_, '=')) => {
                            self.chars.next();
                            Some((i, Token::AddAssign, i + 2))
                        }
                        Some((_, '+')) => {
                            self.chars.next();
                            Some((i, Token::Increment, i + 2))
                        }
                        _ => Some((i, Token::Add, i + 1)),
                    };
                }
                Some((i, '-')) => {
                    return match self.chars.peek() {
                        Some((_, '=')) => {
                            self.chars.next();
                            Some((i, Token::SubtractAssign, i + 2))
                        }
                        Some((_, '-')) => {
                            self.chars.next();
                            Some((i, Token::Decrement, i + 2))
                        }
                        Some((_, '>')) => {
                            self.chars.next();
                            Some((i, Token::YulArrow, i + 2))
                        }
                        _ => Some((i, Token::Subtract, i + 1)),
                    };
                }
                Some((i, '*')) => {
                    return match self.chars.peek() {
                        Some((_, '=')) => {
                            self.chars.next();
                            Some((i, Token::MulAssign, i + 2))
                        }
                        Some((_, '*')) => {
                            self.chars.next();
                            Some((i, Token::Power, i + 2))
                        }
                        _ => Some((i, Token::Mul, i + 1)),
                    };
                }
                Some((i, '%')) => {
                    return match self.chars.peek() {
                        Some((_, '=')) => {
                            self.chars.next();
                            Some((i, Token::ModuloAssign, i + 2))
                        }
                        _ => Some((i, Token::Modulo, i + 1)),
                    };
                }
                Some((i, '<')) => {
                    return match self.chars.peek() {
                        Some((_, '<')) => {
                            self.chars.next();
                            if let Some((_, '=')) = self.chars.peek() {
                                self.chars.next();
                                Some((i, Token::ShiftLeftAssign, i + 3))
                            } else {
                                Some((i, Token::ShiftLeft, i + 2))
                            }
                        }
                        Some((_, '=')) => {
                            self.chars.next();
                            Some((i, Token::LessEqual, i + 2))
                        }
                        _ => Some((i, Token::Less, i + 1)),
                    };
                }
                Some((i, '>')) => {
                    return match self.chars.peek() {
                        Some((_, '>')) => {
                            self.chars.next();
                            if let Some((_, '=')) = self.chars.peek() {
                                self.chars.next();
                                Some((i, Token::ShiftRightAssign, i + 3))
                            } else {
                                Some((i, Token::ShiftRight, i + 2))
                            }
                        }
                        Some((_, '=')) => {
                            self.chars.next();
                            Some((i, Token::MoreEqual, i + 2))
                        }
                        _ => Some((i, Token::More, i + 1)),
                    };
                }
                Some((i, '.')) => {
                    if let Some((_, a)) = self.chars.peek() {
                        if a.is_ascii_digit() && !self.parse_semver {
                            return match self.parse_number(i + 1, '.') {
                                Err(lex_error) => {
                                    self.errors.push(lex_error);
                                    None
                                }
                                Ok(parse_result) => Some(parse_result),
                            };
                        }
                    }
                    return Some((i, Token::Member, i + 1));
                }
                Some((i, '[')) => return Some((i, Token::OpenBracket, i + 1)),
                Some((i, ']')) => return Some((i, Token::CloseBracket, i + 1)),
                Some((i, ':')) => {
                    return match self.chars.peek() {
                        Some((_, '=')) => {
                            self.chars.next();
                            Some((i, Token::ColonAssign, i + 2))
                        }
                        _ => Some((i, Token::Colon, i + 1)),
                    };
                }
                Some((i, '?')) => return Some((i, Token::Question, i + 1)),
                Some((_, ch)) if ch.is_whitespace() => (),
                Some((start, _)) => {
                    let mut end;

                    loop {
                        if let Some((i, ch)) = self.chars.next() {
                            end = i;

                            if ch.is_whitespace() {
                                break;
                            }
                        } else {
                            end = self.input.len();
                            break;
                        }
                    }

                    self.errors.push(LexicalError::UnrecognisedToken(
                        Loc::File(self.file_no, start, end),
                        self.input[start..end].to_owned(),
                    ));
                }
                None => return None, // End of file
            }
        }
    }

    fn match_identifier(&mut self, start: usize) -> (&'input str, usize) {
        let end;
        loop {
            if let Some((i, ch)) = self.chars.peek() {
                if !UnicodeXID::is_xid_continue(*ch) && *ch != '$' {
                    end = *i;
                    break;
                }
                self.chars.next();
            } else {
                end = self.input.len();
                break;
            }
        }

        (&self.input[start..end], end)
    }
}

impl<'input> Iterator for Lexer<'input> {
    type Item = Spanned<'input>;

    fn next(&mut self) -> Option<Self::Item> {
        // Lexer should be aware of whether the last two tokens were
        // pragma followed by identifier. If this is true, then special parsing should be
        // done for the pragma value
        if let [Some(Token::Pragma), Some(Token::Identifier(_))] = self.last_tokens {
            self.parse_semver = true;
        }

        let token = self.next();

        self.last_tokens = [
            self.last_tokens[1],
            match token {
                Some((_, n, _)) => Some(n),
                _ => None,
            },
        ];

        token
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer() {
        let mut comments = Vec::new();
        let mut errors = Vec::new();

        let multiple_errors = r#" 9ea -9e € bool hex uint8 hex"g"   /**  "#;
        let tokens = Lexer::new(multiple_errors, 0, &mut comments, &mut errors).collect::<Vec<_>>();
        assert_eq!(
            tokens,
            vec![
                (3, Token::Identifier("a"), 4),
                (5, Token::Subtract, 6),
                (13, Token::Bool, 17),
                (18, Token::Identifier("hex"), 21),
                (22, Token::Uint(8), 27),
            ]
        );

        assert_eq!(
            errors,
            vec![
                LexicalError::MissingExponent(Loc::File(0, 1, 42)),
                LexicalError::MissingExponent(Loc::File(0, 6, 42)),
                LexicalError::UnrecognisedToken(Loc::File(0, 9, 12), '€'.to_string()),
                LexicalError::InvalidCharacterInHexLiteral(Loc::File(0, 32, 33), 'g'),
                LexicalError::EndOfFileInComment(Loc::File(0, 37, 42)),
            ]
        );

        let mut errors = Vec::new();
        let tokens = Lexer::new("bool", 0, &mut comments, &mut errors).collect::<Vec<_>>();

        assert_eq!(tokens, vec!((0, Token::Bool, 4)));

        let tokens = Lexer::new("uint8", 0, &mut comments, &mut errors).collect::<Vec<_>>();

        assert_eq!(tokens, vec!((0, Token::Uint(8), 5)));

        let tokens = Lexer::new("hex", 0, &mut comments, &mut errors).collect::<Vec<_>>();

        assert_eq!(tokens, vec!((0, Token::Identifier("hex"), 3)));

        let tokens = Lexer::new(
            "hex\"cafe_dead\" /* adad*** */",
            0,
            &mut comments,
            &mut errors,
        )
        .collect::<Vec<_>>();

        assert_eq!(tokens, vec!((0, Token::HexLiteral("hex\"cafe_dead\""), 14)));

        let tokens = Lexer::new(
            "// foo bar\n0x00fead0_12 00090 0_0",
            0,
            &mut comments,
            &mut errors,
        )
        .collect::<Vec<_>>();

        assert_eq!(
            tokens,
            vec!(
                (11, Token::HexNumber("0x00fead0_12"), 23),
                (24, Token::Number("00090", ""), 29),
                (30, Token::Number("0_0", ""), 33)
            )
        );

        let tokens = Lexer::new(
            "// foo bar\n0x00fead0_12 9.0008 0_0",
            0,
            &mut comments,
            &mut errors,
        )
        .collect::<Vec<_>>();

        assert_eq!(
            tokens,
            vec!(
                (11, Token::HexNumber("0x00fead0_12"), 23),
                (24, Token::RationalNumber("9", "0008", ""), 30),
                (31, Token::Number("0_0", ""), 34)
            )
        );

        let tokens = Lexer::new(
            "// foo bar\n0x00fead0_12 .0008 0.9e2",
            0,
            &mut comments,
            &mut errors,
        )
        .collect::<Vec<_>>();

        assert_eq!(
            tokens,
            vec!(
                (11, Token::HexNumber("0x00fead0_12"), 23),
                (24, Token::RationalNumber("", "0008", ""), 29),
                (30, Token::RationalNumber("0", "9", "2"), 35)
            )
        );

        let tokens = Lexer::new(
            "// foo bar\n0x00fead0_12 .0008 0.9e-2-2",
            0,
            &mut comments,
            &mut errors,
        )
        .collect::<Vec<_>>();

        assert_eq!(
            tokens,
            vec!(
                (11, Token::HexNumber("0x00fead0_12"), 23),
                (24, Token::RationalNumber("", "0008", ""), 29),
                (30, Token::RationalNumber("0", "9", "-2"), 36),
                (36, Token::Subtract, 37),
                (37, Token::Number("2", ""), 38)
            )
        );

        let tokens = Lexer::new("1.2_3e2-", 0, &mut comments, &mut errors).collect::<Vec<_>>();

        assert_eq!(
            tokens,
            vec!(
                (0, Token::RationalNumber("1", "2_3", "2"), 7),
                (7, Token::Subtract, 8)
            )
        );

        let tokens = Lexer::new("\"foo\"", 0, &mut comments, &mut errors).collect::<Vec<_>>();

        assert_eq!(tokens, vec!((0, Token::StringLiteral(false, "foo"), 5)));

        let tokens = Lexer::new(
            "pragma solidity >=0.5.0 <0.7.0;",
            0,
            &mut comments,
            &mut errors,
        )
        .collect::<Vec<_>>();

        assert_eq!(
            tokens,
            vec!(
                (0, Token::Pragma, 6),
                (7, Token::Identifier("solidity"), 15),
                (16, Token::MoreEqual, 18),
                (18, Token::Number("0", ""), 19),
                (19, Token::Member, 20),
                (20, Token::Number("5", ""), 21),
                (21, Token::Member, 22),
                (22, Token::Number("0", ""), 23),
                (24, Token::Less, 25),
                (25, Token::Number("0", ""), 26),
                (26, Token::Member, 27),
                (27, Token::Number("7", ""), 28),
                (28, Token::Member, 29),
                (29, Token::Number("0", ""), 30),
                (30, Token::Semicolon, 31),
            )
        );

        let tokens = Lexer::new(
            "pragma solidity \t>=0.5.0 <0.7.0 \n ;",
            0,
            &mut comments,
            &mut errors,
        )
        .collect::<Vec<_>>();

        assert_eq!(
            tokens,
            vec!(
                (0, Token::Pragma, 6),
                (7, Token::Identifier("solidity"), 15),
                (17, Token::MoreEqual, 19),
                (19, Token::Number("0", ""), 20),
                (20, Token::Member, 21),
                (21, Token::Number("5", ""), 22),
                (22, Token::Member, 23),
                (23, Token::Number("0", ""), 24),
                (25, Token::Less, 26),
                (26, Token::Number("0", ""), 27),
                (27, Token::Member, 28),
                (28, Token::Number("7", ""), 29),
                (29, Token::Member, 30),
                (30, Token::Number("0", ""), 31),
                (34, Token::Semicolon, 35),
            )
        );

        let tokens =
            Lexer::new("pragma solidity 赤;", 0, &mut comments, &mut errors).collect::<Vec<_>>();

        assert_eq!(
            tokens,
            vec!(
                (0, Token::Pragma, 6),
                (7, Token::Identifier("solidity"), 15),
                (16, Token::Identifier("赤"), 19),
                (19, Token::Semicolon, 20)
            )
        );

        let tokens = Lexer::new(">>= >> >= >", 0, &mut comments, &mut errors).collect::<Vec<_>>();

        assert_eq!(
            tokens,
            vec!(
                (0, Token::ShiftRightAssign, 3),
                (4, Token::ShiftRight, 6),
                (7, Token::MoreEqual, 9),
                (10, Token::More, 11),
            )
        );

        let tokens = Lexer::new("<<= << <= <", 0, &mut comments, &mut errors).collect::<Vec<_>>();

        assert_eq!(
            tokens,
            vec!(
                (0, Token::ShiftLeftAssign, 3),
                (4, Token::ShiftLeft, 6),
                (7, Token::LessEqual, 9),
                (10, Token::Less, 11),
            )
        );

        let tokens = Lexer::new("-16 -- - -=", 0, &mut comments, &mut errors).collect::<Vec<_>>();

        assert_eq!(
            tokens,
            vec!(
                (0, Token::Subtract, 1),
                (1, Token::Number("16", ""), 3),
                (4, Token::Decrement, 6),
                (7, Token::Subtract, 8),
                (9, Token::SubtractAssign, 11),
            )
        );

        let tokens = Lexer::new("-4 ", 0, &mut comments, &mut errors).collect::<Vec<_>>();

        assert_eq!(
            tokens,
            vec!((0, Token::Subtract, 1), (1, Token::Number("4", ""), 2),)
        );

        let mut errors = Vec::new();
        let _ = Lexer::new(r#"hex"abcdefg""#, 0, &mut comments, &mut errors).collect::<Vec<_>>();

        assert_eq!(
            errors,
            vec![LexicalError::InvalidCharacterInHexLiteral(
                Loc::File(0, 10, 11),
                'g'
            )]
        );

        let mut errors = Vec::new();
        let _ = Lexer::new(r#" € "#, 0, &mut comments, &mut errors).collect::<Vec<_>>();

        assert_eq!(
            errors,
            vec!(LexicalError::UnrecognisedToken(
                Loc::File(0, 1, 4),
                "€".to_owned()
            ))
        );

        let mut errors = Vec::new();
        let _ = Lexer::new(r#"€"#, 0, &mut comments, &mut errors).collect::<Vec<_>>();

        assert_eq!(
            errors,
            vec!(LexicalError::UnrecognisedToken(
                Loc::File(0, 0, 3),
                "€".to_owned()
            ))
        );

        let tokens =
            Lexer::new(r#"pragma foo bar"#, 0, &mut comments, &mut errors).collect::<Vec<_>>();

        assert_eq!(
            tokens,
            vec!(
                (0, Token::Pragma, 6),
                (7, Token::Identifier("foo"), 10),
                (11, Token::Identifier("bar"), 14),
            )
        );

        comments.truncate(0);

        let tokens = Lexer::new(r#"/// foo"#, 0, &mut comments, &mut errors).count();

        assert_eq!(tokens, 0);
        assert_eq!(
            comments,
            vec![Comment::DocLine(Loc::File(0, 0, 7), "/// foo".to_owned())],
        );

        comments.truncate(0);

        let tokens = Lexer::new("/// jadajadadjada\n// bar", 0, &mut comments, &mut errors).count();

        assert_eq!(tokens, 0);
        assert_eq!(
            comments,
            vec!(
                Comment::DocLine(Loc::File(0, 0, 17), "/// jadajadadjada".to_owned()),
                Comment::Line(Loc::File(0, 18, 24), "// bar".to_owned())
            )
        );

        comments.truncate(0);

        let tokens = Lexer::new("/**/", 0, &mut comments, &mut errors).count();

        assert_eq!(tokens, 0);
        assert_eq!(
            comments,
            vec!(Comment::Block(Loc::File(0, 0, 4), "/**/".to_owned()))
        );

        comments.truncate(0);

        let tokens = Lexer::new(r#"/** foo */"#, 0, &mut comments, &mut errors).count();

        assert_eq!(tokens, 0);
        assert_eq!(
            comments,
            vec!(Comment::DocBlock(
                Loc::File(0, 0, 10),
                "/** foo */".to_owned()
            ))
        );

        comments.truncate(0);

        let tokens = Lexer::new(
            "/** jadajadadjada */\n/* bar */",
            0,
            &mut comments,
            &mut errors,
        )
        .count();

        assert_eq!(tokens, 0);
        assert_eq!(
            comments,
            vec!(
                Comment::DocBlock(Loc::File(0, 0, 20), "/** jadajadadjada */".to_owned()),
                Comment::Block(Loc::File(0, 21, 30), "/* bar */".to_owned())
            )
        );

        let tokens = Lexer::new("/************/", 0, &mut comments, &mut errors).next();
        assert_eq!(tokens, None);

        let mut errors = Vec::new();
        let _ = Lexer::new("/**", 0, &mut comments, &mut errors).next();
        assert_eq!(
            errors,
            vec!(LexicalError::EndOfFileInComment(Loc::File(0, 0, 3)))
        );

        let mut errors = Vec::new();
        let tokens = Lexer::new("//////////////", 0, &mut comments, &mut errors).next();
        assert_eq!(tokens, None);

        // some unicode tests
        let tokens = Lexer::new(
            ">=\u{a0} . très\u{2028}αβγδεζηθικλμνξοπρστυφχψω\u{85}カラス",
            0,
            &mut comments,
            &mut errors,
        )
        .collect::<Vec<_>>();

        assert_eq!(
            tokens,
            vec!(
                (0, Token::MoreEqual, 2),
                (5, Token::Member, 6),
                (7, Token::Identifier("très"), 12),
                (15, Token::Identifier("αβγδεζηθικλμνξοπρστυφχψω"), 63),
                (65, Token::Identifier("カラス"), 74)
            )
        );

        let tokens = Lexer::new(r#"unicode"€""#, 0, &mut comments, &mut errors).collect::<Vec<_>>();

        assert_eq!(tokens, vec!((0, Token::StringLiteral(true, "€"), 12)));

        let tokens =
            Lexer::new(r#"unicode "€""#, 0, &mut comments, &mut errors).collect::<Vec<_>>();

        assert_eq!(
            tokens,
            vec!(
                (0, Token::Identifier("unicode"), 7),
                (8, Token::StringLiteral(false, "€"), 13),
            )
        );

        // scientific notation
        let tokens = Lexer::new(r#" 1e0 "#, 0, &mut comments, &mut errors).collect::<Vec<_>>();

        assert_eq!(tokens, vec!((1, Token::Number("1", "0"), 4)));

        let tokens = Lexer::new(r#" -9e0123"#, 0, &mut comments, &mut errors).collect::<Vec<_>>();

        assert_eq!(
            tokens,
            vec!((1, Token::Subtract, 2), (2, Token::Number("9", "0123"), 8),)
        );

        let mut errors = Vec::new();
        let tokens = Lexer::new(r#" -9e"#, 0, &mut comments, &mut errors).collect::<Vec<_>>();

        assert_eq!(tokens, vec!((1, Token::Subtract, 2)));
        assert_eq!(
            errors,
            vec!(LexicalError::MissingExponent(Loc::File(0, 2, 4)))
        );

        let mut errors = Vec::new();
        let tokens = Lexer::new(r#"9ea"#, 0, &mut comments, &mut errors).collect::<Vec<_>>();

        assert_eq!(tokens, vec!((2, Token::Identifier("a"), 3)));
        assert_eq!(
            errors,
            vec!(LexicalError::MissingExponent(Loc::File(0, 0, 3)))
        );

        let mut errors = Vec::new();
        let tokens = Lexer::new(r#"42.a"#, 0, &mut comments, &mut errors).collect::<Vec<_>>();

        assert_eq!(
            tokens,
            vec!(
                (0, Token::Number("42", ""), 2),
                (2, Token::Member, 3),
                (3, Token::Identifier("a"), 4)
            )
        );

        let tokens = Lexer::new(r#"42..a"#, 0, &mut comments, &mut errors).collect::<Vec<_>>();

        assert_eq!(
            tokens,
            vec!(
                (0, Token::Number("42", ""), 2),
                (2, Token::Member, 3),
                (3, Token::Member, 4),
                (4, Token::Identifier("a"), 5)
            )
        );

        comments.truncate(0);

        let tokens = Lexer::new("/// jadajadadjada\n// bar", 0, &mut comments, &mut errors).count();

        assert_eq!(tokens, 0);
        assert_eq!(
            comments,
            vec!(
                Comment::DocLine(Loc::File(0, 0, 17), "/// jadajadadjada".to_owned()),
                Comment::Line(Loc::File(0, 18, 24), "// bar".to_owned())
            )
        );

        comments.truncate(0);

        let tokens = Lexer::new("/**/", 0, &mut comments, &mut errors).count();

        assert_eq!(tokens, 0);
        assert_eq!(
            comments,
            vec!(Comment::Block(Loc::File(0, 0, 4), "/**/".to_owned()))
        );

        comments.truncate(0);

        let tokens = Lexer::new(r#"/** foo */"#, 0, &mut comments, &mut errors).count();

        assert_eq!(tokens, 0);
        assert_eq!(
            comments,
            vec!(Comment::DocBlock(
                Loc::File(0, 0, 10),
                "/** foo */".to_owned()
            ))
        );

        comments.truncate(0);

        let tokens = Lexer::new(
            "/** jadajadadjada */\n/* bar */",
            0,
            &mut comments,
            &mut errors,
        )
        .count();

        assert_eq!(tokens, 0);
        assert_eq!(
            comments,
            vec!(
                Comment::DocBlock(Loc::File(0, 0, 20), "/** jadajadadjada */".to_owned()),
                Comment::Block(Loc::File(0, 21, 30), "/* bar */".to_owned())
            )
        );

        let tokens = Lexer::new("/************/", 0, &mut comments, &mut errors).next();
        assert_eq!(tokens, None);

        let mut errors = Vec::new();
        let _ = Lexer::new("/**", 0, &mut comments, &mut errors).next();
        assert_eq!(
            errors,
            vec!(LexicalError::EndOfFileInComment(Loc::File(0, 0, 3)))
        );

        let mut errors = Vec::new();
        let tokens = Lexer::new("//////////////", 0, &mut comments, &mut errors).next();
        assert_eq!(tokens, None);

        // some unicode tests
        let tokens = Lexer::new(
            ">=\u{a0} . très\u{2028}αβγδεζηθικλμνξοπρστυφχψω\u{85}カラス",
            0,
            &mut comments,
            &mut errors,
        )
        .collect::<Vec<(usize, Token, usize)>>();

        assert_eq!(
            tokens,
            vec!(
                (0, Token::MoreEqual, 2),
                (5, Token::Member, 6),
                (7, Token::Identifier("très"), 12),
                (15, Token::Identifier("αβγδεζηθικλμνξοπρστυφχψω"), 63),
                (65, Token::Identifier("カラス"), 74)
            )
        );

        let tokens =
            Lexer::new(r#"unicode"€""#, 0, &mut comments, &mut errors)
                .collect::<Vec<(usize, Token, usize)>>();

        assert_eq!(tokens, vec!((0, Token::StringLiteral(true, "€"), 12)));

        let tokens =
            Lexer::new(r#"unicode "€""#, 0, &mut comments, &mut errors)
                .collect::<Vec<(usize, Token, usize)>>();

        assert_eq!(
            tokens,
            vec!(
                (0, Token::Identifier("unicode"), 7),
                (8, Token::StringLiteral(false, "€"), 13),
            )
        );

        // scientific notation
        let tokens =
            Lexer::new(r#" 1e0 "#, 0, &mut comments, &mut errors)
                .collect::<Vec<(usize, Token, usize)>>();

        assert_eq!(tokens, vec!((1, Token::Number("1", "0"), 4)));

        let tokens =
            Lexer::new(r#" -9e0123"#, 0, &mut comments, &mut errors)
                .collect::<Vec<(usize, Token, usize)>>();

        assert_eq!(
            tokens,
            vec!((1, Token::Subtract, 2), (2, Token::Number("9", "0123"), 8),)
        );

        let mut errors = Vec::new();
        let tokens = Lexer::new(r#" -9e"#, 0, &mut comments, &mut errors)
            .collect::<Vec<(usize, Token, usize)>>();

        assert_eq!(tokens, vec!((1, Token::Subtract, 2)));
        assert_eq!(
            errors,
            vec!(LexicalError::MissingExponent(Loc::File(0, 2, 4)))
        );

        let mut errors = Vec::new();
        let tokens = Lexer::new(r#"9ea"#, 0, &mut comments, &mut errors)
            .collect::<Vec<(usize, Token, usize)>>();

        assert_eq!(tokens, vec!((2, Token::Identifier("a"), 3)));
        assert_eq!(
            errors,
            vec!(LexicalError::MissingExponent(Loc::File(0, 0, 3)))
        );

        let mut errors = Vec::new();
        let tokens = Lexer::new(r#"42.a"#, 0, &mut comments, &mut errors)
            .collect::<Vec<(usize, Token, usize)>>();

        assert_eq!(
            tokens,
            vec!(
                (0, Token::Number("42", ""), 2),
                (2, Token::Member, 3),
                (3, Token::Identifier("a"), 4)
            )
        );

        let tokens =
            Lexer::new(r#"42..a"#, 0, &mut comments, &mut errors)
                .collect::<Vec<(usize, Token, usize)>>();

        assert_eq!(
            tokens,
            vec!(
                (0, Token::Number("42", ""), 2),
                (2, Token::Member, 3),
                (3, Token::Member, 4),
                (4, Token::Identifier("a"), 5)
            )
        );

        let mut errors = Vec::new();
        let _ = Lexer::new(r#"hex"g""#, 0, &mut comments, &mut errors)
            .collect::<Vec<(usize, Token, usize)>>();
        assert_eq!(
            errors,
            vec!(LexicalError::InvalidCharacterInHexLiteral(
                Loc::File(0, 4, 5),
                'g'
            ),)
        );

        let mut errors = Vec::new();
        let tokens =
            Lexer::new(".9", 0, &mut comments, &mut errors).collect::<Vec<(usize, Token, usize)>>();

        assert_eq!(tokens, vec!((0, Token::RationalNumber("", "9", ""), 2)));

        let mut errors = Vec::new();
        let tokens = Lexer::new(".9e10", 0, &mut comments, &mut errors)
            .collect::<Vec<(usize, Token, usize)>>();

        assert_eq!(tokens, vec!((0, Token::RationalNumber("", "9", "10"), 5)));

        let mut errors = Vec::new();
        let tokens = Lexer::new(".9", 0, &mut comments, &mut errors).collect::<Vec<_>>();

        assert_eq!(tokens, vec!((0, Token::RationalNumber("", "9", ""), 2)));

        let mut errors = Vec::new();
        let tokens = Lexer::new(".9e10", 0, &mut comments, &mut errors).collect::<Vec<_>>();

        assert_eq!(tokens, vec!((0, Token::RationalNumber("", "9", "10"), 5)));

        errors.clear();
        comments.clear();
        let tokens =
            Lexer::new("@my_annotation", 0, &mut comments, &mut errors).collect::<Vec<_>>();
        assert_eq!(tokens, vec![(0, Token::Annotation("my_annotation"), 14)]);
        assert!(errors.is_empty());
        assert!(comments.is_empty());

        errors.clear();
        comments.clear();
        let tokens =
            Lexer::new("@ my_annotation", 0, &mut comments, &mut errors).collect::<Vec<_>>();
        assert_eq!(tokens, vec![(2, Token::Identifier("my_annotation"), 15)]);
        assert_eq!(
            errors,
            vec![LexicalError::UnrecognisedToken(
                Loc::File(0, 0, 1),
                "@".to_string()
            )]
        );
        assert!(comments.is_empty());
    }
}
