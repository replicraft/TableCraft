//! The only module allowed to depend on upstream parser/token types.

use crate::select::Select;
use sqlparser::parser::Parser;

mod access;
mod actions;
mod calls;
mod data_type;
mod definition;
mod diagnostic;
mod expression;
mod identifiers;
mod lex;
mod limits;
mod parameters;
mod precedence;
mod queries;
mod relations;
mod scalar;

#[cfg(test)]
mod tests;

pub use diagnostic::{ParseError, ParseErrorKind, SourceLocation};
pub use limits::ParseLimits;
pub use scalar::{parse_expression, parse_field_rule};

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
    let tokens = lex::tokens(source, &dialect)?;
    let tokens = limits.check_tokens(source, tokens)?;
    let mut parser = Parser::new(&dialect)
        .with_recursion_limit(128)
        .with_tokens_with_locations(tokens);
    let context = limits::ParseContext::new(limits);
    let query = access::select(&mut parser, &context)?;
    Ok(ParsedSelect {
        source: source.to_owned(),
        query,
    })
}

/// Original SQL and its complete owned query statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedQuery {
    source: String,
    query: crate::query::Query,
}
impl ParsedQuery {
    /// Exact original SQL, including trivia and delimiters.
    pub fn source(&self) -> &str {
        &self.source
    }
    /// Owned query syntax; no binding or execution has occurred.
    pub fn query(&self) -> &crate::query::Query {
        &self.query
    }
}
/// Parse one Access query and optional PARAMETERS declaration.
/// Pass-through statements and multiple independent queries are rejected.
pub fn parse_query(source: &str, limits: ParseLimits) -> Result<ParsedQuery, ParseError> {
    let _span = tracing::debug_span!("sql.parse", bytes = source.len()).entered();
    limits.check_input(source)?;
    let dialect = access::AccessDialect;
    let tokens = limits.check_tokens(source, lex::tokens(source, &dialect)?)?;
    let mut parser = Parser::new(&dialect).with_tokens_with_locations(tokens);
    let context = limits::ParseContext::new(limits);
    let query = queries::query(&mut parser, &context)?;
    access::finish(&mut parser)?;
    Ok(ParsedQuery {
        source: source.to_owned(),
        query,
    })
}
