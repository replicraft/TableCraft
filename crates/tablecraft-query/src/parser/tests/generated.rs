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
        let _ = crate::parse_query(&source, ParseLimits::default());
        let _ = crate::parse_query(&format!("PARAMETERS p LONG; SELECT {source} FROM Samples"), ParseLimits::default());
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
    fn generated_query_families_preserve_complete_owned_models(
        value in any::<i64>(), kind in 0_u8..13,
        letters in "[a-zA-Z0-9 '\"#(),]{0,24}",
    ) {
        let literal = format!("'{}'", letters.replace('\'', "''"));
        let sql = match kind {
            0 => format!("SELECT IIf(Code > {value}, {literal}, NULL) AS ResultValue FROM Samples ORDER BY Code + 1"),
            1 => format!("SELECT Label, Sum(Code) FROM Samples GROUP BY Label HAVING Max(Code) > {value}"),
            2 => format!("PARAMETERS [Minimum] LONG; SELECT Code FROM Samples WHERE Code > [Minimum] UNION ALL SELECT {value} FROM Others"),
            3 => format!("TRANSFORM Sum(Code) SELECT Label FROM Samples GROUP BY Label PIVOT Flag IN ({literal}, NULL)"),
            4 => format!("INSERT INTO Samples (Code, Label) VALUES ({value}, {literal})"),
            5 => format!("UPDATE Samples SET Code = {value}, Label = {literal} WHERE Code NOT IN (1, NULL)"),
            6 => format!("DELETE FROM Samples WHERE Code = {value} AND Label LIKE {literal}"),
            7 => format!("SELECT s.Code FROM Samples AS s LEFT JOIN Others AS o ON s.Code = o.Code WHERE o.Code > {value}"),
            8 => format!("SELECT Code INTO NewRows IN 'target.accdb' FROM Samples IN 'source.accdb' WHERE Code > {value}"),
            9 => format!("CREATE TABLE GeneratedRows ([Field {letters}] DECIMAL(28, 4), CONSTRAINT GeneratedKey PRIMARY KEY ([Field {letters}]))"),
            10 => format!("CREATE UNIQUE INDEX GeneratedIndex ON GeneratedRows ([Field {letters}] DESC) WITH IGNORE NULL"),
            11 => format!("ALTER TABLE GeneratedRows ADD CONSTRAINT ParentRef FOREIGN KEY ([Field {letters}]) REFERENCES ParentRows ON DELETE SET NULL"),
            _ => format!("ALTER TABLE GeneratedRows ADD COLUMN [Field {letters}] TEXT(80) NOT NULL"),
        };
        super::queries::query_round_trip(&sql);
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
