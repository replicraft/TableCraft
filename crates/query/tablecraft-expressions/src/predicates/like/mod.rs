//! Bounded wildcard matching with explicit scalar/UTF-16 policies.
mod pattern;
mod utf16;
pub use crate::comparison::StringInput as LikeString;
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

/// The host selects the database/query wildcard convention; it is not inferred.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LikeMode {
    /// Access wildcards: `*`, `?`, `#` and `[!list]`.
    Ansi89,
    /// SQL-compatible wildcards: `%`, `_` and `[^list]`.
    Ansi92,
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
    if collation.like_profile() == LikeProfile::Utf16Weights {
        return utf16::matches(
            LikeString::Text(text),
            LikeString::Text(pattern),
            mode,
            collation,
            &mut work,
        );
    }
    let tokens = pattern::compile(pattern, mode, collation, &mut work)?;
    let width = tokens.len().checked_add(1).ok_or(EvalError::Allocation)?;
    let mut previous = work.buffer::<u8>(width)?;
    let mut current = work.buffer::<u8>(width)?;
    previous.resize(width, 0);
    current.resize(width, 0);
    previous[0] = 1;
    for (i, token) in tokens.iter().enumerate() {
        work.tick()?;
        previous[i + 1] = u8::from(matches!(token, Token::Many) && previous[i] != 0);
    }
    for character in text.chars() {
        work.tick()?;
        current[0] = 0;
        for (i, token) in tokens.iter().enumerate() {
            work.tick()?;
            current[i + 1] = u8::from(match token {
                Token::Many => previous[i + 1] != 0 || current[i] != 0,
                _ => previous[i] != 0 && token.matches(character, collation, &mut work)?,
            });
        }
        std::mem::swap(&mut previous, &mut current);
    }
    work.tick()?;
    Ok(if previous[width - 1] != 0 {
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
    value: LikeString<'_>,
    pattern: LikeString<'_>,
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
mod raw_tests;
