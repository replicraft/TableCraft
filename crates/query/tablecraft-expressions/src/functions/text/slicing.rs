//! Borrowed UTF-16 slices with checked Long positions and no source cloning.
use super::{TextFunction, TextResult, boundary, units};
use crate::{EvalError, functions::conversions::Conversion};
use tablecraft_core::values::Value;

pub(super) fn long(value: &Value) -> Result<i32, EvalError> {
    if matches!(value, Value::Null) {
        return Err(EvalError::TypeMismatch);
    }
    match Conversion::Long.apply(value)? {
        Value::Int32(value) => Ok(value),
        _ => Err(EvalError::TypeMismatch),
    }
}

pub(crate) fn apply<'a>(
    function: TextFunction,
    text: &'a str,
    position: i32,
    length: Option<i32>,
    step: &mut impl FnMut() -> Result<(), EvalError>,
) -> Result<TextResult<'a>, EvalError> {
    let length = if function == TextFunction::Mid {
        if position < 1 {
            return Err(EvalError::Domain);
        }
        length
    } else {
        Some(position)
    };
    if length.is_some_and(|length| length < 0) {
        return Err(EvalError::Domain);
    }
    let total = units(text, step)?;
    let (start, end) = match function {
        TextFunction::Left => (0, position.min(total)),
        TextFunction::Right => (total.saturating_sub(position).max(0), total),
        TextFunction::Mid => {
            let start = (position - 1).min(total);
            (
                start,
                start.saturating_add(length.unwrap_or(total)).min(total),
            )
        }
        _ => return Err(EvalError::UnsupportedExpression),
    };
    if start == end {
        return Ok(TextResult::Text(""));
    }
    let start = boundary(text, start, step)?;
    let end = boundary(text, end, step)?;
    Ok(TextResult::Text(&text[start..end]))
}

#[cfg(test)]
mod tests;
