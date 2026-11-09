//! Explicit UTF-16 wildcard counts and weighted literal/element transitions.
use crate::comparison::StringInput;
mod classes;
mod pattern;
pub(super) mod search;
use super::{LikeMode, Work, reachable};
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
    origins: Option<Origins>,
}

pub(super) struct Origins {
    previous: Vec<usize>,
    current: Vec<usize>,
    reverse: bool,
}

pub(super) fn matches(
    text: StringInput<'_>,
    pattern: StringInput<'_>,
    mode: LikeMode,
    col: &dyn Collation,
    work: &mut Work<'_>,
) -> Result<Truth, EvalError> {
    let pattern = pattern::compile(pattern, mode, work)?;
    let malformed = matches!(pattern.tokens.last(), Some(Token::Invalid));
    let units = text.units(work.budget, work.cancelled)?;
    let width = units.len().checked_add(1).ok_or(EvalError::Allocation)?;
    let mut states = States {
        previous: work.buffer(width)?,
        current: work.buffer(width)?,
        candidates: work.buffer(width)?,
        origins: None,
    };
    states.previous.resize(width, 0);
    states.current.resize(width, 0);
    states.candidates.resize(width, 0);
    states.previous[0] = 1;
    for (number, token) in pattern.tokens.iter().enumerate() {
        work.tick()?;
        if !reachable(&states.previous, work)? {
            return Ok(Truth::False);
        }
        for flag in &mut states.current {
            work.tick()?;
            *flag = 0;
        }
        let last = number + 1 == pattern.tokens.len();
        if !states.advance(&units, &pattern, token, malformed, last, col, work)? {
            return Ok(Truth::False);
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
    fn merge(origins: &Origins, old: usize, incoming: usize) -> usize {
        if old == usize::MAX {
            incoming
        } else if origins.reverse {
            old.max(incoming)
        } else {
            old.min(incoming)
        }
    }

    fn mark(&mut self, end: usize, start: usize) {
        self.current[end] = 1;
        if let Some(origins) = &mut self.origins {
            origins.current[end] =
                Self::merge(origins, origins.current[end], origins.previous[start]);
        }
    }

    fn extend_many(&mut self, end: usize, previous_end: usize) {
        self.current[end] = 1;
        if let Some(origins) = &mut self.origins {
            origins.current[end] =
                Self::merge(origins, origins.current[end], origins.current[previous_end]);
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "SQL timing, pattern state and host comparison controls remain independent"
    )]
    fn advance(
        &mut self,
        units: &[u16],
        pattern: &pattern::Pattern,
        token: &Token,
        malformed: bool,
        last: bool,
        col: &dyn Collation,
        work: &mut Work<'_>,
    ) -> Result<bool, EvalError> {
        let width = units.len() + 1;
        match token {
            Token::Invalid => {
                if reachable(&self.previous[..units.len()], work)? {
                    return Err(EvalError::InvalidPattern);
                }
                return Ok(false);
            }
            Token::Many => {
                if malformed {
                    return Err(EvalError::InvalidPattern);
                }
                for at in 0..width {
                    work.tick()?;
                    if self.previous[at] != 0 {
                        self.mark(at, at);
                    }
                    if at > 0 && self.current[at - 1] != 0 {
                        self.extend_many(at, at - 1);
                    }
                }
            }
            Token::One | Token::Digit => {
                for (at, unit) in units.iter().enumerate() {
                    work.tick()?;
                    if self.previous[at] != 0
                        && (matches!(token, Token::One) || (ZERO..=NINE).contains(unit))
                    {
                        self.mark(at + 1, at);
                    }
                }
            }
            Token::Literal(range) => {
                // Borrow literal units selected by a borrowed compiled token; cloning bounds
                // avoids moving its range out of the reusable compiled pattern.
                self.literal(units, &pattern.literals[range.clone()], last, col, work)?
            }
            Token::Class(range, negated) => {
                if !reachable(&self.previous[..units.len()], work)? {
                    return Ok(false);
                }
                // Borrow class units selected by reusable compiled token; only index bounds are
                // copied, not class storage.
                let list = &pattern.units[range.clone()];
                for range in pattern::ranges(list) {
                    let (low, high) = range?;
                    if compare(&[low], &[high], col, work)? == Ordering::Greater {
                        return Err(EvalError::InvalidPattern);
                    }
                }
                self.class(units, list, *negated, last, col, work)?;
            }
        }
        Ok(true)
    }

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
                    LikeSpanState::Match => self.mark(end, start),
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
