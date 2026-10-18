use sqlparser::{dialect::Dialect, keywords::Keyword, parser::Parser, tokenizer::Token};

use crate::select::{FieldReference, Name, Projection, Select, SelectPredicate, SortKey, Table};

use super::{ParseError, ParseErrorKind, ParseLimits, expression};

/// Only lexical rules are supplied here. The bounded outer grammar below avoids
/// inheriting SQL Server SELECT extensions or interpreting Access as generic SQL.
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
        ch.is_alphanumeric() || ch == '_'
    }
}

pub(super) fn name(parser: &mut Parser<'_>) -> Result<Name, ParseError> {
    let token = parser.peek_token();
    match token.token {
        Token::Word(ref word)
            if (word.quote_style == Some('[') || word.keyword == Keyword::NoKeyword)
                && !word.value.is_empty()
                && !word
                    .value
                    .chars()
                    .any(|c| c.is_control() || "[]`.!".contains(c)) =>
        {
            parser.next_token();
            Ok(Name(word.value.clone()))
        }
        _ => Err(ParseError::at(
            parser,
            ParseErrorKind::Unsupported,
            "expected an unreserved or bracketed name",
        )),
    }
}

fn field(parser: &mut Parser<'_>) -> Result<FieldReference, ParseError> {
    let first = name(parser)?;
    if parser.consume_token(&Token::Period) {
        Ok(FieldReference {
            qualifier: Some(first),
            name: name(parser)?,
        })
    } else {
        Ok(FieldReference {
            qualifier: None,
            name: first,
        })
    }
}

fn alias(parser: &mut Parser<'_>) -> Result<Option<Name>, ParseError> {
    if parser.parse_keyword(Keyword::AS) {
        name(parser).map(Some)
    } else {
        Ok(None)
    }
}

fn projection(parser: &mut Parser<'_>) -> Result<Projection, ParseError> {
    if parser.consume_token(&Token::Mul) {
        return Ok(Projection::Wildcard(None));
    }
    let first = name(parser)?;
    let field = if parser.consume_token(&Token::Period) {
        if parser.consume_token(&Token::Mul) {
            return Ok(Projection::Wildcard(Some(first)));
        }
        FieldReference {
            qualifier: Some(first),
            name: name(parser)?,
        }
    } else {
        FieldReference {
            qualifier: None,
            name: first,
        }
    };
    Ok(Projection::Field {
        field,
        alias: alias(parser)?,
    })
}

fn predicate(parser: &mut Parser<'_>) -> Result<SelectPredicate, ParseError> {
    if parser.parse_keyword(Keyword::ALL) {
        return Ok(SelectPredicate::All);
    }
    if parser.parse_keyword(Keyword::DISTINCT) {
        return Ok(SelectPredicate::Distinct);
    }
    if parser.parse_keyword(Keyword::DISTINCTROW) {
        return Ok(SelectPredicate::DistinctRow);
    }
    if !parser.parse_keyword(Keyword::TOP) {
        return Ok(SelectPredicate::All);
    }
    let token = parser.peek_token();
    let count = match &token.token {
        Token::Number(value, false) if value.bytes().all(|b| b.is_ascii_digit()) => {
            value.parse::<u32>().ok()
        }
        _ => None,
    }
    .filter(|count| (1..=i32::MAX as u32).contains(count))
    .ok_or_else(|| {
        ParseError::at(
            parser,
            ParseErrorKind::Unsupported,
            "TOP requires a positive 32-bit integer",
        )
    })?;
    parser.next_token();
    let percent = parser.parse_keyword(Keyword::PERCENT);
    if percent && count > 100 {
        return Err(ParseError::new(
            ParseErrorKind::Syntax,
            "TOP PERCENT must be at most 100",
            token.span.start,
        ));
    }
    Ok(SelectPredicate::Top { count, percent })
}

fn expect(parser: &mut Parser<'_>, keyword: Keyword) -> Result<(), ParseError> {
    if parser.parse_keyword(keyword) {
        Ok(())
    } else {
        Err(ParseError::at(
            parser,
            ParseErrorKind::Syntax,
            "expected SELECT, FROM or BY keyword",
        ))
    }
}

pub(super) fn select(parser: &mut Parser<'_>, limits: ParseLimits) -> Result<Select, ParseError> {
    if !parser.parse_keyword(Keyword::SELECT) {
        return Err(ParseError::at(
            parser,
            ParseErrorKind::Unsupported,
            "only a single SELECT statement is supported",
        ));
    }
    let predicate = predicate(parser)?;
    let mut columns = vec![projection(parser)?];
    while parser.consume_token(&Token::Comma) {
        columns.push(projection(parser)?);
    }
    expect(parser, Keyword::FROM)?;
    let from = Table {
        name: name(parser)?,
        alias: alias(parser)?,
    };
    let selection = if parser.parse_keyword(Keyword::WHERE) {
        Some(expression::parse(parser, limits)?)
    } else {
        None
    };
    let mut order_by = Vec::new();
    if parser.parse_keyword(Keyword::ORDER) {
        expect(parser, Keyword::BY)?;
        loop {
            let field = field(parser)?;
            let descending = parser.parse_keyword(Keyword::DESC);
            if !descending {
                let _ = parser.parse_keyword(Keyword::ASC);
            }
            order_by.push(SortKey { field, descending });
            if !parser.consume_token(&Token::Comma) {
                break;
            }
        }
    }
    let _ = parser.consume_token(&Token::SemiColon);
    if !matches!(parser.peek_token().token, Token::EOF) {
        return Err(ParseError::at(
            parser,
            ParseErrorKind::Unsupported,
            "unsupported SELECT clause or extra statement",
        ));
    }
    Ok(Select {
        predicate,
        projection: columns,
        from,
        selection,
        order_by,
    })
}
