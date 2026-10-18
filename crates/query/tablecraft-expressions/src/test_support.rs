pub(crate) fn config() -> proptest::test_runner::Config {
    proptest::test_runner::Config {
        cases: 256,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x5441_424c_4558_5052),
        ..Default::default()
    }
}
