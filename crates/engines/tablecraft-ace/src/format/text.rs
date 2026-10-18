use crate::InspectError;
use crate::profile::{MAX_ENCODED_TEXT_BYTES, MAX_LONG_TEXT_BYTES};
use tablecraft_core::limits::ResourceBudget;

pub(crate) const UTF16_UNIT_BYTES: usize = size_of::<u16>();
const COMPRESSION_MARKER: [u8; 2] = [0xff, 0xfe];
// A single UTF-16 unit needs at most three UTF-8 bytes (a pair needs four).
const MAX_UTF8_BYTES_PER_UNIT: usize = 3;

// Native UTF-16LE, with the Jet/ACE compressed-text marker and zero mode switches.
// Invalid UTF-16 is rejected, never replaced by lossy Unicode or fabricated names.
pub(crate) fn decode(
    bytes: &[u8],
    compressed: bool,
    budget: &mut ResourceBudget,
) -> Result<String, InspectError> {
    decode_bounded(bytes, compressed, MAX_ENCODED_TEXT_BYTES, budget)
}
pub(crate) fn decode_long(
    bytes: &[u8],
    budget: &mut ResourceBudget,
) -> Result<String, InspectError> {
    decode_bounded(bytes, true, MAX_LONG_TEXT_BYTES, budget)
}
fn decode_bounded(
    bytes: &[u8],
    compressed: bool,
    limit: usize,
    budget: &mut ResourceBudget,
) -> Result<String, InspectError> {
    if bytes.len() > limit {
        return Err(InspectError::StructureLimit);
    }
    budget.charge_allocation(bytes.len() as u64, UTF16_UNIT_BYTES as u64)?;
    let mut units = Vec::new();
    units
        .try_reserve_exact(bytes.len())
        .map_err(|_| InspectError::Allocation)?;
    if compressed && bytes.starts_with(&COMPRESSION_MARKER) {
        let mut pos = COMPRESSION_MARKER.len();
        let mut narrow = true;
        while pos < bytes.len() {
            if bytes[pos] == 0 {
                narrow = !narrow;
                pos += 1;
            } else if narrow {
                units.push(u16::from(bytes[pos]));
                pos += 1;
            } else {
                let pair = bytes
                    .get(pos..pos + UTF16_UNIT_BYTES)
                    .ok_or(InspectError::Truncated)?;
                if pair[1] == 0 {
                    return Err(InspectError::Malformed);
                }
                units.push(u16::from_le_bytes([pair[0], pair[1]]));
                pos += UTF16_UNIT_BYTES;
            }
        }
    } else {
        if !bytes.len().is_multiple_of(UTF16_UNIT_BYTES) {
            return Err(InspectError::Malformed);
        }
        for pair in bytes.as_chunks::<UTF16_UNIT_BYTES>().0 {
            units.push(u16::from_le_bytes([pair[0], pair[1]]));
        }
    }
    budget.charge_allocation(units.len() as u64, MAX_UTF8_BYTES_PER_UNIT as u64)?;
    let mut text = String::new();
    text.try_reserve_exact(units.len() * MAX_UTF8_BYTES_PER_UNIT)
        .map_err(|_| InspectError::Allocation)?;
    for decoded in char::decode_utf16(units) {
        text.push(decoded.map_err(|_| InspectError::Malformed)?);
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_text_is_strict_and_compressed_modes_preserve_unicode() {
        let mut budget = ResourceBudget::new(65_536, 0, 0);
        let utf16: Vec<u8> = "A中🦀".encode_utf16().flat_map(u16::to_le_bytes).collect();
        assert_eq!(decode(&utf16, false, &mut budget).unwrap(), "A中🦀");
        assert_eq!(
            decode(
                &[0xff, 0xfe, b'A', 0, 0x2d, 0x4e, 0, b'B'],
                true,
                &mut budget
            )
            .unwrap(),
            "A中B"
        );
        for invalid in [
            &[0][..],
            &[0, 0xd8],
            &[0xff, 0xfe, 0, 1],
            &[0xff, 0xfe, 0, 1, 0],
        ] {
            assert!(decode(invalid, true, &mut budget).is_err());
        }
    }
}
