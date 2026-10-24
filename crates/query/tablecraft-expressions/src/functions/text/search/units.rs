//! Borrowed complete UTF-16 units for binary substring search.
use crate::EvalError;
#[cfg(feature = "sql")]
use crate::comparison::StringInput;
#[cfg(feature = "sql")]
use std::{slice::Iter, str::EncodeUtf16};

#[cfg(feature = "sql")]
const UNIT_BYTES: usize = size_of::<u16>();

#[cfg(feature = "sql")]
#[derive(Clone)]
pub(super) enum Units<'a> {
    Text(EncodeUtf16<'a>),
    Raw(Iter<'a, [u8; UNIT_BYTES]>),
}

#[cfg(feature = "sql")]
impl<'a> Units<'a> {
    pub(super) fn new(input: StringInput<'a>) -> Result<Self, EvalError> {
        match input {
            StringInput::Text(text) => Ok(Self::Text(text.encode_utf16())),
            StringInput::Utf16Le(bytes) => {
                i32::try_from(bytes.len()).map_err(|_| EvalError::Overflow)?;
                let (units, remainder) = bytes.as_chunks::<UNIT_BYTES>();
                if !remainder.is_empty() {
                    return Err(EvalError::UnrepresentableText);
                }
                Ok(Self::Raw(units.iter()))
            }
        }
    }
}

#[cfg(feature = "sql")]
impl Iterator for Units<'_> {
    type Item = u16;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Text(text) => text.next(),
            Self::Raw(units) => units.next().map(|bytes| u16::from_le_bytes(*bytes)),
        }
    }
}

pub(super) fn length(
    units: impl Iterator<Item = u16>,
    step: &mut impl FnMut() -> Result<(), EvalError>,
) -> Result<usize, EvalError> {
    let mut count = 0usize;
    for _ in units {
        step()?;
        count = count.checked_add(1).ok_or(EvalError::Overflow)?;
    }
    i32::try_from(count).map_err(|_| EvalError::Overflow)?;
    Ok(count)
}
