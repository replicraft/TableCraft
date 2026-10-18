use std::fmt;

use sqlparser::{parser::Parser, tokenizer::Location};
use thiserror::Error;

/// A one-based Unicode-character source location, independent of upstream spans.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceLocation {
    /// One-based line.
    pub line: u64,
    /// One-based column, not a UTF-8 byte offset.
    pub column: u64,
}

impl fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// Stable diagnostic categories; unsupported does not mean invalid in Access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseErrorKind {
    /// Malformed input in the supported grammar or an invalid token.
    Syntax,
    /// Syntax or representation outside this slice.
    Unsupported,
    /// Input, token, nesting, expression or parser recursion limit exceeded.
    Limit,
}

/// A typed diagnostic without original SQL, names or literal contents.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message} at {location}")]
pub struct ParseError {
    /// Machine-readable error category.
    pub kind: ParseErrorKind,
    /// Fixed diagnostic text; never includes SQL payloads.
    pub message: &'static str,
    /// Detection point; unsupported expressions point to their start.
    pub location: SourceLocation,
}

impl ParseError {
    pub(super) fn new(kind: ParseErrorKind, message: &'static str, location: Location) -> Self {
        Self {
            kind,
            message,
            location: SourceLocation {
                line: location.line.max(1),
                column: location.column.max(1),
            },
        }
    }

    pub(super) fn at(parser: &Parser<'_>, kind: ParseErrorKind, message: &'static str) -> Self {
        Self::new(kind, message, parser.peek_token().span.start)
    }
}
