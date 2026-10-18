//! SELECT clauses built over Access expression and relation adapters.
use super::{
    ParseError, ParseErrorKind as K,
    expression::{self, word},
    relations,
};
use crate::select::{
    Expression, ExternalDatabase, Name, Projection, Select, SelectPredicate, SortKey,
};
use sqlparser::{dialect::Dialect, keywords::Keyword as W, parser::Parser, tokenizer::Token};

const PERCENT_TOTAL: u32 = 100;
#[derive(Debug)]
pub(super) struct AccessDialect;
impl Dialect for AccessDialect {
    fn is_delimited_identifier_start(&self, ch: char) -> bool {
        ch == '['
    }
    fn is_identifier_start(&self, ch: char) -> bool {
        ch.is_alphabetic() || ch == '_'
    }
    fn is_identifier_part(&self, ch: char) -> bool {
        ch.is_alphanumeric() || ch == '_' || ch == '$'
    }
}
pub(super) fn name(p: &mut Parser<'_>) -> Result<Name, ParseError> {
    match p.peek_token().token {
        Token::Word(ref w)
            if (w.quote_style == Some('[')
                || (w.quote_style.is_none() && super::identifiers::unreserved(&w.value)))
                && !w.value.is_empty()
                && !w
                    .value
                    .chars()
                    .any(|c| c.is_control() || "[]`.!".contains(c)) =>
        {
            let name = Name(w.value.clone());
            p.next_token();
            Ok(name)
        }
        _ => Err(ParseError::at(
            p,
            K::Unsupported,
            "expected an unreserved or bracketed name",
        )),
    }
}
pub(super) fn alias(p: &mut Parser<'_>) -> Result<Option<Name>, ParseError> {
    if p.parse_keyword(W::AS)
        || matches!(p.peek_token().token, Token::Word(ref w) if w.quote_style == Some('[') || (w.quote_style.is_none() && super::identifiers::unreserved(&w.value)))
    {
        name(p).map(Some)
    } else {
        Ok(None)
    }
}

