//! Metered, allocation-free search over exact raw string bytes.
use super::ByteString;
use crate::EvalError;
#[cfg(feature = "sql")]
use crate::comparison::StringCompareMode;
#[cfg(feature = "sql")]
use tablecraft_core::values::Value;

const FIRST_POSITION: i32 = 1;
#[cfg(feature = "sql")]
const LAST_STANDARD_MODE: i32 = 2;

/// First one-based byte match at or after `start`, or zero. Empty haystacks and
/// out-of-range starts return zero; an empty needle otherwise returns `start`.
/// Each compared byte checks the host's work/cancellation callback.
pub fn find_binary(
    haystack: &ByteString,
    needle: &ByteString,
    start: i32,
    mut step: impl FnMut() -> Result<(), EvalError>,
) -> Result<i32, EvalError> {
    step()?;
    if start < FIRST_POSITION {
        return Err(EvalError::Domain);
    }
    if haystack.is_empty() || start > haystack.len() {
        return Ok(0);
    }
    if needle.is_empty() {
        return Ok(start);
    }

    let suffix = &haystack.as_bytes()[(start - FIRST_POSITION) as usize..];
    for (offset, window) in suffix.windows(needle.as_bytes().len()).enumerate() {
        step()?;
        let mut equal = true;
        for (left, right) in window.iter().zip(needle.as_bytes()) {
            step()?;
            if left != right {
                equal = false;
                break;
            }
        }
        if equal {
            return start
                .checked_add(i32::try_from(offset).map_err(|_| EvalError::Overflow)?)
                .ok_or(EvalError::Overflow);
        }
    }
    Ok(0)
}

#[cfg(feature = "sql")]
pub(crate) fn mode(argument: Option<&Value>) -> Result<Option<StringCompareMode>, EvalError> {
    let code = super::super::compare::code(argument)?;
    if code.is_some_and(|code| code > LAST_STANDARD_MODE) {
        // LCID lookup is a future byte-string provider boundary, never binary fallback.
        return Err(EvalError::UnsupportedExpression);
    }
    super::super::compare::from_code(code, None)
}

#[cfg(test)]
mod tests;
