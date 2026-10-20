//! Checked StrComp modes with metered, allocation-free binary scans.
use super::TextResult;
use crate::{
    EvalError,
    comparison::{Collation, StringCompareMode},
    numeric::{IntegerType, coerce_integer},
};
use std::cmp::Ordering;
use tablecraft_core::values::Value;

const USE_OPTION: i32 = -1;
const BINARY: i32 = 0;
const TEXT: i32 = 1;
const DATABASE: i32 = 2;

pub(super) fn apply(
    arguments: &[Value],
    collation: Option<&dyn Collation>,
    step: &mut impl FnMut() -> Result<(), EvalError>,
) -> Result<TextResult<'static>, EvalError> {
    // Validate the optional argument even when a string operand is NULL.
    let mode = mode(arguments.get(2), None)?;
    if matches!(arguments[0], Value::Null) || matches!(arguments[1], Value::Null) {
        return Ok(TextResult::Null);
    }
    let (Value::Text(left), Value::Text(right)) = (&arguments[0], &arguments[1]) else {
        return Err(EvalError::TypeMismatch);
    };
    let mode = mode
        .or_else(|| collation.and_then(Collation::option_compare))
        .ok_or(EvalError::CollationRequired)?;
    let order = match mode {
        StringCompareMode::Binary => {
            let mut left = left.encode_utf16();
            let mut right = right.encode_utf16();
            loop {
                step()?;
                match (left.next(), right.next()) {
                    (None, None) => break Ordering::Equal,
                    (None, Some(_)) => break Ordering::Less,
                    (Some(_), None) => break Ordering::Greater,
                    (Some(a), Some(b)) if a != b => break a.cmp(&b),
                    _ => {}
                }
            }
        }
        StringCompareMode::Text | StringCompareMode::Database => {
            let collation = collation.ok_or(EvalError::CollationRequired)?;
            for _ in left.encode_utf16().chain(right.encode_utf16()) {
                step()?;
            }
            step()?;
            if mode == StringCompareMode::Text {
                collation.compare_text(left, right)?
            } else {
                collation.compare(left, right)?
            }
        }
    };
    Ok(TextResult::Comparison(match order {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }))
}

pub(super) fn mode(
    argument: Option<&Value>,
    default: Option<StringCompareMode>,
) -> Result<Option<StringCompareMode>, EvalError> {
    Ok(match argument {
        None => default,
        Some(Value::Null) => return Err(EvalError::InvalidNullConversion),
        Some(value) => {
            let Value::Int32(value) = coerce_integer(value, IntegerType::Int32)? else {
                return Err(EvalError::TypeMismatch);
            };
            match value {
                USE_OPTION => None,
                BINARY => Some(StringCompareMode::Binary),
                TEXT => Some(StringCompareMode::Text),
                DATABASE => Some(StringCompareMode::Database),
                _ => return Err(EvalError::Domain),
            }
        }
    })
}

#[cfg(test)]
mod tests;
