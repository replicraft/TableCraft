use super::super::{LikeMode, LikeString};
use super::Work;
use crate::EvalError;
use std::ops::Range;

const OPEN: u16 = b'[' as u16;
const CLOSE: u16 = b']' as u16;
const DASH: u16 = b'-' as u16;
const DIGIT: u16 = b'#' as u16;

pub(super) enum Token {
    Invalid,
    Many,
    One,
    Digit,
    Literal(Range<usize>),
    Class(Range<usize>, bool),
}
pub(super) struct Pattern {
    pub units: Vec<u16>,
    pub literals: Vec<u16>,
    pub tokens: Vec<Token>,
}

pub(super) fn compile(
    text: LikeString<'_>,
    mode: LikeMode,
    work: &mut Work<'_>,
) -> Result<Pattern, EvalError> {
    let units = text.units(work.budget, work.cancelled)?;
    let mut literals = work.buffer(units.len())?;
    let mut tokens = work.buffer(units.len())?;
    let (many, one, not) = match mode {
        LikeMode::Ansi89 => (b'*' as u16, b'?' as u16, b'!' as u16),
        LikeMode::Ansi92 => (b'%' as u16, b'_' as u16, b'^' as u16),
    };
    let mut run = None;
    let mut at = 0;
    while at < units.len() {
        work.tick()?;
        let unit = units[at];
        at += 1;
        let token = if unit == many {
            Token::Many
        } else if unit == one {
            Token::One
        } else if unit == DIGIT && mode == LikeMode::Ansi89 {
            Token::Digit
        } else if unit == OPEN {
            let mut start = at;
            while at < units.len() && units[at] != CLOSE {
                work.tick()?;
                at += 1;
            }
            if at == units.len() {
                if let Some(start) = run.take() {
                    tokens.push(Token::Literal(start..literals.len()));
                }
                tokens.push(Token::Invalid);
                break;
            }
            let end = at;
            at += 1;
            // Empty [] does not break adjacent literal runs/expansions.
            if start == end {
                continue;
            }
            let negated = units[start] == not;
            if negated {
                start += 1;
            }
            let mut valid = start != end;
            for range in ranges(&units[start..end]) {
                work.tick()?;
                if range.is_err() {
                    valid = false;
                    break;
                }
            }
            if valid {
                Token::Class(start..end, negated)
            } else {
                Token::Invalid
            }
        } else {
            if run.is_none() {
                run = Some(literals.len());
            }
            literals.push(unit);
            continue;
        };
        if let Some(start) = run.take() {
            tokens.push(Token::Literal(start..literals.len()));
        }
        let invalid = matches!(token, Token::Invalid);
        tokens.push(token);
        if invalid {
            break;
        }
    }
    if let Some(start) = run {
        tokens.push(Token::Literal(start..literals.len()));
    }
    work.tick()?;
    Ok(Pattern {
        units,
        literals,
        tokens,
    })
}

pub(super) fn ranges(list: &[u16]) -> impl Iterator<Item = Result<(u16, u16), EvalError>> + '_ {
    let mut at = 0;
    std::iter::from_fn(move || {
        let &low = list.get(at)?;
        let position = at;
        at += 1;
        if low == DASH {
            return Some(if position == 0 || at == list.len() {
                Ok((low, low))
            } else {
                Err(EvalError::InvalidPattern)
            });
        }
        if list.get(at) == Some(&DASH) && at + 1 < list.len() {
            let high = list[at + 1];
            at += 2;
            return Some(Ok((low, high)));
        }
        Some(Ok((low, low)))
    })
}
