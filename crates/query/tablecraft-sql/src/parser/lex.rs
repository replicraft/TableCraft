//! Preserve Access hash-delimited date tokens before filtering source trivia.
use super::{ParseError, ParseErrorKind as K};
use sqlparser::{
    dialect::Dialect,
    keywords::Keyword,
    tokenizer::{Location, Token, TokenWithSpan, Tokenizer, Word},
};

const GUID_TEXT_BYTES: usize = 36;
const GUID_HYPHEN_OFFSETS: [usize; 4] = [8, 13, 18, 23];
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
        if token.token == Token::LBrace {
            let invalid = || ParseError::new(K::Syntax, "invalid GUID literal", token.span.start);
            let prefix = tokens.next().ok_or_else(invalid)?;
            // Whitespace remains in the lexical stream at this stage.
            let mut prefix = prefix;
            while matches!(prefix.token, Token::Whitespace(_)) {
                prefix = tokens.next().ok_or_else(invalid)?;
            }
            if !matches!(prefix.token, Token::Word(ref w) if w.quote_style.is_none() && w.value.eq_ignore_ascii_case("GUID"))
            {
                return Err(ParseError::new(
                    K::Unsupported,
                    "unsupported brace literal",
                    token.span.start,
                ));
            }
            let mut open = tokens.next().ok_or_else(invalid)?;
            while matches!(open.token, Token::Whitespace(_)) {
                open = tokens.next().ok_or_else(invalid)?;
            }
            if open.token != Token::LBrace {
                return Err(invalid());
            }
            let start = byte_offset(source, open.span.end);
            let close = loop {
                let next = tokens.next().ok_or_else(invalid)?;
                if next.token == Token::RBrace {
                    break next;
                }
            };
            let value = source[start..byte_offset(source, close.span.start)].trim();
            if value.len() != GUID_TEXT_BYTES
                || !value.bytes().enumerate().all(|(i, c)| {
                    if GUID_HYPHEN_OFFSETS.contains(&i) {
                        c == b'-'
                    } else {
                        c.is_ascii_hexdigit()
                    }
                })
            {
                return Err(invalid());
            }
            let mut end = tokens.next().ok_or_else(invalid)?;
            while matches!(end.token, Token::Whitespace(_)) {
                end = tokens.next().ok_or_else(invalid)?;
            }
            if end.token != Token::RBrace {
                return Err(invalid());
            }
            token.token = Token::Word(Word {
                value: value.to_owned(),
                quote_style: Some('{'),
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
