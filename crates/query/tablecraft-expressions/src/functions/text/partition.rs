//! Numeric bucket labels without heap allocation or ambient formatting policies.
use super::{TextResult, format_buffer::FormatBuffer, output::TextOutput};
use crate::{EvalError, functions::conversions::Conversion};
use std::fmt::{self, Write};
use tablecraft_core::values::Value;

/// Validated range label, emitted through the host's charged output boundary.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Partition {
    lower: Option<i64>,
    upper: Option<i64>,
    width: usize,
}

impl fmt::Debug for Partition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Partition([redacted])")
    }
}

impl Partition {
    /// Four required operands: number, start, stop and interval.
    pub const ARGUMENTS: usize = 4;

    /// Emit one bounded ASCII label. The host must discard output on failure.
    pub fn write(self, output: &mut impl TextOutput) -> Result<(), EvalError> {
        output.step()?;
        let mut buffer = FormatBuffer::new();
        for (index, bound) in [self.lower, self.upper].into_iter().enumerate() {
            if index != 0 {
                buffer.write_char(':').map_err(|_| EvalError::TextLimit)?;
            }
            match bound {
                Some(value) => write!(buffer, "{value:>width$}", width = self.width),
                None => write!(buffer, "{:>width$}", "", width = self.width),
            }
            .map_err(|_| EvalError::TextLimit)?;
        }

        output.append(buffer.text()?)
    }
}

fn digits(value: i64) -> usize {
    let magnitude = value.unsigned_abs();
    let digits = magnitude.checked_ilog10().unwrap_or(0) as usize + 1;
    digits + usize::from(value < 0)
}

/// Validate numeric operands and retain a stack-only recipe. Any explicit NULL
/// returns NULL before conversion/domain validation; operands are already evaluated.
pub fn prepare(arguments: &[Value]) -> Result<TextResult<'_>, EvalError> {
    prepare_with(arguments, |value| match Conversion::Long.apply(value)? {
        Value::Int32(value) => Ok(value),
        _ => Err(EvalError::TypeMismatch),
    })
}

pub(crate) fn prepare_with(
    arguments: &[Value],
    mut convert: impl FnMut(&Value) -> Result<i32, EvalError>,
) -> Result<TextResult<'_>, EvalError> {
    if arguments.len() != Partition::ARGUMENTS {
        return Err(EvalError::UnsupportedExpression);
    }
    prepare_inputs(
        arguments.iter().any(|value| matches!(value, Value::Null)),
        |index| convert(&arguments[index]),
    )
}

pub(crate) fn prepare_inputs(
    null_operand: bool,
    mut convert: impl FnMut(usize) -> Result<i32, EvalError>,
) -> Result<TextResult<'static>, EvalError> {
    if null_operand {
        return Ok(TextResult::Null);
    }

    let mut values = [0_i64; Partition::ARGUMENTS];
    for (index, converted) in values.iter_mut().enumerate() {
        *converted = i64::from(convert(index)?);
    }
    let [number, start, stop, interval] = values;
    if start < 0 || stop <= start || interval < 1 {
        return Err(EvalError::Domain);
    }

    // Widened endpoints make stop+1 and start-1 safe at either Long boundary.
    let width = digits(stop + 1).max(digits(start - 1));
    let (lower, upper) = if interval == 1 {
        (Some(number), Some(number))
    } else if number < start {
        (None, Some(start - 1))
    } else if number > stop {
        (Some(stop + 1), None)
    } else {
        let lower = start + ((number - start) / interval) * interval;
        (Some(lower), Some((lower + interval - 1).min(stop)))
    };

    Ok(TextResult::Partition(Partition {
        lower,
        upper,
        width,
    }))
}

#[cfg(test)]
mod tests;
