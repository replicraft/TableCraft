//! Independent whole-entry overflow recurrence, never per-component hashing.
use crate::{EditError, format::reader::Reader};
use std::io::Cursor;

pub(super) const MAX_KEY_BYTES: usize = 510;
const DIGEST_BYTES: usize = size_of::<u16>();
const PREFIX_BYTES: usize = MAX_KEY_BYTES - DIGEST_BYTES;
const HIGH_BIT: u16 = 1 << (u16::BITS - 1);
const POLYNOMIAL: u16 = 0x8005;

pub(super) fn truncate(
    bytes: &mut Vec<u8>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    if bytes.len() <= MAX_KEY_BYTES {
        return Ok(());
    }
    let mut register = 0_u16;
    for byte in &bytes[PREFIX_BYTES..bytes.len() - 1] {
        reader.work(u8::BITS as usize + 1)?;
        for _ in 0..u8::BITS {
            let high = register & HIGH_BIT != 0;
            register <<= 1;
            if high {
                register ^= POLYNOMIAL;
            }
        }
        register ^= u16::from(*byte) << u8::BITS;
    }
    register ^= u16::from(*bytes.last().ok_or(EditError::Preservation)?);
    bytes.truncate(MAX_KEY_BYTES);
    bytes[PREFIX_BYTES..].copy_from_slice(&register.to_le_bytes());
    Ok(())
}
