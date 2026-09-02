// SPDX-License-Identifier: Apache-2.0

#![doc = include_str!("../README.md")]
#![warn(missing_debug_implementations, missing_docs)]

use crate::lexer::LexicalError;
use crate::lexer::Token;
use crate::pt::CodeLocation;
use crate::pt::Loc;
use diagnostics::Diagnostic;
use lalrpop_util::ParseError;

pub mod diagnostics;
pub mod doccomment;
pub mod helpers;
pub mod lexer;
pub mod pt;

#[cfg(test)]
mod tests;

#[allow(
    clippy::needless_lifetimes,
    clippy::type_complexity,
    clippy::ptr_arg,
    clippy::just_underscores_and_digits,
    clippy::empty_line_after_outer_attr,
    clippy::large_enum_variant,
    clippy::uninlined_format_args
)]
mod solidity {
    include!(concat!(env!("OUT_DIR"), "/solidity.rs"));
}

/// Parses a Solidity file.
///
/// Soroban-specific storage type keywords (`persistent`, `temporary`,
/// `instance`) are treated as plain identifiers by this function so that
/// standard Solidity code that uses these words as identifiers parses
/// correctly. Use [`parse_soroban`] when targeting the Soroban platform.
pub fn parse(
    src: &str,
    file_no: usize,
) -> Result<(pt::SourceUnit, Vec<pt::Comment>), Vec<Diagnostic>> {
    parse_impl(src, file_no, false)
}

/// Parses a Solidity file targeting the Soroban platform.
///
/// Enables Soroban-specific storage type keywords (`persistent`, `temporary`,
/// `instance`) so that contract state variables can carry storage-type
/// annotations.
pub fn parse_soroban(
    src: &str,
    file_no: usize,
) -> Result<(pt::SourceUnit, Vec<pt::Comment>), Vec<Diagnostic>> {
    parse_impl(src, file_no, true)
}

fn parse_impl(
    src: &str,
    file_no: usize,
    soroban: bool,
) -> Result<(pt::SourceUnit, Vec<pt::Comment>), Vec<Diagnostic>> {
    // parse phase
    let mut comments = Vec::new();
    let mut lexer_errors = Vec::new();
    let mut lex = lexer::Lexer::new(src, file_no, &mut comments, &mut lexer_errors);

    if soroban {
        lex = lex.with_soroban_keywords();
    }

    let mut parser_errors = Vec::new();
    let res = solidity::SourceUnitParser::new().parse(src, file_no, &mut parser_errors, &mut lex);

    let mut diagnostics = Vec::with_capacity(lex.errors.len() + parser_errors.len());
    for lexical_error in lex.errors {
        diagnostics.push(Diagnostic::parser_error(
            lexical_error.loc(),
            lexical_error.to_string(),
        ))
    }

    for e in parser_errors {
        diagnostics.push(parser_error_to_diagnostic(&e.error, file_no));
    }

    match res {
        Err(e) => {
            diagnostics.push(parser_error_to_diagnostic(&e, file_no));
            Err(diagnostics)
        }
        _ if !diagnostics.is_empty() => Err(diagnostics),
        Ok(res) => Ok((res, comments)),
    }
}

/// Convert lalrop parser error to a Diagnostic
fn parser_error_to_diagnostic(
    error: &ParseError<usize, Token, LexicalError>,
    file_no: usize,
) -> Diagnostic {
    match error {
        ParseError::InvalidToken { location } => Diagnostic::parser_error(
            Loc::File(file_no, *location, *location),
            "invalid token".to_string(),
        ),
        ParseError::UnrecognizedToken {
            token: (l, token, r),
            expected,
        } => Diagnostic::parser_error(
            Loc::File(file_no, *l, *r),
            format!(
                "unrecognised token '{}', expected {}",
                token,
                expected.join(", ")
            ),
        ),
        ParseError::User { error } => Diagnostic::parser_error(error.loc(), error.to_string()),
        ParseError::ExtraToken { token } => Diagnostic::parser_error(
            Loc::File(file_no, token.0, token.2),
            format!("extra token '{}' encountered", token.0),
        ),
        ParseError::UnrecognizedEof { expected, location } => Diagnostic::parser_error(
            Loc::File(file_no, *location, *location),
            format!("unexpected end of file, expecting {}", expected.join(", ")),
        ),
    }
}
