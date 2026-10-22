use super::super::output::TextOutput;
use crate::EvalError;
use std::{fmt, ops::Range};

const UTF16_UNIT_BYTES: usize = size_of::<u16>();

/// Immutable string bytes with consuming, allocation-free byte slicing.
/// Hosts charge storage before adopting the buffer. This is an expression
/// intermediate, not a stored Binary value or a promise of valid Unicode.
pub struct ByteString {
    bytes: Vec<u8>,
    range: Range<usize>,
}

impl ByteString {
    /// Adopt an already charged buffer. Native string lengths fit signed Long.
    pub fn from_owned(bytes: Vec<u8>) -> Result<Self, EvalError> {
        i32::try_from(bytes.len()).map_err(|_| EvalError::Overflow)?;
        let range = 0..bytes.len();
        Ok(Self { bytes, range })
    }

    /// Exact active bytes, without terminators, decoding or padding.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[self.range.clone()]
    }

    /// Byte count, representable as an Access Long.
    pub fn len(&self) -> i32 {
        (self.range.end - self.range.start) as i32
    }

    /// Whether the active string contains no bytes.
    pub fn is_empty(&self) -> bool {
        self.range.is_empty()
    }

    /// Retain at most `length` leading bytes without rounding byte boundaries.
    pub fn left(mut self, length: i32) -> Result<Self, EvalError> {
        let length = nonnegative(length)?.min(self.as_bytes().len());
        self.range.end = self.range.start + length;
        Ok(self)
    }

    /// Retain at most `length` trailing bytes without rounding byte boundaries.
    pub fn right(mut self, length: i32) -> Result<Self, EvalError> {
        let length = nonnegative(length)?.min(self.as_bytes().len());
        self.range.start = self.range.end - length;
        Ok(self)
    }

    /// Select bytes from a one-based start, with an optional nonnegative length.
    pub fn mid(mut self, start: i32, length: Option<i32>) -> Result<Self, EvalError> {
        if start < 1 {
            return Err(EvalError::Domain);
        }
        let length = length.map(nonnegative).transpose()?;
        let offset = ((start - 1) as usize).min(self.as_bytes().len());
        self.range.start += offset;
        if let Some(length) = length {
            self.range.end = self.range.start + length.min(self.as_bytes().len());
        }
        Ok(self)
    }

    /// Strictly publish valid UTF-16LE through a metered UTF-8 sink. Odd byte
    /// counts and unpaired surrogates fail; discard partial output on any error.
    pub fn write_utf8(&self, output: &mut impl TextOutput) -> Result<(), EvalError> {
        output.step()?;
        let (units, remainder) = self.as_bytes().as_chunks::<UTF16_UNIT_BYTES>();
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
