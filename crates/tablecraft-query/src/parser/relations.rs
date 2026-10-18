use super::{
    ParseError, ParseErrorKind as K,
    access::{alias, name},
    expression::{self, expect_token},
};
use crate::select::{JoinKind, Table, TableSource};
use sqlparser::{keywords::Keyword as W, parser::Parser, tokenizer::Token};
pub(super) fn sources(
    p: &mut Parser<'_>,
    limits: &super::limits::ParseContext,
) -> Result<Vec<TableSource>, ParseError> {
    let mut result = vec![relation(p, limits, 0)?];
    while p.consume_token(&Token::Comma) {
        result.push(relation(p, limits, 0)?);
    }
    Ok(result)
}
fn relation(
    p: &mut Parser<'_>,
    limits: &super::limits::ParseContext,
    depth: usize,
) -> Result<TableSource, ParseError> {
    let _guard = limits.enter(p)?;
    if depth >= limits.depth() {
        return Err(ParseError::at(
            p,
            K::Limit,
            "SQL relation depth limit exceeded",
        ));
    }
    let mut left = atom(p, limits, depth + 1)?;
    let mut joins = 0;
    loop {
        let kind = if p.parse_keyword(W::INNER) {
            JoinKind::Inner
        } else if p.parse_keyword(W::LEFT) {
            let _ = p.parse_keyword(W::OUTER);
            JoinKind::Left
        } else if p.parse_keyword(W::RIGHT) {
            let _ = p.parse_keyword(W::OUTER);
            JoinKind::Right
        } else {
            break;
        };
        joins += 1;
        if depth + joins >= limits.depth() {
            return Err(ParseError::at(p, K::Limit, "SQL join depth limit exceeded"));
        }
        if !p.parse_keyword(W::JOIN) {
            return Err(ParseError::at(p, K::Syntax, "expected JOIN"));
        }
        let right = atom(p, limits, depth + 1)?;
        if !p.parse_keyword(W::ON) {
            return Err(ParseError::at(p, K::Syntax, "JOIN requires ON"));
        }
        let condition = expression::parse(p, limits)?;
        left = TableSource::Join {
            left: Box::new(left),
            kind,
            right: Box::new(right),
            condition,
        };
    }
    Ok(left)
}
fn atom(
    p: &mut Parser<'_>,
    limits: &super::limits::ParseContext,
    depth: usize,
) -> Result<TableSource, ParseError> {
    if p.consume_token(&Token::LParen) {
        if matches!(p.peek_token().token, Token::Word(ref w) if w.keyword == W::SELECT) {
            let query = super::queries::read(p, limits)?;
            expect_token(p, Token::RParen)?;
            return Ok(TableSource::Derived {
                query: Box::new(query),
                alias: alias(p)?,
            });
        }
        let source = relation(p, limits, depth)?;
        expect_token(p, Token::RParen)?;
        let alias = alias(p)?;
        Ok(if alias.is_some() {
            TableSource::Group {
                source: Box::new(source),
                alias,
            }
        } else {
            source
        })
    } else {
        Ok(TableSource::Table(Table {
            name: name(p)?,
            alias: alias(p)?,
        }))
    }
}
