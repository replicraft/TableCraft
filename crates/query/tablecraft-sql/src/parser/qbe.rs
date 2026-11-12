//! Token-based QBE alias prefixes, separate from executable SQL grammar.
use super::{ParseError, ParseErrorKind, ParseLimits, access, lex};
use sqlparser::{
    parser::Parser,
    tokenizer::{Location, Token, Tokenizer},
};

/// Separate an optional QBE `Name: operand` prefix without parsing the operand.
///
/// The alias uses the existing Access identifier grammar. Quoted names, strings
/// and date literals keep their colons. The returned operand borrows its exact
/// source suffix after separator whitespace, including incomplete editable
/// expression text. Consumers must
/// parse and qualify that expression before building or executing a query.
///
/// # Errors
/// Returns input/token budget, alias-identifier errors, or lexical errors before
/// a complete alias prefix. Operand syntax is deferred to Build. This does not
/// accept QBE colon syntax as executable SELECT syntax.
pub fn parse_qbe_field_prefix(
    source: &str,
    limits: ParseLimits,
) -> Result<(Option<String>, &str), ParseError> {
    limits.check_input(source)?;
    let dialect = access::AccessDialect;
    let mut tokens = Vec::new();
    let lexical = Tokenizer::new(&dialect, source).tokenize_with_location_into_buf(&mut tokens);
    tokens.retain(|token| !matches!(token.token, Token::Whitespace(_)));
    if tokens.len() > limits.max_tokens.min(ParseLimits::MAX_TOKENS) {
        return Err(ParseError::new(
            ParseErrorKind::Limit,
            "QBE field token limit exceeded",
            Location { line: 1, column: 1 },
        ));
    }
    tokens.truncate(2);
    let mut parser = Parser::new(&dialect).with_tokens_with_locations(tokens);
    if !matches!(parser.peek_tokens::<2>(), [Token::Word(_), Token::Colon]) {
        lexical.map_err(|error| {
            ParseError::new(
                ParseErrorKind::Syntax,
                "invalid QBE field token",
                error.location,
            )
        })?;
        return Ok((None, source));
    }

    // The operand is deliberately unparsed editor text. A tokenizer error after
    // a complete alias prefix belongs to that suffix and is checked at Build.
    let alias = access::name(&mut parser)?;
    let colon = parser.next_token();
    let operand = &source[lex::byte_offset(source, colon.span.end)..];
    Ok((Some(alias.0), operand.trim_start()))
}

/// Whether identifier contents can be displayed as one unquoted QBE name.
///
/// This uses the Access identifier grammar, not expression-depth semantics.
/// Quoted, reserved or punctuation-bearing contents return `false`.
///
/// # Errors
/// Returns input/token budget errors. Invalid editable contents return `false`.
pub fn qbe_identifier_is_bare(source: &str, limits: ParseLimits) -> Result<bool, ParseError> {
    limits.check_input(source)?;
    let dialect = access::AccessDialect;
    let mut tokens = Vec::new();
    let lexical = Tokenizer::new(&dialect, source).tokenize_with_location_into_buf(&mut tokens);
    tokens.retain(|token| !matches!(token.token, Token::Whitespace(_)));
    if tokens.len() > limits.max_tokens.min(ParseLimits::MAX_TOKENS) {
        return Err(ParseError::new(
            ParseErrorKind::Limit,
            "QBE identifier token limit exceeded",
            Location { line: 1, column: 1 },
        ));
    }
    if lexical.is_err()
        || !matches!(tokens.as_slice(), [token] if matches!(&token.token, Token::Word(word) if word.quote_style.is_none() && word.value == source))
    {
        return Ok(false);
    }
    let mut parser = Parser::new(&dialect).with_tokens_with_locations(tokens);
    match access::name(&mut parser) {
        Ok(_) => Ok(true),
        Err(error) if error.kind == ParseErrorKind::Limit => Err(error),
        Err(_) => Ok(false),
    }
}

#[cfg(test)]
mod tests;
