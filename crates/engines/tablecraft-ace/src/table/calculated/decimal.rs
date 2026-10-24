//! Observed cached Numeric payloads carry their own scale and signed magnitude.
use crate::{InspectError, format::bytes::u16_at};
use tablecraft_core::values::Decimal;
use tablecraft_schema::DecimalConstraints;

const PAYLOAD_BYTES: usize = 16;
const REMAINING_BYTES: u16 = (PAYLOAD_BYTES - size_of::<u16>()) as u16;
const SCALE_OFFSET: usize = size_of::<u16>();
const SIGN_OFFSET: usize = SCALE_OFFSET + size_of::<u8>();
const MAGNITUDE_OFFSET: usize = SIGN_OFFSET + size_of::<u8>();
const HIGH_WORD_BYTES: usize = size_of::<u32>();
const LOW_WORD_OFFSET: usize = MAGNITUDE_OFFSET + HIGH_WORD_BYTES;
const NEGATIVE: u8 = 0x80;
const COEFFICIENT_LIMIT: u128 = 10_u128.pow(DecimalConstraints::MAX_PRECISION as u32);

pub(super) fn decode(bytes: &[u8]) -> Result<Decimal, InspectError> {
    if bytes.len() != PAYLOAD_BYTES {
        return Err(InspectError::Malformed);
    }
    if u16_at(bytes, 0)? != REMAINING_BYTES {
        return Err(InspectError::Unsupported);
    }
    let scale = bytes[SCALE_OFFSET];
    let negative = match bytes[SIGN_OFFSET] {
        0 => false,
        NEGATIVE => true,
        _ => return Err(InspectError::Unsupported),
    };
    let high = u32::from_le_bytes(
        bytes[MAGNITUDE_OFFSET..LOW_WORD_OFFSET]
            .try_into()
            .map_err(|_| InspectError::Malformed)?,
    );
    let low = u64::from_le_bytes(
        bytes[LOW_WORD_OFFSET..]
            .try_into()
            .map_err(|_| InspectError::Malformed)?,
    );
    let magnitude = (u128::from(high) << u64::BITS) | u128::from(low);
    if magnitude >= COEFFICIENT_LIMIT || scale > Decimal::MAX_SCALE {
        return Err(InspectError::Malformed);
    }
    // Canonical independent writes use a positive sign for zero. Do not erase
    // an unqualified negative-zero state through the shared Decimal constructor.
    if magnitude == 0 && negative {
        return Err(InspectError::Unsupported);
    }
    let coefficient = if negative {
        -(magnitude as i128)
    } else {
        magnitude as i128
    };
    Decimal::new(coefficient, scale).map_err(|_| InspectError::Malformed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_payloads_retain_effective_scale_sign_and_word_order() {
        let negative = [14, 0, 4, 128, 0, 0, 0, 0, 57, 48, 0, 0, 0, 0, 0, 0];
        assert_eq!(decode(&negative), Ok(Decimal::new(-12345, 4).unwrap()));
        let maximum = [
            14, 0, 0, 0, 94, 206, 79, 32, 255, 255, 255, 15, 97, 2, 37, 62,
        ];
        assert_eq!(
            decode(&maximum),
            Ok(Decimal::new(9_999_999_999_999_999_999_999_999_999, 0).unwrap())
        );
        let tiny = [14, 0, 28, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(decode(&tiny), Ok(Decimal::new(1, 28).unwrap()));
        let zero = [14, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(decode(&zero), Ok(Decimal::new(0, 4).unwrap()));
    }

    #[test]
    fn unobserved_headers_signs_scales_widths_and_coefficients_fail() {
        let golden = [14, 0, 4, 128, 0, 0, 0, 0, 57, 48, 0, 0, 0, 0, 0, 0];
        for end in 0..golden.len() {
            assert!(decode(&golden[..end]).is_err());
        }
        let mut extra = golden.to_vec();
        extra.push(0);
        assert!(decode(&extra).is_err());
        for (offset, value) in [(0, 0), (0, 13), (1, 1), (2, 29), (3, 1), (3, 255)] {
            let mut changed = golden;
            changed[offset] = value;
            assert!(decode(&changed).is_err());
        }
        let mut negative_zero = [0; 16];
        negative_zero[..4].copy_from_slice(&[14, 0, 4, 128]);
        assert_eq!(decode(&negative_zero), Err(InspectError::Unsupported));
        let oversized = [
            14, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
        ];
        assert_eq!(decode(&oversized), Err(InspectError::Malformed));
    }
}
