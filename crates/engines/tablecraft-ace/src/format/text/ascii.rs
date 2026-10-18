//! SIMD classifiers over raw bytes; no casts of validated semantic values.
use wide::u8x16;

const LANES: usize = size_of::<u8x16>();
const UTF16_UNIT_BYTES: usize = size_of::<u16>();
const UTF16_NON_ASCII_MASK: u8x16 = u8x16::new([
    0x80, 0xff, 0x80, 0xff, 0x80, 0xff, 0x80, 0xff, 0x80, 0xff, 0x80, 0xff, 0x80, 0xff, 0x80, 0xff,
]);

pub(super) fn compressed(bytes: &[u8]) -> bool {
    let (blocks, tail) = bytes.as_chunks::<LANES>();
    blocks.iter().all(|block| {
        // By-value POD conversion avoids alignment requirements; byte lanes have
        // identical meaning on both host endian orders and accept all patterns.
        let lanes: u8x16 = bytemuck::cast(*block);
        !(lanes.simd_eq(u8x16::splat(0)) | lanes.simd_gt(u8x16::splat(0x7f))).any()
    }) && tail.iter().all(|b| *b != 0 && b.is_ascii())
}

pub(super) fn utf16le(bytes: &[u8]) -> bool {
    if !bytes.len().is_multiple_of(UTF16_UNIT_BYTES) {
        return false;
    }
    let (blocks, tail) = bytes.as_chunks::<LANES>();
    blocks.iter().all(|block| {
        let lanes: u8x16 = bytemuck::cast(*block);
        !(lanes & UTF16_NON_ASCII_MASK)
            .simd_ne(u8x16::splat(0))
            .any()
    }) && tail
        .as_chunks::<UTF16_UNIT_BYTES>()
        .0
        .iter()
        .all(|pair| pair[0].is_ascii() && pair[1] == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn full_vectors_and_tail_reject_every_non_ascii_or_mode_switch_lane() {
        for length in 0..64 {
            let compressed_ascii = vec![b'A'; length];
            assert!(compressed(&compressed_ascii));
            for at in 0..length {
                for invalid in [0, 0x80, 0xff] {
                    let mut bytes = compressed_ascii.clone();
                    bytes[at] = invalid;
                    assert!(!compressed(&bytes));
                }
            }
            let utf16: Vec<u8> = (0..length).flat_map(|_| [b'A', 0]).collect();
            assert!(utf16le(&utf16));
            for at in 0..utf16.len() {
                let mut bytes = utf16.clone();
                bytes[at] = if at % 2 == 0 { 0x80 } else { 1 };
                assert!(!utf16le(&bytes));
            }
        }
    }

    proptest! {
        #![proptest_config(proptest::test_runner::Config {
            cases: 256,
            rng_seed: proptest::test_runner::RngSeed::Fixed(0x4153434949),
            ..Default::default()
        })]
        #[test]
        fn byte_classifiers_match_scalar_rules_including_every_tail(bytes in prop::collection::vec(any::<u8>(),0..128)) {
            prop_assert_eq!(compressed(&bytes), bytes.iter().all(|b| (1..=127).contains(b)));
            let expected = bytes.len().is_multiple_of(2) && bytes.iter().enumerate().all(|(i, byte)| if i % 2 == 0 { *byte <= 127 } else { *byte == 0 });
            prop_assert_eq!(utf16le(&bytes), expected);
        }
    }
}
