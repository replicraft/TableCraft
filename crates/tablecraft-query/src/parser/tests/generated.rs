use proptest::prelude::*;

use super::*;

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 256,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x5441_424c_4551_5259),
        ..Default::default()
    })]

    #[test]
    fn generated_queries_retain_integer_and_text_syntax(
        number in any::<i64>(),
        text in "[a-zA-Z0-9 '#()\\n]{0,80}",
        count in 1_u32..=100,
        percent in any::<bool>(),
        descending in any::<bool>(),
    ) {
        let sql = format!("SELECT TOP {count} {}[Field Name] FROM [Generated Rows] WHERE [Code] = {number} AND [Label] = '{}' ORDER BY [Field Name] {}",
            if percent { "PERCENT " } else { "" }, text.replace('\'', "''"), if descending { "DESC" } else { "ASC" });
        round_trip(&sql);
    }

    #[test]
    fn arbitrary_input_returns_a_result_without_panicking(source in ".{0,512}") {
        let _ = parse_select(&source, ParseLimits::default());
        let _ = parse_select(&format!("SELECT * FROM Samples WHERE {source}"), ParseLimits::default());
    }
}