fn projection(
    p: &mut Parser<'_>,
    limits: &super::limits::ParseContext,
) -> Result<Projection, ParseError> {
    if p.consume_token(&Token::Mul) {
        return Ok(Projection::Wildcard(None));
    }
    if matches!(
        p.peek_tokens::<3>(),
        [Token::Word(_), Token::Period, Token::Mul]
    ) {
        let qualifier = name(p)?;
        p.next_token();
        p.next_token();
        return Ok(Projection::Wildcard(Some(qualifier)));
    }
    let expression = expression::parse(p, limits)?;
    let alias = alias(p)?;
    Ok(match expression {
        Expression::Field(field) => Projection::Field { field, alias },
        expression => Projection::Expression { expression, alias },
    })
}
fn predicate(p: &mut Parser<'_>) -> Result<SelectPredicate, ParseError> {
    let distinct = if p.parse_keyword(W::DISTINCT) {
        1
    } else if p.parse_keyword(W::DISTINCTROW) {
        2
    } else {
        let _ = p.parse_keyword(W::ALL);
        0
    };
    if !p.parse_keyword(W::TOP) {
        return Ok(match distinct {
            1 => SelectPredicate::Distinct,
            2 => SelectPredicate::DistinctRow,
            _ => SelectPredicate::All,
        });
    }
    let token = p.peek_token();
    let count = match token.token {
        Token::Number(ref value, false) if value.bytes().all(|b| b.is_ascii_digit()) => {
            value.parse::<u32>().ok()
        }
        _ => None,
    }
    .ok_or_else(|| ParseError::at(p, K::Syntax, "TOP requires an unsigned integer"))?;
    p.next_token();
    let percent = p.parse_keyword(W::PERCENT);
    if percent && count > PERCENT_TOTAL {
        return Err(ParseError::new(
            K::Syntax,
            "TOP PERCENT must be at most 100",
            token.span.start,
        ));
    }
    Ok(match distinct {
        1 => SelectPredicate::DistinctTop { count, percent },
        2 => SelectPredicate::DistinctRowTop { count, percent },
        _ => SelectPredicate::Top { count, percent },
    })
}
pub(super) fn expect(p: &mut Parser<'_>, word: W) -> Result<(), ParseError> {
    if p.parse_keyword(word) {
        Ok(())
    } else {
        Err(ParseError::at(
            p,
            K::Syntax,
            "missing Access SQL clause keyword",
        ))
    }
}
pub(super) fn exprs(
    p: &mut Parser<'_>,
    limits: &super::limits::ParseContext,
) -> Result<Vec<Expression>, ParseError> {
    let mut items = vec![expression::parse(p, limits)?];
    while p.consume_token(&Token::Comma) {
        items.push(expression::parse(p, limits)?);
    }
    Ok(items)
}
pub(super) fn order(
    p: &mut Parser<'_>,
    limits: &super::limits::ParseContext,
) -> Result<Vec<SortKey>, ParseError> {
    let mut items = Vec::new();
    if p.parse_keyword(W::ORDER) {
        expect(p, W::BY)?;
        loop {
            let expression = expression::parse(p, limits)?;
            let descending = p.parse_keyword(W::DESC);
            if !descending {
                let _ = p.parse_keyword(W::ASC);
            }
            items.push(SortKey {
                expression,
                descending,
            });
            if !p.consume_token(&Token::Comma) {
                break;
            }
        }
    }
    Ok(items)
}
pub(super) fn external(p: &mut Parser<'_>) -> Result<Option<ExternalDatabase>, ParseError> {
    if !p.parse_keyword(W::IN) {
        return Ok(None);
    }
    fn text(p: &mut Parser<'_>) -> Result<String, ParseError> {
        match p.next_token().token {
            Token::SingleQuotedString(s) | Token::DoubleQuotedString(s) => Ok(s),
            Token::Word(w)
                if w.quote_style.is_none() && super::identifiers::unreserved(&w.value) =>
            {
                let mut path = w.value;
                while p.consume_token(&Token::Period) {
                    let Token::Word(part) = p.next_token().token else {
                        return Err(ParseError::at(
                            p,
                            K::Syntax,
                            "invalid external database path",
                        ));
                    };
                    if part.quote_style.is_some() {
                        return Err(ParseError::at(
                            p,
                            K::Syntax,
                            "bare external path cannot contain identifier delimiters",
                        ));
                    }
                    path.push('.');
                    path.push_str(&part.value);
                }
                Ok(path)
            }
            _ => Err(ParseError::at(
                p,
                K::Syntax,
                "external IN requires quoted database text",
            )),
        }
    }
    let database = text(p)?;
    let connection_bracketed =
        matches!(p.peek_token().token, Token::Word(ref w) if w.quote_style == Some('['));
    let connection = if connection_bracketed {
        let Token::Word(w) = p.next_token().token else {
            unreachable!()
        };
        Some(w.value)
    } else if matches!(
        p.peek_token().token,
        Token::SingleQuotedString(_) | Token::DoubleQuotedString(_)
    ) {
        Some(text(p)?)
    } else {
        None
    };
    Ok(Some(ExternalDatabase {
        database,
        connection,
        connection_bracketed,
    }))
}
pub(super) fn body(
    p: &mut Parser<'_>,
    limits: &super::limits::ParseContext,
) -> Result<Select, ParseError> {
    expect(p, W::SELECT)?;
    let predicate = predicate(p)?;
    let mut projection_items = vec![projection(p, limits)?];
    while p.consume_token(&Token::Comma) {
        projection_items.push(projection(p, limits)?);
    }
    let into = if p.parse_keyword(W::INTO) {
        Some(name(p)?)
    } else {
        None
    };
    let into_external = if into.is_some() { external(p)? } else { None };
    let from = if p.parse_keyword(W::FROM) {
        relations::sources(p, limits)?
    } else {
        Vec::new()
    };
    if from.is_empty()
        && (into.is_some()
            || projection_items
                .iter()
                .any(|p| matches!(p, Projection::Wildcard(_))))
    {
        return Err(ParseError::at(
            p,
            K::Syntax,
            "wildcard and make-table SELECT require FROM",
        ));
    }
    let external_db = external(p)?;
    let selection = if p.parse_keyword(W::WHERE) {
        Some(expression::parse(p, limits)?)
    } else {
        None
    };
    let group_by = if p.parse_keywords(&[W::GROUP, W::BY]) {
        exprs(p, limits)?
    } else {
        Vec::new()
    };
    let having = if p.parse_keyword(W::HAVING) {
        Some(expression::parse(p, limits)?)
    } else {
        None
    };
    Ok(Select {
        predicate,
        projection: projection_items,
        from,
        selection,
        group_by,
        having,
        into,
        into_external,
        external: external_db,
        owner_access: false,
        order_by: Vec::new(),
    })
}
pub(super) fn owner(p: &mut Parser<'_>) -> Result<bool, ParseError> {
    if p.parse_keyword(W::WITH) {
        if !word(p, "OWNERACCESS") || !p.parse_keyword(W::OPTION) {
            return Err(ParseError::at(p, K::Syntax, "expected OWNERACCESS OPTION"));
        }
        Ok(true)
    } else {
        Ok(false)
    }
}
pub(super) fn finish(p: &mut Parser<'_>) -> Result<(), ParseError> {
    let _ = p.consume_token(&Token::SemiColon);
    if p.peek_token().token != Token::EOF {
        return Err(ParseError::at(
            p,
            K::Unsupported,
            "unsupported clause or extra statement",
        ));
    }
    Ok(())
}
pub(super) fn select(
    p: &mut Parser<'_>,
    limits: &super::limits::ParseContext,
) -> Result<Select, ParseError> {
    let mut query = body(p, limits)?;
    query.order_by = order(p, limits)?;
    query.owner_access = owner(p)?;
    finish(p)?;
    Ok(query)
}
