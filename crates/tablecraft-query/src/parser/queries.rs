use super::{
    ParseError,
    access::{self, expect, name},
    actions, expression, parameters,
};
use crate::{
    query::{Crosstab, Query, ReadBody, ReadQuery, Statement, UnionArm},
    select::Expression,
};
use sqlparser::{keywords::Keyword as W, parser::Parser, tokenizer::Token};
pub(super) fn query(
    p: &mut Parser<'_>,
    limits: &super::limits::ParseContext,
) -> Result<Query, ParseError> {
    let _guard = limits.enter(p)?;
    let parameters = parameters::parse(p)?;
    let statement = if matches!(p.peek_token().token, Token::Word(ref w) if matches!(w.keyword, W::CREATE | W::ALTER | W::DROP | W::PROCEDURE))
    {
        if !parameters.is_empty() {
            return Err(ParseError::at(
                p,
                super::ParseErrorKind::Unsupported,
                "schema definitions cannot declare query parameters",
            ));
        }
        Statement::Definition(Box::new(super::definition::parse(p, limits)?))
    } else if expression::word(p, "TRANSFORM") {
        let transform = expression::parse(p, limits)?;
        let mut select = access::body(p, limits)?;
        if select.projection.iter().any(|projection| {
            matches!(projection, crate::select::Projection::Expression { expression, .. } if contains_subquery(expression))
        }) {
            return Err(ParseError::at(
                p,
                super::ParseErrorKind::Unsupported,
                "crosstab output expressions cannot contain subqueries",
            ));
        }
        if select.into.is_some() {
            return Err(super::ParseError::at(
                p,
                super::ParseErrorKind::Syntax,
                "crosstab SELECT cannot contain INTO",
            ));
        }
        select.order_by = access::order(p, limits)?;
        expect(p, W::PIVOT)?;
        let pivot = expression::parse(p, limits)?;
        let (pivot, headings) = match pivot {
            Expression::InList {
                expression,
                values,
                negated: false,
            } => (*expression, Some(values)),
            pivot => (pivot, None),
        };
        Statement::Crosstab(Box::new(Crosstab {
            transform,
            select,
            pivot,
            headings,
        }))
    } else if matches!(p.peek_token().token, Token::Word(ref w) if matches!(w.keyword, W::INSERT | W::UPDATE | W::DELETE))
    {
        actions::parse(p, limits)?
    } else {
        let mut read = read_impl(p, limits, true)?;
        if let ReadBody::Select(ref mut select) = read.body
            && select.into.is_some()
        {
            select.order_by = std::mem::take(&mut read.order_by);
            select.owner_access = read.owner_access;
            let ReadBody::Select(select) = read.body else {
                unreachable!()
            };
            Statement::MakeTable(select)
        } else {
            Statement::Read(Box::new(read))
        }
    };
    Ok(Query {
        parameters,
        statement,
    })
}

fn contains_subquery(expression: &Expression) -> bool {
    use crate::select::FunctionArgument;
    match expression {
        Expression::Subquery(_) | Expression::Exists(_) | Expression::InQuery { .. }
        | Expression::Quantified { .. } => true,
        Expression::Call { arguments, .. } => arguments.iter().any(|argument| {
            matches!(argument, FunctionArgument::Expression(value) if contains_subquery(value))
        }),
        Expression::Unary { expression, .. } | Expression::Member { expression, .. }
        | Expression::IsNull { expression, .. } => contains_subquery(expression),
        Expression::Like { expression, pattern, .. } => contains_subquery(expression) || contains_subquery(pattern),
        Expression::Binary { left, right, .. } => contains_subquery(left) || contains_subquery(right),
        Expression::Between { expression, first_bound, second_bound, .. } => {
            contains_subquery(expression) || contains_subquery(first_bound) || contains_subquery(second_bound)
        }
        Expression::InList { expression, values, .. } => contains_subquery(expression) || values.iter().any(contains_subquery),
        Expression::Field(_) | Expression::Integer(_) | Expression::Text(_) | Expression::Number(_)
        | Expression::DateLiteral(_) | Expression::GuidLiteral(_) | Expression::Identity
        | Expression::Null | Expression::Boolean(_) => false,
    }
}
pub(super) fn read(
    p: &mut Parser<'_>,
    limits: &super::limits::ParseContext,
) -> Result<ReadQuery, ParseError> {
    read_impl(p, limits, false)
}
fn read_impl(
    p: &mut Parser<'_>,
    limits: &super::limits::ParseContext,
    allow_into: bool,
) -> Result<ReadQuery, ParseError> {
    let _guard = limits.enter(p)?;
    fn operand(
        p: &mut Parser<'_>,
        limits: &super::limits::ParseContext,
    ) -> Result<ReadBody, ParseError> {
        if p.parse_keyword(W::TABLE) {
            Ok(ReadBody::Table(name(p)?))
        } else if matches!(p.peek_token().token, Token::Word(ref w) if w.keyword == W::SELECT) {
            Ok(ReadBody::Select(Box::new(access::body(p, limits)?)))
        } else {
            Ok(ReadBody::Reference(name(p)?))
        }
    }
    let first = operand(p, limits)?;
    let mut rest = Vec::new();
    while p.parse_keyword(W::UNION) {
        let all = p.parse_keyword(W::ALL);
        rest.push(UnionArm {
            all,
            body: operand(p, limits)?,
        });
    }
    let make_table =
        |body: &ReadBody| matches!(body, ReadBody::Select(select) if select.into.is_some());
    if (!allow_into || !rest.is_empty())
        && (make_table(&first) || rest.iter().any(|arm| make_table(&arm.body)))
    {
        return Err(super::ParseError::at(
            p,
            super::ParseErrorKind::Syntax,
            "make-table query cannot be a retrieval operand",
        ));
    }
    let body = if rest.is_empty() {
        if matches!(first, ReadBody::Reference(_)) {
            return Err(super::ParseError::at(
                p,
                super::ParseErrorKind::Unsupported,
                "stored query references require UNION",
            ));
        }
        first
    } else {
        ReadBody::Union {
            first: Box::new(first),
            rest,
        }
    };
    Ok(ReadQuery {
        body,
        order_by: access::order(p, limits)?,
        owner_access: access::owner(p)?,
    })
}
