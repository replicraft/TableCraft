use super::*;

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 32,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x4649_5845_444b),
        ..Default::default()
    })]

    #[test]
    fn decimal_group_and_distinct_match_big_integer_cross_products(
        inputs in prop::collection::vec((any::<i64>(), 0_u8..=28, 0_u8..=8), 0..24)
    ) {
        let mut keys = Vec::new();
        for (coefficient, scale, extra) in inputs {
            let coefficient = i128::from(coefficient);
            keys.push(decimal(coefficient, scale));
            if u16::from(scale) + u16::from(extra) <= 28 {
                let enlarged = coefficient * 10_i128.pow(u32::from(extra));
                // i64 magnitudes enlarged by at most 10^8 fit the Decimal envelope.
                keys.push(decimal(enlarged, scale + extra));
            }
            keys.push(decimal(-coefficient, scale));
        }
        keys.extend([Value::Null, decimal(0, 28), decimal(0, 0)]);
        assert_scalar_population(ScalarType::Decimal, keys);
    }

    #[test]
    fn currency_group_and_distinct_match_signed_wide_integer_units(
        inputs in prop::collection::vec(any::<i64>(), 0..40)
    ) {
        let mut keys = Vec::new();
        for value in inputs {
            keys.extend([currency(value), currency(value)]);
        }
        keys.extend([Value::Null, currency(i64::MIN), currency(i64::MAX)]);
        assert_scalar_population(ScalarType::Currency, keys);
    }
}
