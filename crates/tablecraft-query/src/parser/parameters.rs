use super::{ParseError, ParseErrorKind as K, access::name, expression::expect_token};
use crate::query::{Parameter, ParameterType};
use sqlparser::{parser::Parser, tokenizer::Token};
pub(super) fn parse(p: &mut Parser<'_>) -> Result<Vec<Parameter>, ParseError> {
    let mut parameters = Vec::new();
    if !super::expression::word(p, "PARAMETERS") {
        return Ok(parameters);
    }
    loop {
        let name = name(p)?;
        let Token::Word(type_word) = p.next_token().token else {
            return Err(ParseError::at(p, K::Syntax, "expected parameter data type"));
        };
        let type_name = type_word.value.to_ascii_uppercase();
        if type_word.quote_style.is_some()
            || !matches!(
                type_name.as_str(),
                "BINARY"
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
                    | "REAL"
                    | "FLOAT"
                    | "DOUBLE"
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
                "unsupported parameter data type",
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
        parameters.push(Parameter {
            name,
            data_type: ParameterType {
                name: type_name,
                arguments,
            },
        });
        if !p.consume_token(&Token::Comma) {
            break;
        }
    }
    expect_token(p, Token::SemiColon)?;
    Ok(parameters)
}
