use super::{ParseError, ParseErrorKind as K, expression::expect_token};
use crate::query::DataType;
use sqlparser::{parser::Parser, tokenizer::Token};
/// Shared syntactic type carrier; no storage layout is chosen.
pub(super) fn parse(p: &mut Parser<'_>, column: bool) -> Result<DataType, ParseError> {
    let Token::Word(type_word) = p.next_token().token else {
        return Err(ParseError::at(p, K::Syntax, "expected Access data type"));
    };
    let type_name = type_word.value.to_ascii_uppercase();
    if (!column && matches!(type_name.as_str(), "COUNTER" | "AUTOINCREMENT"))
        || type_word.quote_style.is_some()
        || !matches!(
            type_name.as_str(),
            "COUNTER"
                | "AUTOINCREMENT"
                | "BINARY"
                | "BIT"
                | "BOOLEAN"
                | "YESNO"
                | "BYTE"
                | "TINYINT"
                | "SHORT"
                | "SMALLINT"
                | "LONG"
                | "INTEGER"
                | "INT"
                | "LONGINTEGER"
                | "BIGINT"
                | "LONGLONG"
                | "SINGLE"
                | "IEEESINGLE"
                | "REAL"
                | "FLOAT"
                | "DOUBLE"
                | "IEEEDOUBLE"
                | "CURRENCY"
                | "MONEY"
                | "DECIMAL"
                | "NUMERIC"
                | "DATETIME"
                | "DATE"
                | "TIME"
                | "DATETIMEEXTENDED"
                | "GUID"
                | "UNIQUEIDENTIFIER"
                | "CHAR"
                | "CHARACTER"
                | "TEXT"
                | "VARCHAR"
                | "MEMO"
                | "LONGTEXT"
                | "LONGCHAR"
                | "IMAGE"
                | "LONGBINARY"
        )
    {
        return Err(ParseError::at(
            p,
            K::Unsupported,
            "unsupported Access data type",
        ));
    }
    let mut arguments = Vec::new();
    if p.consume_token(&Token::LParen) {
        loop {
            let Token::Number(value, false) = p.next_token().token else {
                return Err(ParseError::at(
                    p,
                    K::Syntax,
                    "type argument requires an unsigned integer",
                ));
            };
            arguments.push(
                value
                    .parse::<u32>()
                    .map_err(|_| ParseError::at(p, K::Syntax, "invalid type argument"))?,
            );
            if !p.consume_token(&Token::Comma) {
                break;
            }
            if arguments.len() >= 2 {
                return Err(ParseError::at(p, K::Syntax, "too many data type arguments"));
            }
        }
        expect_token(p, Token::RParen)?;
    }
    Ok(DataType {
        name: type_name,
        arguments,
    })
}
