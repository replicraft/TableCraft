//! Bounded wildcard matching with explicit scalar/UTF-16 policies.
use crate::comparison::StringInput;
mod pattern;
mod utf16;
use crate::{
    EvalError,
    comparison::{Collation, LikeProfile},
    logic::Truth,
};
use pattern::Token;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
pub use utf16::search::PreparedLikeSearch;

const HIGH_SURROGATES: std::ops::RangeInclusive<u16> = 0xd800..=0xdbff;
const LOW_SURROGATES: std::ops::RangeInclusive<u16> = 0xdc00..=0xdfff;

/// The host selects the database/query wildcard convention; it is not inferred.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LikeMode {
    /// Access wildcards: `*`, `?`, `#` and `[!list]`.
    Ansi89,
    /// SQL-compatible wildcards: `%`, `_` and `[^list]`.
    Ansi92,
}

/// Find alignment within one value, independent from SQL LIKE's whole operand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LikeMatch {
    /// Match at any source boundary.
    Any,
    /// Consume the entire source value.
    Whole,
    /// Begin at the source value's first boundary.
    Start,
}

/// An explicit provider capability, independent from SQL/module comparison mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LikeCase {
    /// Existing database-weighted literal and class policy.
    Database,
    /// Separately admitted case-sensitive provider callbacks.
    Sensitive,
}

/// Locate a consumed UTF-16 span using one compiled wildcard frontier.
/// Earliest start wins forward; latest start wins reverse. For that start,
/// choose the shortest nonempty span, or an empty span if none exists.
/// Bounds and returned endpoints never split a valid surrogate pair.
/// Empty spans are retained; consumers must advance with `next_like_offset`.
#[expect(
    clippy::too_many_arguments,
    reason = "matching, bounds and comparison policies are independent"
)]
pub fn find_like_utf16(
    text: StringInput<'_>,
    pattern: StringInput<'_>,
    mode: LikeMode,
    matching: LikeMatch,
    search: crate::comparison::StringSearch,
    case: LikeCase,
    collation: &dyn Collation,
    budget: &mut ResourceBudget,
    cancelled: Option<&dyn Fn() -> bool>,
) -> Result<Option<std::ops::Range<usize>>, EvalError> {
    PreparedLikeSearch::new(pattern, mode, matching, case, budget, cancelled)?
        .find(text, search, collation, budget, cancelled)
}

/// Advance past a zero-width match without bisecting a valid surrogate pair.
pub fn next_like_offset(units: &[u16], offset: usize) -> Option<usize> {
    let first = *units.get(offset)?;
    let width = if HIGH_SURROGATES.contains(&first)
        && units
            .get(offset + 1)
            .is_some_and(|next| LOW_SURROGATES.contains(next))
    {
        2
    } else {
        1
    };
    Some(offset + width)
}

/// Move before a zero-width match without bisecting a valid surrogate pair.
pub fn previous_like_offset(units: &[u16], offset: usize) -> Option<usize> {
    if offset == 0 || offset > units.len() {
        return None;
    }
    let last = units[offset - 1];
    let width = if offset >= 2
        && LOW_SURROGATES.contains(&last)
        && HIGH_SURROGATES.contains(&units[offset - 2])
    {
        2
    } else {
        1
    };
    Some(offset - width)
}

pub(super) struct Work<'a> {
    budget: &'a mut ResourceBudget,
    cancelled: Option<&'a dyn Fn() -> bool>,
}
impl Work<'_> {
    fn tick(&mut self) -> Result<(), EvalError> {
        if self.cancelled.is_some_and(|f| f()) {
            return Err(EvalError::Cancelled);
        }
        self.budget.charge(Resource::Steps, 1)?;
        Ok(())
    }
    fn buffer<T>(&mut self, count: usize) -> Result<Vec<T>, EvalError> {
        self.budget
            .charge_allocation(count as u64, std::mem::size_of::<T>() as u64)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| EvalError::Allocation)?;
        Ok(result)
    }
}

