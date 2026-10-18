use sqlparser::{
    ast,
    parser::{Parser, ParserError},
    tokenizer::Location,
};

use crate::select::{BinaryOperator, Expression, FieldReference, Name};

use super::{ParseError, ParseErrorKind, ParseLimits};

pub(super) fn parse(
    parser: &mut Parser<'_>,
    limits: ParseLimits,
) -> Result<Expression, ParseError> {
    let location = parser.peek_token().span.start;
    let upstream = parser.parse_expr().map_err(|error| {
        ParseError::at(
            parser,
            if error == ParserError::RecursionLimitExceeded {
                ParseErrorKind::Limit
            } else {
                ParseErrorKind::Syntax
            },
            if error == ParserError::RecursionLimitExceeded {
                "SQL parser recursion limit exceeded"
            } else {
                "invalid or unsupported WHERE expression"
            },
        )
    })?;
    convert(upstream, limits, 1, location)
}

fn unsupported(location: Location) -> ParseError {
    ParseError::new(
        ParseErrorKind::Unsupported,
        "unsupported WHERE expression",
        location,
    )
}

fn name(ident: ast::Ident, location: Location) -> Result<Name, ParseError> {
    // The upstream expression parser is more permissive about reserved words.
    // Apply exactly the outer grammar's identifier policy here as well.
    use sqlparser::keywords::ALL_KEYWORDS;
    let reserved = ALL_KEYWORDS
        .binary_search(&ident.value.to_ascii_uppercase().as_str())
        .is_ok();
    if !matches!(ident.quote_style, None | Some('['))
        || ident.value.is_empty()
        || (ident.quote_style.is_none() && reserved)
        || ident
            .value
            .chars()
            .any(|c| c.is_control() || "[]`.!".contains(c))
    {
        return Err(unsupported(location));
    }
    Ok(Name(ident.value))
}

fn convert(
    expr: ast::Expr,
    limits: ParseLimits,
    depth: usize,
    location: Location,
) -> Result<Expression, ParseError> {
    if depth > limits.depth() {
        return Err(ParseError::new(
            ParseErrorKind::Limit,
            "SQL expression depth limit exceeded",
            location,
        ));
    }
    let nested = |expr| convert(expr, limits, depth + 1, location).map(Box::new);
    Ok(match expr {
        ast::Expr::Identifier(ident) => Expression::Field(FieldReference {
            qualifier: None,
            name: name(ident, location)?,
        }),
        ast::Expr::CompoundIdentifier(mut parts) if parts.len() == 2 => {
            let field = name(parts.pop().expect("two parts"), location)?;
            let qualifier = name(parts.pop().expect("two parts"), location)?;
            Expression::Field(FieldReference {
                qualifier: Some(qualifier),
                name: field,
            })
        }
        ast::Expr::Nested(expr) => return convert(*expr, limits, depth, location),
        ast::Expr::Value(value) => match value.value {
            ast::Value::Number(value, false) if value.bytes().all(|b| b.is_ascii_digit()) => {
                Expression::Integer(value.parse().map_err(|_| unsupported(location))?)
            }
            ast::Value::SingleQuotedString(value) | ast::Value::DoubleQuotedString(value) => {
                Expression::Text(value)
            }
            ast::Value::Null => Expression::Null,
            ast::Value::Boolean(value) => Expression::Boolean(value),
            _ => return Err(unsupported(location)),
        },
        ast::Expr::UnaryOp { op, expr }
            if matches!(op, ast::UnaryOperator::Minus | ast::UnaryOperator::Plus) =>
        {
            let ast::Expr::Value(value) = *expr else {
                return Err(unsupported(location));
            };
            let ast::Value::Number(value, false) = value.value else {
                return Err(unsupported(location));
            };
            if !value.bytes().all(|b| b.is_ascii_digit()) {
                return Err(unsupported(location));
            }
            let signed = if op == ast::UnaryOperator::Minus {
                format!("-{value}")
            } else {
                value
            };
            Expression::Integer(signed.parse().map_err(|_| unsupported(location))?)
        }
        ast::Expr::BinaryOp { left, op, right } => {
            let operator = match op {
                ast::BinaryOperator::Eq => BinaryOperator::Equal,
                ast::BinaryOperator::NotEq => BinaryOperator::NotEqual,
                ast::BinaryOperator::Lt => BinaryOperator::Less,
                ast::BinaryOperator::LtEq => BinaryOperator::LessOrEqual,
                ast::BinaryOperator::Gt => BinaryOperator::Greater,
                ast::BinaryOperator::GtEq => BinaryOperator::GreaterOrEqual,
                ast::BinaryOperator::And => BinaryOperator::And,
                ast::BinaryOperator::Or => BinaryOperator::Or,
                _ => return Err(unsupported(location)),
            };
            Expression::Binary {
                left: nested(*left)?,
                operator,
                right: nested(*right)?,
            }
        }
        ast::Expr::IsNull(expr) => Expression::IsNull {
            expression: nested(*expr)?,
            negated: false,
        },
        ast::Expr::IsNotNull(expr) => Expression::IsNull {
            expression: nested(*expr)?,
            negated: true,
        },
        ast::Expr::Between {
            expr,
            negated,
            low,
            high,
        } => Expression::Between {
            expression: nested(*expr)?,
            // Upstream names these low/high; Access syntax may reverse them.
            // Retain the input order without imposing numerical ordering.
            first_bound: nested(*low)?,
            second_bound: nested(*high)?,
            negated,
        },
        ast::Expr::InList {
            expr,
            list,
            negated,
        } => {
            if list.is_empty() {
                return Err(ParseError::new(
                    ParseErrorKind::Syntax,
                    "IN requires a non-empty list",
                    location,
                ));
            }
            Expression::InList {
                expression: nested(*expr)?,
                values: list
                    .into_iter()
                    .map(|value| convert(value, limits, depth + 1, location))
                    .collect::<Result<_, _>>()?,
                negated,
            }
        }
        _ => return Err(unsupported(location)),
    })
}
