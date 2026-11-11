//! Bare provider default words remain distinct from bracketed/quoted expressions.
use super::{BareDefaultWord, ColumnDefault, ParseContext, ParseError};
use crate::{
    parser::{default_word::is_literal_word, expression},
    select::Name,
};
use sqlparser::{keywords::Keyword, parser::Parser, tokenizer::Token};

pub(super) fn parse(
    parser: &mut Parser<'_>,
    context: &ParseContext,
) -> Result<ColumnDefault, ParseError> {
    if matches!(parser.peek_token().token, Token::Word(ref word) if is_literal_word(word))
        && ends_word(parser)
    {
        if context.depth() == 0 {
            return Err(ParseError::at(
                parser,
                super::K::Limit,
                "default word depth limit exceeded",
            ));
        }
        let Token::Word(word) = parser.next_token().token else {
            return Err(ParseError::at(
                parser,
                super::K::Syntax,
                "expected default word",
            ));
        };
        return Ok(ColumnDefault::BareWord(BareDefaultWord::from_token(Name(
            word.value,
        ))));
    }
    let expression = expression::parse_column_default(parser, context)?;
    Ok(ColumnDefault::Expression(Box::new(expression)))
}

fn ends_word(parser: &Parser<'_>) -> bool {
    match parser.peek_nth_token(1).token {
        Token::EOF | Token::SemiColon | Token::Comma | Token::RParen => true,
        Token::Word(word) if word.quote_style.is_none() => {
            matches!(
                word.keyword,
                Keyword::DEFAULT
                    | Keyword::WITH
                    | Keyword::CONSTRAINT
                    | Keyword::PRIMARY
                    | Keyword::UNIQUE
                    | Keyword::REFERENCES
                    | Keyword::CHECK
            ) || word.keyword == Keyword::NOT
                && matches!(parser.peek_nth_token(2).token, Token::Word(ref next) if next.quote_style.is_none() && next.keyword == Keyword::NULL)
        }
        _ => false,
    }
}
