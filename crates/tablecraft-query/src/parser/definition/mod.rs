mod constraint;
mod index;
mod saved;
mod table;
use super::{
    ParseError, ParseErrorKind as K,
    access::{expect, name as identifier},
    data_type,
    expression::{expect_token, word},
    limits::ParseContext,
};
use crate::{query::definition::*, select::Name};
use sqlparser::{keywords::Keyword as W, parser::Parser, tokenizer::Token};

pub(super) fn parse(p: &mut Parser<'_>, context: &ParseContext) -> Result<Definition, ParseError> {
    if p.parse_keyword(W::PROCEDURE) {
        return saved::procedure(p, context, false).map(Definition::Procedure);
    }
    if p.parse_keyword(W::CREATE) {
        let temporary = word(p, "TEMPORARY");
        if p.parse_keyword(W::TABLE) {
            return table::create(p, temporary).map(Definition::CreateTable);
        }
        if temporary {
            return Err(ParseError::at(p, K::Syntax, "TEMPORARY requires TABLE"));
        }
        if p.parse_keyword(W::VIEW) {
            return saved::view(p, context).map(Definition::CreateView);
        }
        if p.parse_keyword(W::PROCEDURE) {
            return saved::procedure(p, context, true).map(Definition::Procedure);
        }
        index::create(p).map(Definition::CreateIndex)
    } else if p.parse_keyword(W::ALTER) {
        table::alter(p).map(Definition::AlterTable)
    } else {
        expect(p, W::DROP)?;
        if p.parse_keyword(W::TABLE) {
            Ok(Definition::DropTable(identifier(p)?))
        } else if p.parse_keyword(W::VIEW) {
            Ok(Definition::DropView(identifier(p)?))
        } else if p.parse_keyword(W::PROCEDURE) {
            Ok(Definition::DropProcedure(identifier(p)?))
        } else {
            expect(p, W::INDEX)?;
            let index = identifier(p)?;
            expect(p, W::ON)?;
            Ok(Definition::DropIndex {
                index,
                table: identifier(p)?,
            })
        }
    }
}
pub(super) fn names(p: &mut Parser<'_>) -> Result<Vec<Name>, ParseError> {
    expect_token(p, Token::LParen)?;
    let mut fields = vec![identifier(p)?];
    while p.consume_token(&Token::Comma) {
        fields.push(identifier(p)?);
    }
    expect_token(p, Token::RParen)?;
    Ok(fields)
}
