//! Bitmap selection shared by allocation and index decoders.
mod kernel;
pub(crate) use kernel::{BLOCK_BYTES, SetBits, set_bits};

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn lane_order_tails_and_high_bits_match_independent_golden_positions() {
        let mut input = [0; 16];
        input[0] = 0x81;
        input[7] = 0x80;
        input[15] = 0x80;
        let actual: Vec<_> = input
            .chunks(8)
            .enumerate()
            .flat_map(|(i, block)| set_bits(block).map(move |bit| i * 64 + bit))
            .collect();
        assert_eq!(actual, [0, 7, 63, 127]);
        assert_eq!(
            set_bits(&[0, 0xff, 1]).collect::<Vec<_>>(),
            [8, 9, 10, 11, 12, 13, 14, 15, 16]
        );
        assert!(set_bits(&[]).next().is_none());
    }

    proptest! {
        #![proptest_config(proptest::test_runner::Config {
            cases: 256,
            rng_seed: proptest::test_runner::RngSeed::Fixed(0x4249544d4150),
            ..Default::default()
        })]
        #[test]
        fn all_lengths_and_bytes_match_scalar_bit_positions(bytes in prop::collection::vec(any::<u8>(), 0..=8)) {
            let expected: Vec<_> = (0..bytes.len() * 8).filter(|i| bytes[i / 8] & (1 << (i % 8)) != 0).collect();
            prop_assert_eq!(set_bits(&bytes).collect::<Vec<_>>(), expected);
        }
    }
}
