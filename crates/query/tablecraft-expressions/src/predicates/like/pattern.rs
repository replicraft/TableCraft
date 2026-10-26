use super::{LikeMode, Work};
use crate::{EvalError, comparison::Collation};
use std::cmp::Ordering;

const UTF8_CHAR_BYTES: usize = char::MAX.len_utf8();

pub(super) enum Token<'a> {
    Invalid,
    Many,
    One,
    Digit,
    Literal(char),
    Class(&'a str, bool),
}
impl Token<'_> {
    pub(super) fn matches(
        &self,
        value: char,
        collation: &dyn Collation,
        work: &mut Work<'_>,
    ) -> Result<bool, EvalError> {
        Ok(match self {
            Self::Invalid => return Err(EvalError::InvalidPattern),
            Self::Many | Self::One => true,
            Self::Digit => value.is_ascii_digit(),
            Self::Literal(literal) => compare(*literal, value, collation, work)? == Ordering::Equal,
            Self::Class(list, negated) => {
                let mut found = false;
                for range in ranges(list) {
                    let (low, high) = range?;
                    work.tick()?;
                    found |= compare(low, value, collation, work)? != Ordering::Greater
                        && compare(value, high, collation, work)? != Ordering::Greater;
                }
                found != *negated
            }
        })
    }
}
fn compare(
    a: char,
    b: char,
    collation: &dyn Collation,
    work: &mut Work<'_>,
) -> Result<Ordering, EvalError> {
    work.tick()?;
    let mut left = [0; UTF8_CHAR_BYTES];
    let mut right = [0; UTF8_CHAR_BYTES];
    let order = collation.compare(a.encode_utf8(&mut left), b.encode_utf8(&mut right))?;
    work.tick()?;
    Ok(order)
}
fn ranges(list: &str) -> impl Iterator<Item = Result<(char, char), EvalError>> + '_ {
    let mut chars = list.char_indices().peekable();
    std::iter::from_fn(move || {
        let (offset, first) = chars.next()?;
        if first == '-' {
            return Some(if offset == 0 || offset + first.len_utf8() == list.len() {
                Ok((first, first))
            } else {
                Err(EvalError::InvalidPattern)
            });
        }
        if chars.peek().is_some_and(|(_, c)| *c == '-') {
            let mut rest = chars.clone();
            rest.next();
            if let Some((_, last)) = rest.next() {
                chars = rest;
                return Some(Ok((first, last)));
            }
        }
        Some(Ok((first, first)))
    })
}
pub(super) fn compile<'a>(
    pattern: &'a str,
    mode: LikeMode,
    work: &mut Work<'_>,
) -> Result<Vec<Token<'a>>, EvalError> {
    // UTF-8 byte length bounds the number of tokens, without a separate uncharged scan.
    let mut tokens = work.buffer(pattern.len())?;
    let (many, one, not) = match mode {
        LikeMode::Ansi89 => ('*', '?', '!'),
        LikeMode::Ansi92 => ('%', '_', '^'),
    };
    let mut chars = pattern.char_indices();
    while let Some((_, character)) = chars.next() {
        work.tick()?;
        let token = match character {
            c if c == many => Token::Many,
            c if c == one => Token::One,
            '#' if mode == LikeMode::Ansi89 => Token::Digit,
            '[' => {
                let start = chars.clone().next().map_or(pattern.len(), |(i, _)| i);
                let mut end = None;
                for (i, c) in chars.by_ref() {
                    work.tick()?;
                    if c == ']' {
                        end = Some(i);
                        break;
                    }
                }
                let Some(end) = end else {
                    tokens.push(Token::Invalid);
                    break;
                };
                let list = &pattern[start..end];
                // Access [] denotes a zero-length string, not an empty character set.
                if list.is_empty() {
                    continue;
                }
                let negated = list.starts_with(not);
                let list = if negated {
                    &list[not.len_utf8()..]
                } else {
                    list
                };
                if list.is_empty() {
                    tokens.push(Token::Invalid);
                    break;
                }
                for range in ranges(list) {
                    work.tick()?;
                    if range.is_err() {
                        tokens.push(Token::Invalid);
                        return Ok(tokens);
                    }
                }
                Token::Class(list, negated)
            }
            c => Token::Literal(c),
        };
        tokens.push(token);
    }
    Ok(tokens)
}

/// Range ordering is semantic, so providers run only for a reached class.
pub(super) fn validate_class(
    token: &Token<'_>,
    collation: &dyn Collation,
    work: &mut Work<'_>,
) -> Result<(), EvalError> {
    if let Token::Class(list, _) = token {
        for range in ranges(list) {
            let (low, high) = range?;
            if compare(low, high, collation, work)? == Ordering::Greater {
                return Err(EvalError::InvalidPattern);
            }
        }
    }
    Ok(())
}
