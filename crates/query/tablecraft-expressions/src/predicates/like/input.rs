//! Borrowed String inputs for explicitly selected UTF-16 matching.
use super::Work;
use crate::EvalError;

const UNIT_BYTES: usize = size_of::<u16>();

/// A non-NULL String supplied to the UTF-16 matcher without Unicode publication.
#[derive(Clone, Copy, Debug)]
pub enum LikeString<'a> {
    /// Already decoded Unicode text.
    Text(&'a str),
    /// Little-endian String bytes. Complete units may contain isolated surrogates;
    /// odd byte counts are refused rather than padded or truncated.
    Utf16Le(&'a [u8]),
}

impl LikeString<'_> {
    pub(super) fn units(self, work: &mut Work<'_>) -> Result<Vec<u16>, EvalError> {
        match self {
            Self::Text(text) => {
                let mut units = work.buffer(text.len())?;
                for unit in text.encode_utf16() {
                    work.tick()?;
                    units.push(unit);
                }
                Ok(units)
            }
            Self::Utf16Le(bytes) => {
                i32::try_from(bytes.len()).map_err(|_| EvalError::Overflow)?;
                let (chunks, remainder) = bytes.as_chunks::<UNIT_BYTES>();
                if !remainder.is_empty() {
                    return Err(EvalError::UnrepresentableText);
                }

                let mut units = work.buffer(chunks.len())?;
                for chunk in chunks {
                    work.tick()?;
                    units.push(u16::from_le_bytes(*chunk));
                }
                Ok(units)
            }
        }
    }
}
