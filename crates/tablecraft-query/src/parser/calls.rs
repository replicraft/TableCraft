//! Function-name validation and explicit argument positions.
use super::{
    ParseError, ParseErrorKind as K,
    expression::{argument, expect_token},
    limits::ParseContext,
};
use crate::select::{Expression, FunctionArgument as A, Name};
use sqlparser::{keywords::Keyword, parser::Parser, tokenizer::Token};
pub(super) fn lookahead(parser: &Parser<'_>) -> bool {
    let mut index = 0;
    loop {
        if !matches!(parser.peek_nth_token(index).token, Token::Word(_)) {
            return false;
        }
        match parser.peek_nth_token(index + 1).token {
            Token::LParen => return true,
            Token::Period => index += 2,
            _ => return false,
        }
    }
}
pub(super) fn parse(
    parser: &mut Parser<'_>,
    limits: &ParseContext,
    recursion: usize,
) -> Result<Expression, ParseError> {
    let mut names = Vec::new();
    loop {
        let Token::Word(word) = parser.next_token().token else {
            return Err(ParseError::at(
                parser,
                K::Syntax,
                "invalid function identifier",
            ));
        };
        if word.value.is_empty()
            || word
                .value
                .chars()
                .any(|c| c.is_control() || "[]`.!".contains(c))
        {
            return Err(ParseError::at(
                parser,
                K::Syntax,
                "invalid function identifier",
            ));
        }
        if word.quote_style.is_none()
            && word.keyword != Keyword::NoKeyword
            && !matches!(
                word.value.to_ascii_uppercase().as_str(),
                "COUNT"
                    | "SUM"
                    | "AVG"
                    | "MIN"
                    | "MAX"
                    | "FIRST"
                    | "LAST"
                    | "LEFT"
                    | "RIGHT"
                    | "MID"
                    | "DATE"
                    | "TIME"
                    | "YEAR"
                    | "MONTH"
                    | "DAY"
                    | "HOUR"
                    | "MINUTE"
                    | "SECOND"
                    | "FORMAT"
                    | "INT"
                    | "STR"
                    | "DATEPART"
                    | "DATEDIFF"
                    | "DATEADD"
                    | "ROUND"
                    | "TRIM"
                    | "REPLACE"
                    | "ABS"
                    | "FIX"
                    | "SIGN"
                    | "SPACE"
                    | "STRING"
                    | "ASC"
                    | "CHR"
            )
        {
            return Err(ParseError::at(
                parser,
                K::Unsupported,
                "reserved SQL construct is not an Access function",
            ));
        }
        names.push(Name(word.value));
        if !parser.consume_token(&Token::Period) {
            break;
        }
    }
    let name = names.pop().expect("at least one function name");
    expect_token(parser, Token::LParen)?;
    let mut arguments = Vec::new();
    if !parser.consume_token(&Token::RParen) {
        loop {
            if matches!(parser.peek_token().token, Token::Comma | Token::RParen) {
                arguments.push(A::Missing);
            } else if parser.consume_token(&Token::Mul) {
                arguments.push(A::Wildcard);
            } else {
                arguments.push(A::Expression(argument(parser, limits, recursion)?));
            }
            if parser.consume_token(&Token::RParen) {
                break;
            }
            expect_token(parser, Token::Comma)?;
        }
    }
    Ok(Expression::Call {
        qualifier: names,
        name,
        arguments,
    })
}
