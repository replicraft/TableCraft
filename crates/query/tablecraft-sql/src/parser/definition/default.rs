//! Bare provider default words remain distinct from bracketed/quoted expressions.
use super::{BareDefaultWord, ColumnDefault, ParseContext, ParseError};
use crate::{parser::expression, select::Expression};
use sqlparser::{parser::Parser, tokenizer::Token};

pub(super) fn parse(
    parser: &mut Parser<'_>,
    context: &ParseContext,
) -> Result<ColumnDefault, ParseError> {
    let bare_word =
        matches!(parser.peek_token().token, Token::Word(ref word) if word.quote_style.is_none());
    let expression = expression::parse_column_default(parser, context)?;
    match expression {
        Expression::Field(field) if bare_word && field.qualifier.is_none() => Ok(
            ColumnDefault::BareWord(BareDefaultWord::from_token(field.name)),
        ),
        expression => Ok(ColumnDefault::Expression(Box::new(expression))),
    }
}
