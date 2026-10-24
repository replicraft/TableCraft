//! Borrowed complete-unit ordering, without Unicode decoding or byte realignment.
use super::ByteOperand;
use crate::{EvalError, functions::text::compare::binary_units};
use std::{cmp::Ordering, slice::Iter, str::EncodeUtf16};
use tablecraft_core::values::Value;

const UNIT_BYTES: usize = size_of::<u16>();

enum Units<'a> {
    Text(EncodeUtf16<'a>),
    Raw(Iter<'a, [u8; UNIT_BYTES]>),
}

impl<'a> Units<'a> {
    fn new(value: &'a ByteOperand) -> Result<Self, EvalError> {
        match value {
            ByteOperand::Scalar(Value::Text(text)) => Ok(Self::Text(text.encode_utf16())),
            ByteOperand::Bytes(bytes) => {
                let bytes = bytes.as_bytes();
                let (units, remainder) = bytes.as_chunks::<UNIT_BYTES>();
                if !remainder.is_empty() {
                    return Err(EvalError::UnrepresentableText);
                }
                Ok(Self::Raw(units.iter()))
            }
            _ => Err(EvalError::TypeMismatch),
        }
    }
}

impl Iterator for Units<'_> {
    type Item = u16;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Text(text) => text.next(),
            Self::Raw(bytes) => bytes.next().map(|bytes| u16::from_le_bytes(*bytes)),
        }
    }
}

/// Compare non-NULL String operands in unsigned UTF-16 code-unit order.
/// Raw ranges must contain complete units; unpaired surrogates are retained.
/// No allocation, padding or Unicode decoding occurs. The caller owns input
/// admission and budgets; step runs before every compared pair and termination.
pub fn compare_binary_utf16(
    left: &ByteOperand,
    right: &ByteOperand,
    step: &mut impl FnMut() -> Result<(), EvalError>,
) -> Result<Ordering, EvalError> {
    binary_units(Units::new(left)?, Units::new(right)?, step)
}

#[cfg(test)]
mod tests;
