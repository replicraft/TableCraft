//! Owned lossless expression String bytes, separate from stored Unicode Text.
use std::{fmt, ops::Range};
use thiserror::Error;

/// Invalid lossless String selection, without payload diagnostics.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("invalid expression string byte range")]
pub struct ExpressionStringRangeError;

/// Immutable expression String bytes with an owned backing buffer and active range.
/// Odd lengths and unpaired UTF-16 units are retained without decoding. Hosts own
/// allocation budgets before adoption. This type is not a stored Value, native
/// encoding descriptor or serialization format, and does not infer SQL equality.
pub struct ExpressionString {
    bytes: Vec<u8>,
    range: Range<usize>,
}

impl Clone for ExpressionString {
    fn clone(&self) -> Self {
        Self::from_owned(self.as_bytes().to_vec())
    }
}

impl PartialEq for ExpressionString {
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl Eq for ExpressionString {}

impl ExpressionString {
    /// Adopt an already charged allocation without copying or decoding it.
    pub fn from_owned(bytes: Vec<u8>) -> Self {
        let range = 0..bytes.len();

        Self { bytes, range }
    }

    /// Borrow exactly the active bytes, without terminators, padding or realignment.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[self.range.clone()]
    }

    /// Active byte length. Semantic byte-count limits belong to consumers.
    pub fn len(&self) -> usize {
        self.range.end - self.range.start
    }

    /// Whether the active range is empty.
    pub fn is_empty(&self) -> bool {
        self.range.is_empty()
    }

    /// Retain an exact range relative to the current active bytes, without copying.
    /// Function-specific one-based positions and clamping belong to the consumer.
    pub fn select(mut self, range: Range<usize>) -> Result<Self, ExpressionStringRangeError> {
        if range.start > range.end || range.end > self.len() {
            return Err(ExpressionStringRangeError);
        }
        self.range = (self.range.start + range.start)..(self.range.start + range.end);

        Ok(self)
    }
}

impl fmt::Debug for ExpressionString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExpressionString")
            .field("len", &self.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
