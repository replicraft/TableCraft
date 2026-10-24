//! Checked StrComp modes with metered, allocation-free binary scans.
use super::TextResult;
use crate::{
    EvalError,
    comparison::{Collation, StringCompareMode},
    functions::conversions::Conversion,
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
    apply_mode(arguments, mode, collation, step)
}

pub(crate) fn apply_mode(
    arguments: &[Value],
    mode: Option<StringCompareMode>,
    collation: Option<&dyn Collation>,
    step: &mut impl FnMut() -> Result<(), EvalError>,
) -> Result<TextResult<'static>, EvalError> {
    apply_operands(&arguments[0], &arguments[1], mode, collation, step)
}

pub(crate) fn apply_operands(
    left: &Value,
    right: &Value,
    mode: Option<StringCompareMode>,
    collation: Option<&dyn Collation>,
    step: &mut impl FnMut() -> Result<(), EvalError>,
) -> Result<TextResult<'static>, EvalError> {
    if matches!(left, Value::Null) || matches!(right, Value::Null) {
        return Ok(TextResult::Null);
    }
    let (Value::Text(left), Value::Text(right)) = (left, right) else {
        return Err(EvalError::TypeMismatch);
    };
    let mode = mode
        .or_else(|| collation.and_then(Collation::option_compare))
        .ok_or(EvalError::CollationRequired)?;
    let order = match mode {
        StringCompareMode::Binary => binary_units(left.encode_utf16(), right.encode_utf16(), step)?,
        StringCompareMode::Text | StringCompareMode::Database => {
            let collation = collation.ok_or(EvalError::CollationRequired)?;
            for _ in left.encode_utf16().chain(right.encode_utf16()) {
                step()?;
            }
            step()?;
            let order = if mode == StringCompareMode::Text {
                collation.compare_text(left, right)?
            } else {
                collation.compare(left, right)?
            };
            step()?;
            order
        }
    };
    Ok(TextResult::Comparison(match order {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }))
}

pub(crate) fn mode(
    argument: Option<&Value>,
    default: Option<StringCompareMode>,
) -> Result<Option<StringCompareMode>, EvalError> {
    from_code(code(argument)?, default)
}

pub(crate) fn binary_units(
    mut left: impl Iterator<Item = u16>,
    mut right: impl Iterator<Item = u16>,
    step: &mut impl FnMut() -> Result<(), EvalError>,
) -> Result<Ordering, EvalError> {
    loop {
        step()?;
        let order = match (left.next(), right.next()) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(a), Some(b)) if a != b => a.cmp(&b),
            _ => continue,
        };
        return Ok(order);
    }
}

pub(super) fn code(argument: Option<&Value>) -> Result<Option<i32>, EvalError> {
    match argument {
        None => Ok(None),
        Some(Value::Null) => Err(EvalError::InvalidNullConversion),
        Some(value) => {
            let Value::Int32(value) = Conversion::Long.apply(value)? else {
                return Err(EvalError::TypeMismatch);
            };
            Ok(Some(value))
        }
    }
}

pub(super) fn from_code(
    code: Option<i32>,
    default: Option<StringCompareMode>,
) -> Result<Option<StringCompareMode>, EvalError> {
    Ok(match code {
        None => default,
        Some(USE_OPTION) => None,
        Some(BINARY) => Some(StringCompareMode::Binary),
        Some(TEXT) => Some(StringCompareMode::Text),
        Some(DATABASE) => Some(StringCompareMode::Database),
        _ => return Err(EvalError::Domain),
    })
}

#[cfg(test)]
mod tests;
