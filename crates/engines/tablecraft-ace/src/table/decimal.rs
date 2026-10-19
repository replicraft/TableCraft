use crate::{InspectError, format::data_type::NUMERIC_BYTES};
use tablecraft_core::values::Decimal;

const SIGN_BYTES: usize = size_of::<u8>();
const WORD_BYTES: usize = size_of::<u32>();
const NEGATIVE_SIGN: u8 = 0x80;

// Native NUMERIC stores most-significant words first, each word little-endian.
// See the pinned format references and independent evidence in NATIVE_TABLES.md.
pub(super) fn decode(bytes: &[u8], precision: u8, scale: u8) -> Result<Decimal, InspectError> {
    if bytes.len() != NUMERIC_BYTES
        || !(1..=Decimal::MAX_PRECISION).contains(&precision)
        || scale > precision
    {
        return Err(InspectError::Malformed);
    }
    let negative = match bytes[0] {
        0 => false,
        NEGATIVE_SIGN => true,
        _ => return Err(InspectError::Unsupported),
    };
    let mut canonical = [0; size_of::<u128>()];
    for (input, output) in bytes[SIGN_BYTES..]
        .as_chunks::<WORD_BYTES>()
        .0
        .iter()
        .zip(canonical.as_chunks_mut::<WORD_BYTES>().0.iter_mut())
    {
        let word = u32::from_le_bytes(*input);
        output.copy_from_slice(&word.to_be_bytes());
    }
    let magnitude = u128::from_be_bytes(canonical);
    if magnitude >= 10_u128.pow(u32::from(precision)) {
        return Err(InspectError::Malformed);
    }
    // The precision bound proves the magnitude fits i128 before applying sign.
    let coefficient = i128::try_from(magnitude).map_err(|_| InspectError::Malformed)?;
    Decimal::new(if negative { -coefficient } else { coefficient }, scale)
        .map_err(|_| InspectError::Malformed)
}

#[cfg(any(test, feature = "ace16-write-preview"))]
pub(super) fn encode(value: Decimal) -> [u8; NUMERIC_BYTES] {
    let mut bytes = [0; NUMERIC_BYTES];
    if value.coefficient() < 0 {
        bytes[0] = NEGATIVE_SIGN;
    }
    let canonical = value.coefficient().unsigned_abs().to_be_bytes();
    for (input, output) in canonical.as_chunks::<WORD_BYTES>().0.iter().zip(
        bytes[SIGN_BYTES..]
            .as_chunks_mut::<WORD_BYTES>()
            .0
            .iter_mut(),
    ) {
        output.copy_from_slice(&u32::from_be_bytes(*input).to_le_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn independent_word_order_and_precision_golden_values() {
        let mut bytes = [0; 17];
        // 0x00000000_01020304_05060708_090a0b0c, not a little-endian u128.
        bytes[5..].copy_from_slice(&[4, 3, 2, 1, 8, 7, 6, 5, 12, 11, 10, 9]);
        assert_eq!(
            decode(&bytes, 28, 4).unwrap().coefficient(),
            311917102708983781990730508
        );
        assert_eq!(decode(&bytes, 28, 4).unwrap().scale(), 4);
        bytes[0] = 0x80;
        assert_eq!(
            decode(&bytes, 28, 4).unwrap().coefficient(),
            -311917102708983781990730508
        );
        assert_eq!(decode(&bytes, 26, 4), Err(InspectError::Malformed));
        bytes[0] = 1;
        assert_eq!(decode(&bytes, 28, 4), Err(InspectError::Unsupported));
        for (p, s) in [(0, 0), (29, 0), (4, 5)] {
            assert_eq!(decode(&[0; 17], p, s), Err(InspectError::Malformed));
        }
        assert_eq!(decode(&[0; 16], 28, 4), Err(InspectError::Malformed));
        assert_eq!(decode(&[0xff; 17], 28, 4), Err(InspectError::Unsupported));
        let mut overflow = [0xff; 17];
        overflow[0] = 0;
        assert_eq!(decode(&overflow, 28, 4), Err(InspectError::Malformed));
        // Semantic reading maps the native sign of zero to the exact zero carrier.
        assert_eq!(
            decode(
                &[0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                28,
                4
            )
            .unwrap(),
            Decimal::new(0, 4).unwrap()
        );
    }

    #[test]
    fn literal_field_encoding_keeps_word_order_scale_and_sign() {
        let value = Decimal::new(311917102708983781990730508, 4).unwrap();
        let expected = [0, 0, 0, 0, 0, 4, 3, 2, 1, 8, 7, 6, 5, 12, 11, 10, 9];
        assert_eq!(encode(value), expected);
        let mut negative = expected;
        negative[0] = 0x80;
        assert_eq!(
            encode(Decimal::new(-value.coefficient(), 4).unwrap()),
            negative
        );
        assert_eq!(encode(Decimal::new(0, 28).unwrap()), [0; NUMERIC_BYTES]);
    }

    proptest! {
        #![proptest_config(proptest::test_runner::Config{cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x444543494d414c),..Default::default()})]
        #[test]
        fn malformed_numeric_payloads_never_panic(bytes in prop::collection::vec(any::<u8>(), 0..40), precision in any::<u8>(), scale in any::<u8>()) {
            if let Ok(value) = decode(&bytes, precision, scale) {
                prop_assert_eq!(value.scale(), scale);
                prop_assert!(value.coefficient().unsigned_abs() < 10_u128.pow(u32::from(precision)));
            }
        }
    }
}
