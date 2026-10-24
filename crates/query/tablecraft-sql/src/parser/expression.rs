//! Access precedence on sqlparser's bounded token stream; no generic dialect fallback.

use super::{ParseError, ParseErrorKind as K, access, precedence as power, reference_names};
use crate::select::{
    BinaryOperator as B, Expression as E, FieldReference, FunctionArgument as A, UnaryOperator as U,
};
use sqlparser::{keywords::Keyword, parser::Parser, tokenizer::Token};

pub(super) fn word(parser: &mut Parser<'_>, value: &str) -> bool {
    if matches!(parser.peek_token().token, Token::Word(ref w) if w.quote_style.is_none() && w.value.eq_ignore_ascii_case(value))
    {
        parser.next_token();
        true
    } else {
        false
    }
}
pub(super) fn expect_token(parser: &mut Parser<'_>, token: Token) -> Result<(), ParseError> {
    if parser.consume_token(&token) {
        Ok(())
    } else {
        Err(ParseError::at(parser, K::Syntax, "missing SQL delimiter"))
    }
}
pub(super) fn parse(
    parser: &mut Parser<'_>,
    limits: &super::limits::ParseContext,
) -> Result<E, ParseError> {
    let start = parser.peek_token().span.start;
    let expr = pratt(parser, limits, 0, 0)?;
    check_depth(&expr, limits.depth(), 1)
        .map_err(|()| ParseError::new(K::Limit, "SQL expression depth limit exceeded", start))?;
    Ok(expr)
}
fn check_depth(expr: &E, max: usize, depth: usize) -> Result<(), ()> {
    if depth > max {
        return Err(());
    }
    let child = |v: &E| check_depth(v, max, depth + 1);
    match expr {
        E::Binary { left, right, .. } => {
            child(left)?;
            child(right)?;
        }
        E::Unary { expression, .. }
        | E::IsNull { expression, .. }
        | E::Member { expression, .. } => child(expression)?,
        E::Between {
            expression,
            first_bound,
            second_bound,
            ..
        } => {
            child(expression)?;
            child(first_bound)?;
            child(second_bound)?;
        }
        E::InList {
            expression, values, ..
        } => {
            child(expression)?;
            for v in values {
                child(v)?;
            }
        }
        E::Like {
            expression,
            pattern,
            ..
        } => {
            child(expression)?;
            child(pattern)?;
        }
        E::InQuery { expression, .. } | E::Quantified { expression, .. } => child(expression)?,
        E::Call { arguments, .. } => {
            for arg in arguments {
                if let A::Expression(v) = arg {
                    child(v)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}
fn binary(token: &Token) -> Option<(u8, B)> {
    Some(match token {
        Token::Caret => (power::POWER, B::Power),
        Token::Mul => (power::MULTIPLICATION, B::Multiply),
        Token::Div => (power::MULTIPLICATION, B::Divide),
        Token::Backslash => (power::INTEGER_DIVISION, B::IntegerDivide),
        Token::Plus => (power::ADDITION, B::Add),
        Token::Minus => (power::ADDITION, B::Subtract),
        Token::Ampersand => (power::CONCATENATION, B::Concatenate),
        Token::Eq => (power::COMPARISON, B::Equal),
        Token::Neq => (power::COMPARISON, B::NotEqual),
        Token::Lt => (power::COMPARISON, B::Less),
        Token::LtEq => (power::COMPARISON, B::LessOrEqual),
        Token::Gt => (power::COMPARISON, B::Greater),
        Token::GtEq => (power::COMPARISON, B::GreaterOrEqual),
        Token::Word(w) if w.quote_style.is_none() => match w.value.to_ascii_uppercase().as_str() {
            "MOD" => (power::MODULO, B::Modulo),
            "AND" => (power::AND, B::And),
            "OR" => (power::OR, B::Or),
            "XOR" => (power::XOR, B::Xor),
            "EQV" => (power::EQUIVALENCE, B::Equivalence),
            "IMP" => (power::IMPLICATION, B::Implication),
            _ => return None,
        },
        _ => return None,
    })
}
fn pratt(
    parser: &mut Parser<'_>,
    limits: &super::limits::ParseContext,
    min: u8,
    recursion: usize,
) -> Result<E, ParseError> {
    if recursion > super::limits::MAX_RECURSION {
        return Err(ParseError::at(
            parser,
            K::Limit,
            "SQL parser recursion limit exceeded",
        ));
    }
    let _guard = limits.enter(parser)?;
    let mut left = prefix(parser, limits, recursion + 1)?;
    loop {
        if let Some((precedence, op)) = binary(&parser.peek_token().token) {
            if precedence < min {
                break;
            }
            parser.next_token();
            if matches!(
                op,
                B::Equal | B::NotEqual | B::Less | B::LessOrEqual | B::Greater | B::GreaterOrEqual
            ) {
                let quantifier = if parser.parse_keyword(Keyword::ALL) {
                    Some(crate::select::Quantifier::All)
                } else if parser.parse_keyword(Keyword::ANY) {
                    Some(crate::select::Quantifier::Any)
                } else if parser.parse_keyword(Keyword::SOME) {
                    Some(crate::select::Quantifier::Some)
                } else {
                    None
                };
                if let Some(quantifier) = quantifier {
                    expect_token(parser, Token::LParen)?;
                    let query = super::queries::read(parser, limits)?;
                    expect_token(parser, Token::RParen)?;
                    left = E::Quantified {
                        expression: Box::new(left),
                        operator: op,
                        quantifier,
                        query: Box::new(query),
                    };
                    continue;
                }
            }
            let right = pratt(parser, limits, precedence + 1, recursion + 1)?;
            left = E::Binary {
                left: Box::new(left),
                operator: op,
                right: Box::new(right),
            };
            continue;
        }
        if min > power::COMPARISON {
            break;
        }
        let negated = parser.parse_keyword(Keyword::NOT);
        if parser.parse_keyword(Keyword::BETWEEN) {
            let first = pratt(parser, limits, power::COMPARISON + 1, recursion + 1)?;
            if !parser.parse_keyword(Keyword::AND) {
                return Err(ParseError::at(parser, K::Syntax, "BETWEEN requires AND"));
            }
            let second = pratt(parser, limits, power::COMPARISON + 1, recursion + 1)?;
            left = E::Between {
                expression: Box::new(left),
                first_bound: Box::new(first),
                second_bound: Box::new(second),
                negated,
            };
        } else if parser.parse_keyword(Keyword::IN) {
            expect_token(parser, Token::LParen)?;
            if matches!(parser.peek_token().token, Token::Word(ref w) if w.keyword == Keyword::SELECT)
            {
                let query = super::queries::read(parser, limits)?;
                expect_token(parser, Token::RParen)?;
                left = E::InQuery {
                    expression: Box::new(left),
                    query: Box::new(query),
                    negated,
                };
                continue;
            }
            let mut values = vec![pratt(parser, limits, 0, recursion + 1)?];
            while parser.consume_token(&Token::Comma) {
                values.push(pratt(parser, limits, 0, recursion + 1)?);
            }
            expect_token(parser, Token::RParen)?;
            left = E::InList {
                expression: Box::new(left),
                values,
                negated,
            };
        } else if parser.parse_keyword(Keyword::LIKE) {
            let pattern = pratt(parser, limits, power::COMPARISON + 1, recursion + 1)?;
            left = E::Like {
                expression: Box::new(left),
                pattern: Box::new(pattern),
                negated,
            };
        } else if negated {
            return Err(ParseError::at(
                parser,
                K::Syntax,
                "NOT must precede IN, BETWEEN or LIKE",
            ));
        } else if parser.parse_keyword(Keyword::IS) {
            let negated = parser.parse_keyword(Keyword::NOT);
            if !parser.parse_keyword(Keyword::NULL) {
                return Err(ParseError::at(
                    parser,
                    K::Unsupported,
                    "only IS NULL and IS NOT NULL are Access predicates",
                ));
            }
            left = E::IsNull {
                expression: Box::new(left),
                negated,
            };
        } else {
            break;
        }
    }
    Ok(left)
}
fn prefix(
    parser: &mut Parser<'_>,
    limits: &super::limits::ParseContext,
    recursion: usize,
) -> Result<E, ParseError> {
    let token = parser.peek_token();
    if parser.consume_token(&Token::Plus) || parser.consume_token(&Token::Minus) {
        let minus = token.token == Token::Minus;
        let value = pratt(parser, limits, power::UNARY_SIGN, recursion)?;
        if let E::Integer(n) = value {
            if !minus {
                return Ok(E::Integer(n));
            }
            if let Some(n) = n.checked_neg() {
                return Ok(E::Integer(n));
            }
            return Ok(E::Unary {
                operator: U::Minus,
                expression: Box::new(E::Integer(n)),
            });
        }
        if let E::Number(ref n) = value
            && minus
            && let Ok(n) = format!("-{n}").parse::<i64>()
        {
            return Ok(E::Integer(n));
        }
        return Ok(E::Unary {
            operator: if minus { U::Minus } else { U::Plus },
            expression: Box::new(value),
        });
    }
    if parser.consume_token(&Token::Ampersand) {
        let digits = parser.next_token();
        if digits.span.start != token.span.end {
            return Err(ParseError::at(
                parser,
                K::Syntax,
                "radix literal prefix must be adjacent",
            ));
        }
        let Token::Word(value) = digits.token else {
            return Err(ParseError::at(
                parser,
                K::Syntax,
                "invalid hexadecimal or octal literal",
            ));
        };
        if value.quote_style.is_some() {
            return Err(ParseError::at(
                parser,
                K::Syntax,
                "radix literal digits cannot be quoted",
            ));
        }
        let raw = value.value;
        let (prefix, digits) =
            raw.split_at(raw.char_indices().nth(1).map_or(raw.len(), |(i, _)| i));
        let valid = !digits.is_empty()
            && match prefix.to_ascii_uppercase().as_str() {
                "H" => digits.bytes().all(|b| b.is_ascii_hexdigit()),
                "O" => digits.bytes().all(|b| matches!(b, b'0'..=b'7')),
                _ => false,
            };
        if !valid {
            return Err(ParseError::at(
                parser,
                K::Syntax,
                "invalid hexadecimal or octal literal",
            ));
        }
        return Ok(E::Number(format!(
            "&{}{}",
            prefix.to_ascii_uppercase(),
            digits
        )));
    }
    if parser.parse_keyword(Keyword::NOT) {
        return Ok(E::Unary {
            operator: U::Not,
            expression: Box::new(pratt(parser, limits, power::NOT, recursion)?),
        });
    }
    if parser.parse_keyword(Keyword::EXISTS) {
        expect_token(parser, Token::LParen)?;
        let query = super::queries::read(parser, limits)?;
        expect_token(parser, Token::RParen)?;
        return Ok(E::Exists(Box::new(query)));
    }
    if parser.consume_token(&Token::AtAt) {
        if !word(parser, "IDENTITY") {
            return Err(ParseError::at(
                parser,
                K::Unsupported,
                "only @@IDENTITY is supported",
            ));
        }
        return Ok(E::Identity);
    }
    let mut value = match token.token {
        Token::LParen => {
            parser.next_token();
            if matches!(parser.peek_token().token, Token::Word(ref w) if w.keyword == Keyword::SELECT)
            {
                let query = super::queries::read(parser, limits)?;
                expect_token(parser, Token::RParen)?;
                return Ok(E::Subquery(Box::new(query)));
            }
            let value = pratt(parser, limits, 0, recursion)?;
            expect_token(parser, Token::RParen)?;
            value
        }
        Token::Number(ref n, false) if valid_number(n) => {
            parser.next_token();
            n.parse::<i64>()
                .map(E::Integer)
                .unwrap_or_else(|_| E::Number(n.clone()))
        }
        Token::SingleQuotedString(ref text) | Token::DoubleQuotedString(ref text) => {
            parser.next_token();
            E::Text(text.clone())
        }
        Token::Word(ref w) if w.quote_style == Some('{') => {
            parser.next_token();
            E::GuidLiteral(w.value.clone())
        }
        Token::Word(ref w) if w.quote_style == Some('#') => {
            parser.next_token();
            E::DateLiteral(w.value.clone())
        }
        Token::Word(ref w) if w.quote_style.is_none() && w.keyword == Keyword::NULL => {
            parser.next_token();
            E::Null
        }
        Token::Word(ref w)
            if w.quote_style.is_none() && matches!(w.keyword, Keyword::TRUE | Keyword::FALSE) =>
        {
            parser.next_token();
            E::Boolean(w.keyword == Keyword::TRUE)
        }
        Token::Word(ref w) => {
            if w.quote_style.is_none()
                && matches!(
                    w.value.to_ascii_uppercase().as_str(),
                    "SELECT" | "CASE" | "CAST" | "CONVERT" | "EXISTS"
                )
            {
                return Err(ParseError::at(
                    parser,
                    K::Unsupported,
                    "unsupported expression construct",
                ));
            }
            if super::calls::lookahead(parser) {
                super::calls::parse(parser, limits, recursion)?
            } else {
                E::Field(FieldReference {
                    qualifier: None,
                    name: reference_names::read(parser)?,
                })
            }
        }
        _ => {
            return Err(ParseError::at(
                parser,
                K::Syntax,
                "expected an Access expression",
            ));
        }
    };
    loop {
        let bang = parser.consume_token(&Token::ExclamationMark);
        if !bang && !parser.consume_token(&Token::Period) {
            break;
        }
        if let E::Field(FieldReference {
            qualifier: None,
            name,
        }) = &value
            && reference_names::prompt_only(name)
        {
            return Err(ParseError::at(
                parser,
                K::Unsupported,
                "parameter prompt cannot qualify a member",
            ));
        }
        if !matches!(value, E::Field(_) | E::Member { .. } | E::Call { .. }) {
            return Err(ParseError::at(
                parser,
                K::Syntax,
                "member access requires an object or identifier",
            ));
        }
        // A dot selects a member, so a keyword here cannot introduce a clause.
        // Root names, aliases and bang access retain their separate name rules.
        let member = if bang {
            access::name(parser)?
        } else {
            reference_names::member(parser)?
        };
        value = match value {
            E::Field(FieldReference {
                qualifier: None,
                name,
            }) if !bang => E::Field(FieldReference {
                qualifier: Some(name),
                name: member,
            }),
            _ => E::Member {
                expression: Box::new(value),
                member,
                bang,
            },
        };
    }
    Ok(value)
}
fn valid_number(value: &str) -> bool {
    let mut split = value.split(['e', 'E']);
    let mantissa = split.next().unwrap_or_default();
    if let Some(exponent) = split.next() {
        let exponent = exponent.trim_start_matches(['+', '-']);
        if exponent.is_empty() || !exponent.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    if split.next().is_some() {
        return false;
    }
    let mut dot = mantissa.split('.');
    let first = dot.next().unwrap_or_default();
    let second = dot.next().unwrap_or_default();
    dot.next().is_none()
        && !(first.is_empty() && second.is_empty())
        && first
            .bytes()
            .chain(second.bytes())
            .all(|b| b.is_ascii_digit())
}

pub(super) fn argument(
    parser: &mut Parser<'_>,
    limits: &super::limits::ParseContext,
    recursion: usize,
) -> Result<E, ParseError> {
    pratt(parser, limits, 0, recursion)
}