/// Match text with explicit wildcard and collation policies. NULL yields unknown.
/// No implicit numeric/date formatting, locale expansion or normalization occurs;
/// hosts can explicitly select UTF-16 weighted spans through LikeProfile.
/// Pattern compilation and every matching transition consume work; buffers consume
/// the shared byte budget. Matching is iterative, with no exponential backtracking.
/// Host collation callbacks must enforce their own work/time bounds.
pub fn like(
    value: &Value,
    pattern: &Value,
    mode: Option<LikeMode>,
    collation: Option<&dyn Collation>,
    budget: &mut ResourceBudget,
    cancelled: Option<&dyn Fn() -> bool>,
) -> Result<Truth, EvalError> {
    let mut work = Work { budget, cancelled };
    work.tick()?;
    let mode = mode.ok_or(EvalError::LikeModeRequired)?;
    if matches!(value, Value::Null) || matches!(pattern, Value::Null) {
        return Ok(Truth::Unknown);
    }
    let (Value::Text(text), Value::Text(pattern)) = (value, pattern) else {
        return Err(EvalError::TypeMismatch);
    };
    let collation = collation.ok_or(EvalError::CollationRequired)?;
    if collation.like_profile() == LikeProfile::Unsupported {
        return Err(EvalError::UnsupportedExpression);
    }
    if collation.like_profile() == LikeProfile::Utf16Weights {
        return utf16::matches(
            StringInput::Text(text),
            StringInput::Text(pattern),
            mode,
            collation,
            &mut work,
        );
    }
    let tokens = pattern::compile(pattern, mode, &mut work)?;
    let malformed = matches!(tokens.last(), Some(Token::Invalid));
    let width = text.len().checked_add(1).ok_or(EvalError::Allocation)?;
    let mut previous = work.buffer::<u8>(width)?;
    let mut current = work.buffer::<u8>(width)?;
    previous.resize(width, 0);
    current.resize(width, 0);
    previous[0] = 1;

    for token in &tokens {
        work.tick()?;
        if !reachable(&previous, &mut work)? {
            return Ok(Truth::False);
        }
        for flag in &mut current {
            work.tick()?;
            *flag = 0;
        }
        match token {
            Token::Many => {
                // A reached star validates the remaining grammar, without
                // invoking providers for unreached range-order semantics.
                if malformed {
                    return Err(EvalError::InvalidPattern);
                }
                current[0] = previous[0];
                for (at, character) in text.char_indices() {
                    work.tick()?;
                    let next = at + character.len_utf8();
                    current[next] = u8::from(previous[next] != 0 || current[at] != 0);
                }
            }
            Token::Invalid | Token::Class(..) => {
                if !reachable(&previous[..text.len()], &mut work)? {
                    return Ok(Truth::False);
                }
                if matches!(token, Token::Invalid) {
                    return Err(EvalError::InvalidPattern);
                }
                pattern::validate_class(token, collation, &mut work)?;
                for (at, character) in text.char_indices() {
                    work.tick()?;
                    if previous[at] != 0 && token.matches(character, collation, &mut work)? {
                        current[at + character.len_utf8()] = 1;
                    }
                }
            }
            _ => {
                for (at, character) in text.char_indices() {
                    work.tick()?;
                    if previous[at] != 0 && token.matches(character, collation, &mut work)? {
                        current[at + character.len_utf8()] = 1;
                    }
                }
            }
        }
        std::mem::swap(&mut previous, &mut current);
    }
    work.tick()?;
    Ok(if previous[text.len()] != 0 {
        Truth::True
    } else {
        Truth::False
    })
}

/// Match non-NULL Strings under an explicitly selected UTF-16 weight provider.
/// Inputs are borrowed; checked unit buffers and transitions consume the shared
/// allocation/work budget. No decoding, alignment cast or replacement occurs.
/// Scalar-only providers are refused; callers handle NULL and other value kinds.
pub fn like_utf16(
    value: StringInput<'_>,
    pattern: StringInput<'_>,
    mode: LikeMode,
    collation: &dyn Collation,
    budget: &mut ResourceBudget,
    cancelled: Option<&dyn Fn() -> bool>,
) -> Result<Truth, EvalError> {
    let mut work = Work { budget, cancelled };
    work.tick()?;
    if collation.like_profile() != LikeProfile::Utf16Weights {
        return Err(EvalError::UnsupportedExpression);
    }

    utf16::matches(value, pattern, mode, collation, &mut work)
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod timing_tests;

#[cfg(test)]
mod raw_tests;

/// A zero frontier proves an earlier mismatch before later pattern errors.
fn reachable(states: &[u8], work: &mut Work<'_>) -> Result<bool, EvalError> {
    for flag in states {
        work.tick()?;
        if *flag != 0 {
            return Ok(true);
        }
    }
    Ok(false)
}
