//! Bracketed unqualified references can be parameter prompts, not object paths.
use super::{ParseError, access};
use crate::select::Name;
use sqlparser::{parser::Parser, tokenizer::Token};

pub(super) fn read(parser: &mut Parser<'_>) -> Result<Name, ParseError> {
    if let Token::Word(word) = parser.peek_token().token
        && word.quote_style == Some('[')
        && !word.value.is_empty()
        && !word
            .value
            .chars()
            .any(|ch| ch.is_control() || "[]".contains(ch))
    {
        parser.next_token();
        return Ok(Name(word.value));
    }
    access::name(parser)
}

pub(super) fn prompt_only(name: &Name) -> bool {
    name.as_str().chars().any(|ch| ".!`".contains(ch))
}

pub(super) fn member(parser: &mut Parser<'_>) -> Result<Name, ParseError> {
    if matches!(parser.peek_token().token, Token::Word(word) if word.quote_style.is_none()) {
        access::member_name(parser)
    } else {
        read(parser)
    }
}
