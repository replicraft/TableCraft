//! Provider bare defaults use lexical words, not ordinary identifier-name policy.
use super::{ParseError, ParseErrorKind, ParseLimits, access::AccessDialect, lex};
use crate::select::Name;
use sqlparser::{
    keywords::Keyword,
    tokenizer::{Location, Token, Word},
};

pub(super) fn is_literal_word(word: &Word) -> bool {
    word.quote_style.is_none()
        && !matches!(word.keyword, Keyword::NULL | Keyword::TRUE | Keyword::FALSE)
}

pub(crate) fn parse(source: &str, limits: ParseLimits) -> Result<Name, ParseError> {
    limits.check_input(source)?;
    let tokens = limits.check_tokens(source, lex::tokens(source, &AccessDialect)?)?;
    if limits.depth() == 0 {
        return Err(ParseError::new(
            ParseErrorKind::Limit,
            "default word depth limit exceeded",
            Location { line: 1, column: 1 },
        ));
    }
    if tokens.len() == 1 {
        let mut tokens = tokens.into_iter();
        if let Some(token) = tokens.next()
            && let Token::Word(word) = token.token
            && is_literal_word(&word)
            && word.value == source
        {
            return Ok(Name(word.value));
        }
    }
    Err(ParseError::new(
        ParseErrorKind::Syntax,
        "default word requires one unquoted word token",
        Location { line: 1, column: 1 },
    ))
}
