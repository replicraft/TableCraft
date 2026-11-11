//! Documented ACE synonyms retain spelling without selecting native storage.
use crate::{parse_query, query::Statement};

#[test]
fn documented_single_word_type_aliases_round_trip_in_fields_and_parameters() {
    for alias in [
        "LOGICAL",
        "LOGICAL1",
        "INTEGER1",
        "INTEGER2",
        "INTEGER4",
        "FLOAT4",
        "FLOAT8",
        "NUMBER",
        "DEC",
        "ALPHANUMERIC",
        "STRING",
        "NCHAR",
        "VARBINARY",
        "GENERAL",
        "OLEOBJECT",
        "NOTE",
        "NTEXT",
    ] {
        for sql in [
            format!("CREATE TABLE T (F {alias})"),
            format!("PARAMETERS P {alias}; SELECT P"),
        ] {
            let parsed = parse_query(&sql, Default::default())
                .unwrap_or_else(|error| panic!("{sql}: {error}"));
            let normalized = parsed.query().to_string();
            let reparsed = parse_query(&normalized, Default::default()).unwrap();
            assert_eq!(parsed.query(), reparsed.query(), "{normalized}");
            if let Statement::Read(_) = &parsed.query().statement {
                assert_eq!(parsed.query().parameters[0].data_type.name, alias);
            }
        }
    }
}

#[test]
fn retired_timestamp_and_quoted_type_names_remain_rejected() {
    for sql in [
        "CREATE TABLE T (F TIMESTAMP)",
        "PARAMETERS P TIMESTAMP; SELECT P",
        "CREATE TABLE T (F [FLOAT4])",
        "PARAMETERS P [INTEGER4]; SELECT P",
    ] {
        assert!(parse_query(sql, Default::default()).is_err(), "{sql}");
    }
}
