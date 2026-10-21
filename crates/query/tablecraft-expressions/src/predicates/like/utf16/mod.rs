//! Explicit UTF-16 wildcard counts and weighted literal/element transitions.
mod classes;
mod pattern;
use super::{LikeMode, Work};
use crate::{
    EvalError,
    comparison::{Collation, LikeSpanState},
    logic::Truth,
};
use pattern::Token;
use std::cmp::Ordering;

const ZERO: u16 = b'0' as u16;
const NINE: u16 = b'9' as u16;

pub(super) struct States {
    previous: Vec<u8>,
    current: Vec<u8>,
    candidates: Vec<u8>,
}

pub(super) fn matches(
    text: &str,
    pattern: &str,
    mode: LikeMode,
    col: &dyn Collation,
    work: &mut Work<'_>,
) -> Result<Truth, EvalError> {
    let pattern = pattern::compile(pattern, mode, col, work)?;
    let mut units = work.buffer(text.len())?;
    for unit in text.encode_utf16() {
        work.tick()?;
        units.push(unit);
    }
    let width = units.len().checked_add(1).ok_or(EvalError::Allocation)?;
    let mut states = States {
        previous: work.buffer(width)?,
        current: work.buffer(width)?,
        candidates: work.buffer(width)?,
    };
    states.previous.resize(width, 0);
    states.current.resize(width, 0);
    states.candidates.resize(width, 0);
    states.previous[0] = 1;
    for (number, token) in pattern.tokens.iter().enumerate() {
        work.tick()?;
        for flag in &mut states.current {
            work.tick()?;
            *flag = 0;
        }
        let last = number + 1 == pattern.tokens.len();
        match token {
            Token::Many => {
                for at in 0..width {
                    work.tick()?;
                    states.current[at] = u8::from(
                        states.previous[at] != 0 || (at > 0 && states.current[at - 1] != 0),
                    );
                }
            }
            Token::One | Token::Digit => {
                for (at, unit) in units.iter().enumerate() {
                    work.tick()?;
                    states.current[at + 1] = u8::from(
                        states.previous[at] != 0
                            && (matches!(token, Token::One) || (ZERO..=NINE).contains(unit)),
                    );
                }
            }
            Token::Literal(range) => {
                states.literal(&units, &pattern.literals[range.clone()], last, col, work)?
            }
            Token::Class(range, negated) => states.class(
                &units,
                &pattern.units[range.clone()],
                *negated,
                last,
                col,
                work,
            )?,
        }
        std::mem::swap(&mut states.previous, &mut states.current);
    }
    work.tick()?;
    Ok(if states.previous[units.len()] != 0 {
        Truth::True
    } else {
        Truth::False
    })
}

impl States {
    fn literal(
        &mut self,
        text: &[u16],
        literal: &[u16],
        last: bool,
        col: &dyn Collation,
        work: &mut Work<'_>,
    ) -> Result<(), EvalError> {
        for start in 0..=text.len() {
            work.tick()?;
            if self.previous[start] == 0 {
                continue;
            }
            let first = if last { text.len() } else { start };
            for end in first..=text.len() {
                work.tick()?;
                work.tick()?;
                let state = col.like_literal_step(&text[start..end], literal)?;
                work.tick()?;
                match state {
                    LikeSpanState::Match => self.current[end] = 1,
                    LikeSpanState::Possible => {}
                    LikeSpanState::Impossible => break,
                }
            }
        }
        Ok(())
    }
}

fn compare(
    left: &[u16],
    right: &[u16],
    col: &dyn Collation,
    work: &mut Work<'_>,
) -> Result<Ordering, EvalError> {
    work.tick()?;
    let result = col.compare_like_units(left, right)?;
    work.tick()?;
    Ok(result)
}

#[cfg(test)]
mod tests;
