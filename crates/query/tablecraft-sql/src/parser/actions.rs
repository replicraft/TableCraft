use super::{
    ParseError, ParseErrorKind as K,
    access::{self, expect, name},
    expression::{self, expect_token},
    queries, relations,
};
use crate::{
    query::{Assignment, Delete, Insert, InsertSource, Statement, Update},
    select::FieldReference,
};
use sqlparser::{keywords::Keyword as W, parser::Parser, tokenizer::Token};
pub(super) fn parse(
    p: &mut Parser<'_>,
    limits: &super::limits::ParseContext,
) -> Result<Statement, ParseError> {
    if p.parse_keyword(W::INSERT) {
        expect(p, W::INTO)?;
        let target = name(p)?;
        let mut fields = Vec::new();
        if p.consume_token(&Token::LParen) {
            fields.push(name(p)?);
            while p.consume_token(&Token::Comma) {
                fields.push(name(p)?);
            }
            expect_token(p, Token::RParen)?;
        }
        let external = access::external(p)?;
        let source = if p.parse_keyword(W::VALUES) {
            expect_token(p, Token::LParen)?;
            let values = access::exprs(p, limits)?;
            expect_token(p, Token::RParen)?;
            InsertSource::Values(values)
        } else {
            InsertSource::Query(Box::new(queries::read(p, limits)?))
        };
        return Ok(Statement::Insert(Box::new(Insert {
            target,
            fields,
            external,
            source,
        })));
    }
    if p.parse_keyword(W::UPDATE) {
        let sources = relations::sources(p, limits)?;
        expect(p, W::SET)?;
        let mut assignments = Vec::new();
        loop {
            let first = name(p)?;
            let target = if p.consume_token(&Token::Period) {
                FieldReference {
                    qualifier: Some(first),
                    name: name(p)?,
                }
            } else {
                FieldReference {
                    qualifier: None,
                    name: first,
                }
            };
            expect_token(p, Token::Eq)?;
            let value = expression::parse(p, limits)?;
            assignments.push(Assignment { target, value });
            if !p.consume_token(&Token::Comma) {
                break;
            }
        }
        let selection = selection(p, limits)?;
        return Ok(Statement::Update(Box::new(Update {
            sources,
            assignments,
            selection,
        })));
    }
    expect(p, W::DELETE)?;
    let mut targets = Vec::new();
    if !p.consume_token(&Token::Mul)
        && !matches!(p.peek_token().token, Token::Word(ref w) if w.keyword == W::FROM)
    {
        loop {
            targets.push(name(p)?);
            expect_token(p, Token::Period)?;
            expect_token(p, Token::Mul)?;
            if !p.consume_token(&Token::Comma) {
                break;
            }
        }
    }
    expect(p, W::FROM)?;
    let sources = relations::sources(p, limits)?;
    let selection = selection(p, limits)?;
    if sources.is_empty() {
        return Err(ParseError::at(p, K::Syntax, "DELETE requires a source"));
    }
    Ok(Statement::Delete(Box::new(Delete {
        targets,
        sources,
        selection,
    })))
}
fn selection(
    p: &mut Parser<'_>,
    limits: &super::limits::ParseContext,
) -> Result<Option<crate::select::Expression>, ParseError> {
    if p.parse_keyword(W::WHERE) {
        expression::parse(p, limits).map(Some)
    } else {
        Ok(None)
    }
}
