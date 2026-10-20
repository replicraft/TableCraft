//! Text-only Access built-ins with explicit UTF-16 positions and metered scans.
use crate::{EvalError, comparison::Collation};
mod compare;
mod search;
#[cfg(feature = "sql")]
pub mod sql_arguments;
use tablecraft_core::values::Value;

/// Closed text operations; locale conversion and byte-oriented variants are absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextFunction {
    /// UTF-16 code-unit length, returned as Int32.
    Len,
    /// Remove leading U+0020 spaces.
    LTrim,
    /// Remove trailing U+0020 spaces.
    RTrim,
    /// Remove leading and trailing U+0020 spaces.
    Trim,
    /// Select the first length UTF-16 units.
    Left,
    /// Select the last length UTF-16 units.
    Right,
    /// Select from a one-based UTF-16 position, with optional length.
    Mid,
    /// Compare two strings, returning Int16 -1/0/1 or NULL.
    StrComp,
    /// First substring position as a one-based Int32, or zero/NULL.
    InStr,
    /// Last substring position before an end boundary, as Int32, or zero/NULL.
    InStrRev,
}
impl TextFunction {
    /// Largest accepted argument count; used for fixed-size evaluation storage.
    pub const MAX_ARGUMENTS: usize = 4;
    /// Resolve only ASCII-insensitive unqualified names.
    pub fn resolve(name: &str) -> Option<Self> {
        [
            ("Len", Self::Len),
            ("LTrim", Self::LTrim),
            ("RTrim", Self::RTrim),
            ("Trim", Self::Trim),
            ("Left", Self::Left),
            ("Right", Self::Right),
            ("Mid", Self::Mid),
            ("StrComp", Self::StrComp),
            ("InStr", Self::InStr),
            ("InStrRev", Self::InStrRev),
        ]
        .into_iter()
        .find_map(|(spelling, operation)| name.eq_ignore_ascii_case(spelling).then_some(operation))
    }
    /// Validate syntax shape before evaluating operands.
    pub const fn accepts_arity(self, count: usize) -> bool {
        match self {
            Self::Len | Self::LTrim | Self::RTrim | Self::Trim => count == 1,
            Self::Left | Self::Right => count == 2,
            Self::Mid | Self::StrComp => count == 2 || count == 3,
            Self::InStr | Self::InStrRev => count >= 2 && count <= Self::MAX_ARGUMENTS,
        }
    }
}
/// Borrowed output; hosts must meter and fallibly copy Text before materializing.
#[derive(Debug, PartialEq, Eq)]
pub enum TextResult<'a> {
    /// SQL NULL, distinct from zero-length text.
    Null,
    /// Exact Int32 UTF-16 length.
    Length(i32),
    /// One-based Int32 substring position, or zero when not found.
    Position(i32),
    /// Signed Int16 string ordering result.
    Comparison(i16),
    /// A valid Unicode slice of an input string, without allocation.
    Text(&'a str),
}
/// Apply a text operation over evaluated operands, invoking `step` per scanned
/// Unicode scalar (UTF-16 unit for comparison/search). Positions count UTF-16 units;
/// slicing across a surrogate pair fails rather than producing replacement characters. Only Text/NULL input and exact
/// Byte/Int16/Int32/Int64 positional operands in Long range are supported. StrComp
/// and substring search instead use checked numeric-to-Long conversion for modes
/// and starts, with explicit function-specific defaults.
pub fn apply<'a>(
    function: TextFunction,
    arguments: &'a [Value],
    step: impl FnMut() -> Result<(), EvalError>,
) -> Result<TextResult<'a>, EvalError> {
    apply_with_collation(function, arguments, None, step)
}
/// Apply text functions with an explicit database/default/text comparison host.
/// Existing non-comparison functions ignore this context. Hosts still own callback
/// work limits; string inputs and binary comparisons are scanned through step.
pub fn apply_with_collation<'a>(
    function: TextFunction,
    arguments: &'a [Value],
    collation: Option<&dyn Collation>,
    mut step: impl FnMut() -> Result<(), EvalError>,
) -> Result<TextResult<'a>, EvalError> {
    if !function.accepts_arity(arguments.len()) {
        return Err(EvalError::UnsupportedExpression);
    }
    step()?;
    if function == TextFunction::StrComp {
        return compare::apply(arguments, collation, &mut step);
    }
    if matches!(function, TextFunction::InStr | TextFunction::InStrRev) {
        return search::apply(function, arguments, collation, &mut step);
    }
    let text = match &arguments[0] {
        Value::Null => return Ok(TextResult::Null),
        Value::Text(text) => text.as_str(),
        _ => return Err(EvalError::TypeMismatch),
    };
    match function {
        TextFunction::StrComp | TextFunction::InStr | TextFunction::InStrRev => {
            unreachable!("handled before single-text functions")
        }
        TextFunction::Len => Ok(TextResult::Length(units(text, &mut step)?)),
        TextFunction::LTrim | TextFunction::RTrim | TextFunction::Trim => {
            let mut result = text;
            if function != TextFunction::RTrim {
                for ch in text.chars() {
                    step()?;
                    if ch != ' ' {
                        break;
                    }
                    result = &result[1..];
                }
            }
            if function != TextFunction::LTrim {
                let mut end = result.len();
                for ch in result.chars().rev() {
                    step()?;
                    if ch != ' ' {
                        break;
                    }
                    end -= 1;
                }
                result = &result[..end];
            }
            Ok(TextResult::Text(result))
        }
        TextFunction::Left | TextFunction::Right | TextFunction::Mid => {
            let position = long(&arguments[1])?;
            let length = if function == TextFunction::Mid {
                if position < 1 {
                    return Err(EvalError::Domain);
                }
                arguments.get(2).map(long).transpose()?
            } else {
                Some(position)
            };
            if length.is_some_and(|length| length < 0) {
                return Err(EvalError::Domain);
            }
            let total = units(text, &mut step)?;
            let (start, end) = match function {
                TextFunction::Left => (0, position.min(total)),
                TextFunction::Right => (total.saturating_sub(position).max(0), total),
                _ => {
                    let start = (position - 1).min(total);
                    (
                        start,
                        start.saturating_add(length.unwrap_or(total)).min(total),
                    )
                }
            };
            if start == end {
                return Ok(TextResult::Text(""));
            }
            let start = boundary(text, start, &mut step)?;
            let end = boundary(text, end, &mut step)?;
            Ok(TextResult::Text(&text[start..end]))
        }
    }
}
fn long(value: &Value) -> Result<i32, EvalError> {
    let integer = match value {
        Value::Byte(value) => i64::from(*value),
        Value::Int16(value) => i64::from(*value),
        Value::Int32(value) => i64::from(*value),
        Value::Int64(value) => *value,
        _ => return Err(EvalError::TypeMismatch),
    };
    integer.try_into().map_err(|_| EvalError::Overflow)
}
fn units(text: &str, step: &mut impl FnMut() -> Result<(), EvalError>) -> Result<i32, EvalError> {
    text.chars().try_fold(0_i32, |length, ch| {
        step()?;
        length
            .checked_add(ch.len_utf16() as i32)
            .ok_or(EvalError::Overflow)
    })
}
fn boundary(
    text: &str,
    position: i32,
    step: &mut impl FnMut() -> Result<(), EvalError>,
) -> Result<usize, EvalError> {
    let mut units = 0;
    for (byte, ch) in text.char_indices() {
        step()?;
        if units == position {
            return Ok(byte);
        }
        units += ch.len_utf16() as i32;
        if units > position {
            return Err(EvalError::UnrepresentableText);
        }
    }
    Ok(text.len())
}
#[cfg(test)]
mod tests;
