use super::{
    raw_support::{RawResolver, Units},
    support::*,
};
use tablecraft_expressions::{EvalError, predicates::LikeMode};
use tablecraft_query::source::MemorySource;

#[test]
fn raw_like_uses_explicit_unit_provider_through_derived_rows_and_filters() {
    let source = MemorySource::default();
    for (sql, expected) in [
        (
            "SELECT S LIKE ChrW(55296),S NOT LIKE ChrW(55297),S LIKE '?' FROM (SELECT ChrW(55296) AS S) AS Q",
            vec![vec![Value::Boolean(true); 3]],
        ),
        (
            "SELECT LenB(S) FROM (SELECT ChrW(55296) AS S) AS Q WHERE S LIKE '[!a]'",
            vec![vec![Value::Int32(2)]],
        ),
        (
            "SELECT MidB('😀',1,2) LIKE ChrW(55357),ChrW(55357) LIKE MidB('😀',1,2)",
            vec![vec![Value::Boolean(true); 2]],
        ),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    like_mode: Some(LikeMode::Ansi89),
                    collation: Some(&Units(false)),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.rows(), expected, "{sql}");
    }
}

#[test]
fn raw_like_routes_field_policies_and_refuses_conflicts_and_unknowns() {
    let source = Fixture::new();
    let options = ExecutionOptions {
        like_mode: Some(LikeMode::Ansi89),
        collation_resolver: Some(&RawResolver),
        ..Default::default()
    };
    let parsed = parse_query(
        "SELECT A&ChrW(55296) LIKE 'a'&ChrW(55296),B&ChrW(55296) LIKE 'A'&ChrW(55296) FROM T WHERE Id=1",
        Default::default(),
    ).unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(&source, &[], options)
        .unwrap();
    assert_eq!(
        result.rows(),
        &[vec![Value::Boolean(true), Value::Boolean(false)]]
    );

    for (sql, expected) in [
        (
            "SELECT A&ChrW(55296) LIKE B&ChrW(55296) FROM T",
            QueryError::CollationConflict,
        ),
        (
            "SELECT U&ChrW(55296) LIKE ChrW(55296) FROM T",
            QueryError::Evaluation(EvalError::CollationRequired),
        ),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], options)
                .err(),
            Some(expected),
            "{sql}"
        );
    }
}
