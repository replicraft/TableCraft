mod ascii;
use crate::InspectError;
use crate::limits::{MAX_ENCODED_TEXT_BYTES, MAX_LONG_VALUE_BYTES};
use tablecraft_core::limits::ResourceBudget;

pub(crate) const UTF16_UNIT_BYTES: usize = size_of::<u16>();
const COMPRESSION_MARKER: [u8; 2] = [0xff, 0xfe];
// A single UTF-16 unit needs at most three UTF-8 bytes (a pair needs four).
const MAX_UTF8_BYTES_PER_UNIT: usize = 3;
// A checkpoint covers at most this many bytes, UTF-16 units or decoded scalars
// in one loop. A scalar can consume two UTF-16 units (four encoded bytes).
const CANCELLATION_INTERVAL: usize = 4096;

struct Checkpoints<'a> {
    cancelled: Option<&'a dyn Fn() -> bool>,
    remaining: usize,
}

impl Checkpoints<'_> {
    fn check(&self) -> Result<(), InspectError> {
        if self.cancelled.is_some_and(|cancelled| cancelled()) {
            return Err(InspectError::Cancelled);
        }
        Ok(())
    }

    fn advance(&mut self) -> Result<(), InspectError> {
        self.remaining -= 1;
        if self.remaining == 0 {
            self.check()?;
            self.remaining = CANCELLATION_INTERVAL;
        }
        Ok(())
    }

    fn ascii(&self, bytes: &[u8], narrow: bool) -> Result<bool, InspectError> {
        if self.cancelled.is_none() {
            return Ok(if narrow {
                ascii::compressed(bytes)
            } else {
                ascii::utf16le(bytes)
            });
        }

        // The even chunk width keeps UTF-16LE pairs intact. Classifying each
        // chunk has the same predicate as classifying the complete input.
        for chunk in bytes.chunks(CANCELLATION_INTERVAL) {
            self.check()?;
            if !(if narrow {
                ascii::compressed(chunk)
            } else {
                ascii::utf16le(chunk)
            }) {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

// Native UTF-16LE, with the Jet/ACE compressed-text marker and zero mode switches.
// Invalid UTF-16 is rejected, never replaced by lossy Unicode or fabricated names.
pub(crate) fn decode(
    bytes: &[u8],
    compressed: bool,
    budget: &mut ResourceBudget,
) -> Result<String, InspectError> {
    decode_bounded(
        bytes,
        compressed,
        MAX_ENCODED_TEXT_BYTES,
        None,
        budget,
        None,
    )
}
pub(crate) fn decode_long(
    bytes: &[u8],
    budget: &mut ResourceBudget,
) -> Result<String, InspectError> {
    decode_bounded(bytes, true, MAX_LONG_VALUE_BYTES, None, budget, None)
}
pub(crate) fn decode_long_with_limits(
    bytes: &[u8],
    maximum_bytes: usize,
    maximum_decoded_bytes: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<String, InspectError> {
    decode_bounded(
        bytes,
        true,
        maximum_bytes,
        Some(maximum_decoded_bytes),
        budget,
        Some(cancelled),
    )
}
fn decode_bounded(
    bytes: &[u8],
    compressed: bool,
    limit: usize,
    decoded_limit: Option<usize>,
    budget: &mut ResourceBudget,
    cancelled: Option<&dyn Fn() -> bool>,
) -> Result<String, InspectError> {
    if bytes.len() > limit {
        return Err(InspectError::StructureLimit);
    }
    let mut checkpoints = Checkpoints {
        cancelled,
        remaining: CANCELLATION_INTERVAL,
    };
    checkpoints.check()?;
    let narrow = compressed && bytes.starts_with(&COMPRESSION_MARKER);
    let raw = if narrow {
        &bytes[COMPRESSION_MARKER.len()..]
    } else {
        bytes
    };
    if checkpoints.ascii(raw, narrow)? {
        let length = if narrow {
            raw.len()
        } else {
            raw.len() / UTF16_UNIT_BYTES
        };
        if decoded_limit.is_some_and(|limit| length > limit) {
            return Err(InspectError::StructureLimit);
        }
        budget.charge_allocation(length as u64, 1)?;
        let mut text = String::new();
        text.try_reserve_exact(length)
            .map_err(|_| InspectError::Allocation)?;
        if narrow {
            for chunk in raw.chunks(CANCELLATION_INTERVAL) {
                checkpoints.check()?;
                text.push_str(std::str::from_utf8(chunk).map_err(|_| InspectError::Malformed)?);
            }
        } else {
            for pair in raw.as_chunks::<UTF16_UNIT_BYTES>().0 {
                text.push(char::from(pair[0]));
                checkpoints.advance()?;
            }
        }
        checkpoints.check()?;
        return Ok(text);
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
            checkpoints.advance()?;
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
                // A zero switches modes only at the next character boundary.
                // The high byte belongs to this UTF-16 unit, including zero.
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
            checkpoints.advance()?;
        }
    }
    let capacity = if let Some(limit) = decoded_limit {
        let mut length = 0_usize;
        for decoded in char::decode_utf16(units.iter().copied()) {
            checkpoints.advance()?;
            let character = decoded.map_err(|_| InspectError::Malformed)?;
            length = length
                .checked_add(character.len_utf8())
                .ok_or(InspectError::StructureLimit)?;
            if length > limit {
                return Err(InspectError::StructureLimit);
            }
        }
        length
    } else {
        units.len() * MAX_UTF8_BYTES_PER_UNIT
    };
    budget.charge_allocation(capacity as u64, 1)?;
    let mut text = String::new();
    text.try_reserve_exact(capacity)
        .map_err(|_| InspectError::Allocation)?;
    for decoded in char::decode_utf16(units) {
        text.push(decoded.map_err(|_| InspectError::Malformed)?);
        checkpoints.advance()?;
    }
    checkpoints.check()?;
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
        for invalid in [&[0][..], &[0, 0xd8], &[0xff, 0xfe, 0, 1]] {
            assert!(decode(invalid, true, &mut budget).is_err());
        }
    }

    #[test]
    fn ascii_fast_paths_charge_only_retained_text_and_preserve_nul() {
        for (bytes, compressed) in [
            (vec![0xff, 0xfe, b'A', b'B'], true),
            (vec![b'A', 0, b'B', 0], false),
        ] {
            assert_eq!(
                decode(&bytes, compressed, &mut ResourceBudget::new(2, 0, 0)).unwrap(),
                "AB"
            );
            assert!(decode(&bytes, compressed, &mut ResourceBudget::new(1, 0, 0)).is_err());
        }
        assert_eq!(
            decode(
                &[b'A', 0, 0, 0, b'B', 0],
                false,
                &mut ResourceBudget::new(3, 0, 0)
            )
            .unwrap(),
            "A\0B"
        );
    }
}

#[cfg(test)]
#[path = "text/long_tests.rs"]
mod long_tests;

#[cfg(test)]
#[path = "text/wide_tests.rs"]
mod wide_tests;
