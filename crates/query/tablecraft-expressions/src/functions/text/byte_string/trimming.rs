//! Allocation-free U+0020 trimming over the active UTF-16 String range.
use super::ByteString;
use crate::{EvalError, functions::text::TextFunction};

const UNIT_BYTES: usize = size_of::<u16>();
const SPACE_UNIT: u16 = b' ' as u16;

/// Retain a trimmed range without Unicode decoding or copying its backing buffer.
/// Complete units may be isolated surrogates or start at an unaligned byte offset.
/// Odd byte counts are refused. Only LTrim/RTrim/Trim are admitted; other
/// whitespace retains its units. Each edge examination checks caller work and
/// cancellation through step. Final publication remains the caller's decision.
pub fn trim_spaces(
    function: TextFunction,
    value: ByteString,
    mut step: impl FnMut() -> Result<(), EvalError>,
) -> Result<ByteString, EvalError> {
    if !function.trims_spaces() {
        return Err(EvalError::UnsupportedExpression);
    }

    step()?;
    let (units, remainder) = value.as_bytes().as_chunks::<UNIT_BYTES>();
    if !remainder.is_empty() {
        return Err(EvalError::UnrepresentableText);
    }

    let mut start = 0;
    let mut end = units.len();
    if function != TextFunction::RTrim {
        while start < end {
            step()?;
            if u16::from_le_bytes(units[start]) != SPACE_UNIT {
                break;
            }
            start += 1;
        }
    }
    if function != TextFunction::LTrim {
        while start < end {
            step()?;
            if u16::from_le_bytes(units[end - 1]) != SPACE_UNIT {
                break;
            }
            end -= 1;
        }
    }

    value.select(start * UNIT_BYTES..end * UNIT_BYTES)
}

#[cfg(test)]
mod tests;
