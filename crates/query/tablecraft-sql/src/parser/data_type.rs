use super::{
    ParseError, ParseErrorKind as K,
    expression::{expect_token, word},
};
use crate::query::DataType;
use sqlparser::{parser::Parser, tokenizer::Token};
/// Shared syntactic type carrier; no storage layout is chosen.
pub(super) fn parse(p: &mut Parser<'_>, column: bool) -> Result<DataType, ParseError> {
    let Token::Word(type_word) = p.next_token().token else {
        return Err(ParseError::at(p, K::Syntax, "expected Access data type"));
    };
    let quoted = type_word.quote_style.is_some();
    let mut type_name = type_word.value;
    type_name.make_ascii_uppercase();
    if !quoted {
        compound(p, &mut type_name)?;
    }
    if !column && !quoted && type_name == "VALUE" {
        return Ok(DataType {
            name: type_name,
            arguments: Vec::new(),
        });
    }

    if (!column && matches!(type_name.as_str(), "COUNTER" | "AUTOINCREMENT"))
        || quoted
        || !matches!(
            type_name.as_str(),
            "COUNTER"
                | "AUTOINCREMENT"
                | "BINARY"
                | "BIT"
                | "BOOLEAN"
                | "LOGICAL"
                | "LOGICAL1"
                | "YESNO"
                | "BYTE"
                | "TINYINT"
                | "INTEGER1"
                | "SHORT"
                | "SMALLINT"
                | "INTEGER2"
                | "LONG"
                | "INTEGER"
                | "INT"
                | "LONGINTEGER"
                | "INTEGER4"
                | "BIGINT"
                | "LONGLONG"
                | "SINGLE"
                | "IEEESINGLE"
                | "FLOAT4"
                | "REAL"
                | "FLOAT"
                | "DOUBLE"
                | "IEEEDOUBLE"
                | "FLOAT8"
                | "NUMBER"
                | "CURRENCY"
                | "MONEY"
                | "DECIMAL"
                | "DEC"
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
                | "ALPHANUMERIC"
                | "STRING"
                | "NCHAR"
                | "MEMO"
                | "LONGTEXT"
                | "LONGCHAR"
                | "NOTE"
                | "NTEXT"
                | "IMAGE"
                | "LONGBINARY"
                | "GENERAL"
                | "OLEOBJECT"
                | "VARBINARY"
                | "CHARACTER VARYING"
                | "NATIONAL CHARACTER"
                | "NATIONAL CHAR"
                | "NATIONAL CHARACTER VARYING"
                | "NATIONAL CHAR VARYING"
                | "BINARY VARYING"
                | "BIT VARYING"
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

// These phrases are Access synonyms in Microsoft's equivalent-type table, not
// arbitrary ANSI names. A bare BIT keeps its native Boolean meaning.
fn compound(p: &mut Parser<'_>, name: &mut String) -> Result<(), ParseError> {
    match name.as_str() {
        "NATIONAL" => {
            if word(p, "CHARACTER") {
                name.push_str(" CHARACTER");
            } else if word(p, "CHAR") {
                name.push_str(" CHAR");
            } else {
                return Err(ParseError::at(
                    p,
                    K::Syntax,
                    "NATIONAL requires CHAR or CHARACTER",
                ));
            }
            if word(p, "VARYING") {
                name.push_str(" VARYING");
            }
        }
        "CHARACTER" | "BINARY" | "BIT" if word(p, "VARYING") => {
            name.push_str(" VARYING");
        }
        _ => {}
    }
    Ok(())
}
