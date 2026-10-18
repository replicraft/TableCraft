//! The only module allowed to depend on upstream parser/token/AST types.

mod access;
mod diagnostic;
mod expression;
mod limits;

#[cfg(test)]
mod tests;

pub use diagnostic::{ParseError, ParseErrorKind, SourceLocation};
pub use limits::ParseLimits;

use sqlparser::{parser::Parser, tokenizer::Tokenizer};

use crate::select::Select;

/// Original source and its normalized syntax model, kept separately.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSelect {
    source: String,
    query: Select,
}

impl ParsedSelect {
    /// The exact input, including whitespace and comments if supplied.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The supported owned model; upstream AST types never escape the adapter.
    pub fn query(&self) -> &Select {
        &self.query
    }
}

/// Parse exactly one supported SELECT, with optional trailing semicolon.
///
/// Limits are checked before parsing. Unsupported syntax is an error, never a
/// fallback to another SQL dialect. This function does not evaluate SQL.
pub fn parse_select(source: &str, limits: ParseLimits) -> Result<ParsedSelect, ParseError> {
    limits.check_input(source)?;
    let dialect = access::AccessDialect;
    let tokens = Tokenizer::new(&dialect, source)
        .tokenize_with_location()
        .map_err(|error| {
            ParseError::new(ParseErrorKind::Syntax, "invalid SQL token", error.location)
        })?;
    let tokens = limits.check_tokens(source, tokens)?;
    let mut parser = Parser::new(&dialect)
        .with_recursion_limit(128)
        .with_tokens_with_locations(tokens);
    let query = access::select(&mut parser, limits)?;
    Ok(ParsedSelect {
        source: source.to_owned(),
        query,
    })
}
