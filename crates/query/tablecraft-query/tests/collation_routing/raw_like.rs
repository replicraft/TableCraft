use super::support::*;
use std::cmp::Ordering;
use tablecraft_expressions::{
    EvalError,
    comparison::{Collation, LikeProfile},
    predicates::LikeMode,
};
use tablecraft_query::source::MemorySource;

struct Units(bool);

impl Collation for Units {
    fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
        Err(EvalError::UnsupportedExpression)
    }

    fn like_profile(&self) -> LikeProfile {
        LikeProfile::Utf16Weights
    }

    fn compare_like_units(&self, left: &[u16], right: &[u16]) -> Result<Ordering, EvalError> {
        let fold = |unit: &u16| {
            if self.0 && (65..=90).contains(unit) {
                unit + 32
            } else {
                *unit
            }
        };
        Ok(left.iter().map(fold).cmp(right.iter().map(fold)))
    }

    fn is_like_unit(&self, units: &[u16]) -> Result<bool, EvalError> {
        Ok(units.len() == 1)
    }
}

struct RawResolver;

impl CollationResolver for RawResolver {
    fn resolve(&self, id: TextCollationId) -> Result<&dyn Collation, QueryError> {
        match id.host_key() {
            1 => Ok(&Units(true)),
            2 => Ok(&Units(false)),
            _ => Err(EvalError::CollationRequired.into()),
        }
    }
}

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
