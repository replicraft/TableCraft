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

    #[test]
    fn both_text_delimiters_retain_identical_unicode_contents(
        characters in prop::collection::vec(any::<char>(), 0..80),
    ) {
        let text: String = characters.into_iter().collect();
        let single = format!("SELECT * FROM Samples WHERE Label = '{}'", text.replace('\'', "''"));
        let double = format!("SELECT * FROM Samples WHERE Label = \"{}\"", text.replace('"', "\"\""));
        let single = parsed(&single);
        let double = parsed(&double);
        prop_assert_eq!(single.query(), double.query());
        let Expression::Binary { right, .. } = double.query().selection.as_ref().unwrap() else {
            unreachable!("comparison expected");
        };
        prop_assert_eq!(right.as_ref(), &Expression::Text(text));
        round_trip(double.source());
    }

    #[test]
    fn between_retains_arbitrary_written_bounds(
        first in any::<i64>(), second in any::<i64>(), negated in any::<bool>(),
    ) {
        let sql = format!("SELECT * FROM Samples WHERE Code {}BETWEEN {first} AND {second}",
            if negated { "NOT " } else { "" });
        let result = parsed(&sql);
        let Expression::Between { first_bound, second_bound, negated: actual, .. } =
            result.query().selection.as_ref().unwrap() else { unreachable!("BETWEEN expected"); };
        prop_assert_eq!(first_bound.as_ref(), &Expression::Integer(first));
        prop_assert_eq!(second_bound.as_ref(), &Expression::Integer(second));
        prop_assert_eq!(*actual, negated);
        round_trip(&sql);
    }

    #[test]
    fn in_list_retains_arbitrary_integer_sequence(
        integers in prop::collection::vec(any::<i64>(), 1..32), negated in any::<bool>(),
    ) {
        let entries = integers.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ");
        let sql = format!("SELECT * FROM Samples WHERE Code {}IN ({entries})", if negated { "NOT " } else { "" });
        let result = parsed(&sql);
        let Expression::InList { values, negated: actual, .. } = result.query().selection.as_ref().unwrap() else { unreachable!("IN expected"); };
        let expected = integers.into_iter().map(Expression::Integer).collect::<Vec<_>>();
        prop_assert_eq!(values, &expected);
        prop_assert_eq!(*actual, negated);
        round_trip(&sql);
    }
}
