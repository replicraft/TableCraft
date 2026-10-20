//! Borrowed Replace arguments and bounded streaming construction.
use super::compare;
use crate::{
    EvalError,
    comparison::{Collation, StringCompareMode, StringSearch},
    numeric::{IntegerType, coerce_integer},
};
use std::ops::Range;
use tablecraft_core::values::Value;

const FIRST_POSITION: i32 = 1;
const ALL_MATCHES: i32 = -1;

/// Host-owned work and output accounting. Discard partial output on any error;
/// append must enforce byte ceilings and charge before fallible allocation.
pub trait TextOutput {
    /// Charge work and check cancellation before scanning or provider invocation.
    fn step(&mut self) -> Result<(), EvalError>;
    /// Append a borrowed fragment under the host's cumulative output budget.
    fn append(&mut self, fragment: &str) -> Result<(), EvalError>;
}
/// Validated borrowed Replace operands. Construction allocates nothing; writing
/// consumes the caller's work/output budget without storing a match list.
#[derive(Debug, PartialEq, Eq)]
pub struct Replacement<'a> {
    source: &'a str,
    find: &'a str,
    replacement: &'a str,
    start: usize,
    count: i32,
    mode: Option<StringCompareMode>,
}
pub(super) fn prepare(arguments: &[Value]) -> Result<Replacement<'_>, EvalError> {
    let start = long(arguments.get(3), FIRST_POSITION)?;
    let count = long(arguments.get(4), ALL_MATCHES)?;
    let mode = compare::mode(arguments.get(5), Some(StringCompareMode::Binary))?;
    if start < FIRST_POSITION || count < ALL_MATCHES {
        return Err(EvalError::Domain);
    }
    Ok(Replacement {
        source: text(&arguments[0])?,
        find: text(&arguments[1])?,
        replacement: text(&arguments[2])?,
        start: (start - FIRST_POSITION) as usize,
        count,
        mode,
    })
}
impl Replacement<'_> {
    /// Emit the suffix starting at the requested UTF-16 position, replacing
    /// non-overlapping source matches. Inserted text is never searched again.
    /// Host text/database modes use Collation::find and validate returned spans.
    /// Zero-width provider matches and boundaries splitting surrogate pairs fail.
    pub fn write(
        &self,
        collation: Option<&dyn Collation>,
        output: &mut impl TextOutput,
    ) -> Result<(), EvalError> {
        let length = units(self.source, output)?;
        units(self.find, output)?;
        units(self.replacement, output)?;
        if self.start >= length {
            return Ok(());
        }
        let byte = boundary(self.source, self.start, output)?;
        let mut remaining = &self.source[byte..];
        let mut remaining_units = length - self.start;
        if self.find.is_empty() || self.count == 0 {
            return output.append(remaining);
        }
        let mode = self
            .mode
            .or_else(|| collation.and_then(Collation::option_compare))
            .ok_or(EvalError::CollationRequired)?;
        let mut count = self.count;
        while !remaining.is_empty() && count != 0 {
            output.step()?;
            let found = if mode == StringCompareMode::Binary {
                binary(remaining, self.find, output)?
            } else {
                let found = collation.ok_or(EvalError::CollationRequired)?.find(
                    remaining,
                    self.find,
                    StringSearch::ForwardFrom(0),
                    mode,
                )?;
                match found {
                    None => None,
                    Some(found) => {
                        // A nonempty consumed span guarantees progress even for
                        // unlimited replacements and adversarial provider callbacks.
                        if found.start >= found.end || found.end > remaining_units {
                            return Err(EvalError::InvalidTextMatch);
                        }
                        Some(
                            boundary(remaining, found.start, output)?
                                ..boundary(remaining, found.end, output)?,
                        )
                    }
                }
            };
            let Some(found) = found else { break };
            output.append(&remaining[..found.start])?;
            output.append(self.replacement)?;
            // Count consumed UTF-16 units once while advancing through the source.
            remaining_units -= units(&remaining[..found.end], output)?;
            remaining = &remaining[found.end..];
            if count > 0 {
                count -= 1;
            }
        }
        output.append(remaining)
    }
}
fn long(value: Option<&Value>, default: i32) -> Result<i32, EvalError> {
    match value {
        None => Ok(default),
        Some(Value::Null) => Err(EvalError::InvalidNullConversion),
        Some(value) => match coerce_integer(value, IntegerType::Int32)? {
            Value::Int32(value) => Ok(value),
            _ => Err(EvalError::TypeMismatch),
        },
    }
}
fn text(value: &Value) -> Result<&str, EvalError> {
    match value {
        Value::Text(value) => Ok(value),
        Value::Null => Err(EvalError::InvalidNullConversion),
        _ => Err(EvalError::TypeMismatch),
    }
}
fn units(text: &str, output: &mut impl TextOutput) -> Result<usize, EvalError> {
    let mut count = 0usize;
    for _ in text.encode_utf16() {
        output.step()?;
        count = count.checked_add(1).ok_or(EvalError::Overflow)?;
    }
    i32::try_from(count).map_err(|_| EvalError::Overflow)?;
    Ok(count)
}
fn boundary(text: &str, position: usize, output: &mut impl TextOutput) -> Result<usize, EvalError> {
    let mut units = 0;
    for (byte, character) in text.char_indices() {
        output.step()?;
        if units == position {
            return Ok(byte);
        }
        units += character.len_utf16();
        if units > position {
            return Err(EvalError::UnrepresentableText);
        }
    }
    if units == position {
        Ok(text.len())
    } else {
        Err(EvalError::InvalidTextMatch)
    }
}
fn binary(
    haystack: &str,
    needle: &str,
    output: &mut impl TextOutput,
) -> Result<Option<Range<usize>>, EvalError> {
    for (byte, _) in haystack.char_indices() {
        output.step()?;
        let mut candidate = haystack[byte..].encode_utf16();
        let mut matched = true;
        for unit in needle.encode_utf16() {
            output.step()?;
            if candidate.next() != Some(unit) {
                matched = false;
                break;
            }
        }
        if matched {
            return Ok(Some(byte..byte + needle.len()));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests;
