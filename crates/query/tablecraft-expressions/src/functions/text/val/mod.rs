//! Bounded locale-independent numeric-prefix parsing for Val.
mod radix;
mod scan;
mod suffix;
use super::{TextResult, output::TextOutput};
use crate::EvalError;
use tablecraft_core::values::{Double, Value};

/// Charged normalization storage. It must start empty, return exactly the appended
/// text and retain it for evaluation. Discard contents on error; append must charge
/// payload bytes before fallible allocation, as with other text output kernels.
pub trait NumericBuffer: TextOutput {
    /// Borrow the normalized prefix accumulated so far.
    fn text(&self) -> &str;
}
/// Borrowed Val input; normalization and conversion run under an explicit buffer.
#[derive(Debug, PartialEq, Eq)]
pub struct NumericPrefix<'a> {
    text: &'a str,
}
pub(super) fn prepare(value: &Value) -> Result<TextResult<'_>, EvalError> {
    match value {
        Value::Text(text) => Ok(TextResult::NumericPrefix(NumericPrefix { text })),
        Value::Null => Err(EvalError::InvalidNullConversion),
        _ => Err(EvalError::TypeMismatch),
    }
}
impl NumericPrefix<'_> {
    /// Parse a numeric prefix into a finite Double, validating type suffixes before
    /// final conversion. This is distinct from locale/implicit Variant coercion.
    pub fn evaluate(&self, buffer: &mut impl NumericBuffer) -> Result<Double, EvalError> {
        if !buffer.text().is_empty() {
            return Err(EvalError::Domain);
        }
        let mut cursor = scan::Cursor::new(self.text);
        let mut first = cursor.next(buffer)?;
        let negative = first == Some('-');
        if matches!(first, Some('+' | '-')) {
            first = cursor.next(buffer)?;
        }
        if first == Some('&') {
            return radix::parse(&mut cursor, negative, buffer);
        }
        if !first.is_some_and(|ch| ch.is_ascii_digit() || ch == '.') {
            return Ok(Double::from_float(0.0));
        }
        if negative {
            buffer.append("-")?;
        }
        let mut digits = 0usize;
        let mut point = false;
        while let Some(ch) = first {
            if ch.is_ascii_digit() {
                digits += 1;
            } else if ch == '.' && !point {
                point = true;
            } else {
                break;
            }
            let mut byte = [0; char::MAX.len_utf8()];
            buffer.append(ch.encode_utf8(&mut byte))?;
            first = cursor.next(buffer)?;
        }
        if digits == 0 {
            return Ok(Double::from_float(0.0));
        }
        if matches!(first, Some('e' | 'E' | 'd' | 'D')) {
            let mut lookahead = cursor.clone();
            let mut next = lookahead.next(buffer)?;
            let sign = if matches!(next, Some('+' | '-')) {
                let sign = next;
                next = lookahead.next(buffer)?;
                sign
            } else {
                None
            };
            if next.is_some_and(|ch| ch.is_ascii_digit()) {
                buffer.append("e")?;
                if let Some(sign) = sign {
                    let mut byte = [0; char::MAX.len_utf8()];
                    buffer.append(sign.encode_utf8(&mut byte))?;
                }
                cursor = lookahead;
                while let Some(ch) = next.filter(|ch| ch.is_ascii_digit()) {
                    let mut byte = [0; char::MAX.len_utf8()];
                    buffer.append(ch.encode_utf8(&mut byte))?;
                    next = cursor.next(buffer)?;
                }
                first = next;
            } else {
                // An incomplete exponent is not part of a recognized numeric prefix.
                first = None;
            }
        }
        // Charge the bounded normalization pass before suffix validation and the
        // standard-library conversion, checking cancellation per prefix byte.
        for _ in 0..buffer.text().len() {
            buffer.step()?;
        }
        suffix::decimal(buffer.text(), first)
    }
}
#[cfg(test)]
mod tests;
