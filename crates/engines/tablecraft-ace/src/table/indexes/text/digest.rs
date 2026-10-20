//! Native entry-level overflow framing, separate from character-weight encoding.
use super::{KeyWork, MAX_KEY_BYTES};
use crate::{InspectError, header::check};

const DIGEST_BYTES: usize = size_of::<u16>();
const POLYNOMIAL: u16 = 0x8005;
const HIGH_BIT: u16 = 1 << (u16::BITS - 1);
const BYTE_SHIFT: u32 = u8::BITS;

pub(in crate::table::indexes) fn truncate(
    bytes: &mut Vec<u8>,
    work: &mut KeyWork<'_>,
) -> Result<(), InspectError> {
    if bytes.len() <= MAX_KEY_BYTES {
        return Ok(());
    }
    let kept = MAX_KEY_BYTES - DIGEST_BYTES;
    let mut register = 0_u16;
    // Advance the polynomial register before feeding each discarded byte.
    // The final byte enters its low lane without another advance. Serialization
    // is low lane first. This is intentionally not an ordinary CRC API call.
    for byte in &bytes[kept..bytes.len() - 1] {
        check(work.budget, work.cancelled)?;
        for _ in 0..u8::BITS {
            let high = register & HIGH_BIT != 0;
            register <<= 1;
            if high {
                register ^= POLYNOMIAL;
            }
        }
        register ^= u16::from(*byte) << BYTE_SHIFT;
    }
    register ^= u16::from(*bytes.last().ok_or(InspectError::Malformed)?);
    bytes.truncate(MAX_KEY_BYTES);
    bytes[kept..].copy_from_slice(&register.to_le_bytes());
    Ok(())
}
