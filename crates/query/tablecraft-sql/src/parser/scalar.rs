use super::{ParseError, ParseErrorKind, ParseLimits, access, expression, lex, limits};
use crate::select::Expression;
use sqlparser::{
    keywords::Keyword,
    parser::Parser,
    tokenizer::{Location, Token, TokenWithSpan, Word},
};

/// Parse exactly one bounded owned expression without binding or evaluation.
/// Calls/subqueries may parse; consumers must enforce their permitted forms.
pub fn parse_expression(source: &str, limits: ParseLimits) -> Result<Expression, ParseError> {
    parse(source, None, limits, true)
}

/// Parse one complete expression without accepting a SQL statement terminator.
/// Semicolons inside strings remain data. Names/calls/subqueries may parse;
/// consumers still own binding, permitted syntax and evaluation.
pub fn parse_expression_strict(
    source: &str,
    limits: ParseLimits,
) -> Result<Expression, ParseError> {
    parse(source, None, limits, false)
}

/// Parse a field validation expression, supplying an implied field before bare
/// comparisons, IS, BETWEEN, IN and LIKE in operand positions. The field name is
/// inserted as one quoted token, never interpolated SQL. Binding/evaluation and
/// field-only reference restrictions belong to the validation consumer.
pub fn parse_field_rule(
    source: &str,
    field: &str,
    limits: ParseLimits,
) -> Result<Expression, ParseError> {
    if field.len() > limits.max_bytes.min(ParseLimits::MAX_BYTES) {
        return Err(ParseError::new(
            ParseErrorKind::Limit,
            "validation field limit exceeded",
            Location { line: 1, column: 1 },
        ));
    }
    if field.is_empty() || field.chars().any(char::is_control) {
        return Err(ParseError::new(
            ParseErrorKind::Unsupported,
            "invalid validation field",
            Location { line: 1, column: 1 },
        ));
    }
    parse(source, Some(field), limits, true)
}

fn parse(
    source: &str,
    field: Option<&str>,
    limits: ParseLimits,
    allow_terminator: bool,
) -> Result<Expression, ParseError> {
    let _span = tracing::debug_span!("sql.expression.parse", bytes = source.len()).entered();
    limits.check_input(source)?;
    let dialect = access::AccessDialect;
    let mut tokens = limits.check_tokens(source, lex::tokens(source, &dialect)?)?;
    if let Some(field) = field {
        tokens = implied(tokens, field, source.len(), limits)?;
    }
    let mut parser = Parser::new(&dialect).with_tokens_with_locations(tokens);
    let context = limits::ParseContext::new(limits);
    let expression = expression::parse(&mut parser, &context)?;
    if !allow_terminator && parser.peek_token().token == Token::SemiColon {
        return Err(ParseError::at(
            &parser,
            ParseErrorKind::Syntax,
            "expression cannot end with a statement terminator",
        ));
    }
    access::finish(&mut parser)?;
    Ok(expression)
}

fn implied(
    tokens: Vec<TokenWithSpan>,
    field: &str,
    mut bytes: usize,
    limits: ParseLimits,
) -> Result<Vec<TokenWithSpan>, ParseError> {
    let mut expanded = Vec::new();
    let mut operand = true;
    for token in tokens {
        let comparison = matches!(
            token.token,
            Token::Eq | Token::Neq | Token::Lt | Token::LtEq | Token::Gt | Token::GtEq
        ) || matches!(&token.token,Token::Word(word) if word.quote_style.is_none()&&matches!(word.keyword,Keyword::IS|Keyword::BETWEEN|Keyword::IN|Keyword::LIKE));
        if operand && comparison {
            bytes = bytes.checked_add(field.len()).ok_or_else(|| {
                ParseError::new(
                    ParseErrorKind::Limit,
                    "validation expansion limit exceeded",
                    token.span.start,
                )
            })?;
            if bytes > limits.max_bytes.min(ParseLimits::MAX_BYTES) {
                return Err(ParseError::new(
                    ParseErrorKind::Limit,
                    "validation expansion limit exceeded",
                    token.span.start,
                ));
            }
            expanded.push(TokenWithSpan {
                token: Token::Word(Word {
                    value: field.to_owned(),
                    quote_style: Some('['),
                    keyword: Keyword::NoKeyword,
                }),
                span: token.span,
            });
        }
        operand = matches!(
            token.token,
            Token::LParen
                | Token::Comma
                | Token::Eq
                | Token::Neq
                | Token::Lt
                | Token::LtEq
                | Token::Gt
                | Token::GtEq
                | Token::Plus
                | Token::Minus
                | Token::Mul
                | Token::Div
        ) || matches!(&token.token,Token::Word(word) if word.quote_style.is_none()&&matches!(word.keyword,Keyword::AND|Keyword::OR|Keyword::XOR|Keyword::NOT));
        expanded.push(token);
        if expanded.len() > limits.max_tokens.min(ParseLimits::MAX_TOKENS) {
            return Err(ParseError::new(
                ParseErrorKind::Limit,
                "validation token limit exceeded",
                expanded
                    .last()
                    .map(|token| token.span.start)
                    .unwrap_or(Location { line: 1, column: 1 }),
            ));
        }
    }
    Ok(expanded)
}

#[cfg(test)]
mod tests;
