//! Preserve Access hash-delimited date tokens before filtering source trivia.
use super::{ParseError, ParseErrorKind as K};
use sqlparser::{
    dialect::Dialect,
    keywords::Keyword,
    tokenizer::{Location, Token, TokenWithSpan, Tokenizer, Word},
};
pub(super) fn tokens(
    source: &str,
    dialect: &dyn Dialect,
) -> Result<Vec<TokenWithSpan>, ParseError> {
    let tokens = Tokenizer::new(dialect, source)
        .tokenize_with_location()
        .map_err(|e| ParseError::new(K::Syntax, "invalid SQL token", e.location))?;
    let mut tokens = tokens.into_iter();
    let mut result = Vec::new();
    while let Some(mut token) = tokens.next() {
        if matches!(token.token, Token::Sharp | Token::Char('#')) {
            let start = byte_offset(source, token.span.start) + 1;
            let end = loop {
                let next = tokens.next().ok_or_else(|| {
                    ParseError::new(K::Syntax, "unclosed date literal", token.span.start)
                })?;
                if matches!(next.token, Token::Sharp | Token::Char('#')) {
                    break next;
                }
            };
            let value = source[start..byte_offset(source, end.span.start)].trim();
            if value.is_empty() {
                return Err(ParseError::new(
                    K::Syntax,
                    "empty date literal",
                    token.span.start,
                ));
            }
            token.token = Token::Word(Word {
                value: value.to_owned(),
                quote_style: Some('#'),
                keyword: Keyword::NoKeyword,
            });
            token.span.end = end.span.end;
        }
        result.push(token);
    }
    Ok(result)
}
fn byte_offset(source: &str, location: Location) -> usize {
    let prefix = source
        .split_inclusive('\n')
        .take(location.line.saturating_sub(1) as usize)
        .map(str::len)
        .sum::<usize>();
    prefix
        + source[prefix..]
            .chars()
            .take(location.column.saturating_sub(1) as usize)
            .map(char::len_utf8)
            .sum::<usize>()
}
