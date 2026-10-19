use crate::{InspectError, format::bytes::u32_at};

const LENGTH_OFFSET: usize = 16;
const DATA_OFFSET: usize = LENGTH_OFFSET + size_of::<u32>();
const TRAILER_BYTES: usize = 3;
pub(super) const WRAPPER_BYTES: usize = DATA_OFFSET + TRAILER_BYTES;

// Strict observed wrapper, without interpreting expressions or unknown tags.
pub(super) fn payload(bytes: &[u8]) -> Result<&[u8], InspectError> {
    if bytes.len() < WRAPPER_BYTES {
        return Err(InspectError::Malformed);
    }
    if bytes[..LENGTH_OFFSET].iter().any(|byte| *byte != 0) {
        return Err(InspectError::Unsupported);
    }
    let length = u32_at(bytes, LENGTH_OFFSET)? as usize;
    let end = DATA_OFFSET
        .checked_add(length)
        .ok_or(InspectError::Malformed)?;
    if bytes.len().checked_sub(TRAILER_BYTES) != Some(end) {
        return Err(InspectError::Malformed);
    }
    if bytes[end..].iter().any(|byte| *byte != 0) {
        return Err(InspectError::Unsupported);
    }
    Ok(&bytes[DATA_OFFSET..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_wrapper_preserves_payload_and_rejects_unknown_or_malformed_states() {
        let golden = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0xd8, 0xff, 0xff, 0xff, 0,
            0, 0,
        ];
        assert_eq!(payload(&golden), Ok(&[0xd8, 0xff, 0xff, 0xff][..]));
        assert_eq!(payload(&[0; 23]), Ok(&[][..]));
        for end in 0..golden.len() {
            assert!(payload(&golden[..end]).is_err());
        }
        for at in [0, 15, 16, 24, 26] {
            let mut changed = golden;
            changed[at] ^= 1;
            assert!(payload(&changed).is_err());
        }
        let mut longer = golden.to_vec();
        longer.push(0);
        assert!(payload(&longer).is_err());
    }
}
