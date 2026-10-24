use super::super::output::TextOutput;
use crate::EvalError;
use std::{fmt, ops::Range};
use tablecraft_core::values::ExpressionString;

const UTF16_UNIT_BYTES: usize = size_of::<u16>();

/// Immutable string bytes with consuming, allocation-free byte slicing.
/// Hosts charge storage before adopting the buffer. This is an expression
/// intermediate, not a stored Binary value or a promise of valid Unicode.
pub struct ByteString {
    value: ExpressionString,
}

impl ByteString {
    /// Adopt an already charged buffer. Native string lengths fit signed Long.
    pub fn from_owned(bytes: Vec<u8>) -> Result<Self, EvalError> {
        i32::try_from(bytes.len()).map_err(|_| EvalError::Overflow)?;
        Ok(Self {
            value: ExpressionString::from_owned(bytes),
        })
    }

    /// Adopt the shared expression representation, retaining the Long byte limit.
    pub fn from_expression_string(value: ExpressionString) -> Result<Self, EvalError> {
        i32::try_from(value.len()).map_err(|_| EvalError::Overflow)?;
        Ok(Self { value })
    }

    /// Move the active range to a shared expression cell without copying.
    pub fn into_expression_string(self) -> ExpressionString {
        self.value
    }

    /// Exact active bytes, without terminators, decoding or padding.
    pub fn as_bytes(&self) -> &[u8] {
        self.value.as_bytes()
    }

    /// Byte count, representable as an Access Long.
    pub fn len(&self) -> i32 {
        self.value.len() as i32
    }

    /// Whether the active string contains no bytes.
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    /// Retain at most `length` leading bytes without rounding byte boundaries.
    pub fn left(self, length: i32) -> Result<Self, EvalError> {
        let length = nonnegative(length)?.min(self.as_bytes().len());
        self.select(0..length)
    }

    /// Retain at most `length` trailing bytes without rounding byte boundaries.
    pub fn right(self, length: i32) -> Result<Self, EvalError> {
        let length = nonnegative(length)?.min(self.as_bytes().len());
        let total = self.as_bytes().len();
        self.select((total - length)..total)
    }

    /// Select bytes from a one-based start, with an optional nonnegative length.
    pub fn mid(self, start: i32, length: Option<i32>) -> Result<Self, EvalError> {
        if start < 1 {
            return Err(EvalError::Domain);
        }
        let length = length.map(nonnegative).transpose()?;
        let offset = ((start - 1) as usize).min(self.as_bytes().len());
        let end = self.as_bytes().len();
        let length = length.unwrap_or(end - offset).min(end - offset);
        self.select(offset..(offset + length))
    }

    pub(super) fn select(self, range: Range<usize>) -> Result<Self, EvalError> {
        self.value
            .select(range)
            .map(|value| Self { value })
            .map_err(|_| EvalError::Domain)
    }

    /// Strictly publish valid UTF-16LE through a metered UTF-8 sink. Odd byte
    /// counts and unpaired surrogates fail; discard partial output on any error.
    pub fn write_utf8(&self, output: &mut impl TextOutput) -> Result<(), EvalError> {
        write_utf8(self.as_bytes(), output)
    }
}

pub(crate) fn write_utf8(bytes: &[u8], output: &mut impl TextOutput) -> Result<(), EvalError> {
    output.step()?;
    let (units, remainder) = bytes.as_chunks::<UTF16_UNIT_BYTES>();
    if !remainder.is_empty() {
        return Err(EvalError::UnrepresentableText);
    }
    let units = units.iter().copied().map(u16::from_le_bytes);
    let mut buffer = [0; char::MAX.len_utf8()];
    for character in char::decode_utf16(units) {
        output.step()?;
        let character = character.map_err(|_| EvalError::UnrepresentableText)?;
        output.append(character.encode_utf8(&mut buffer))?;
    }
    Ok(())
}

fn nonnegative(value: i32) -> Result<usize, EvalError> {
    usize::try_from(value).map_err(|_| EvalError::Domain)
}

impl fmt::Debug for ByteString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ByteString")
            .field("len", &self.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
