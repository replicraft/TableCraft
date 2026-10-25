use super::support::*;
use std::{cell::Cell, cmp::Ordering};
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::QueryLimits;

pub(super) struct TrimProvider;

impl Collation for TrimProvider {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.trim_end_matches(' ').cmp(right.trim_end_matches(' ')))
    }
}

impl CollationResolver for TrimProvider {
    fn resolve(&self, id: TextCollationId) -> Result<&dyn Collation, QueryError> {
        match id.host_key() {
            1 => Ok(self),
            3 => Ok(&DEFAULT),
            _ => Err(EvalError::CollationRequired.into()),
        }
    }
}

#[test]
fn late_group_policy_merges_earlier_distinct_neutral_keys() {
    let source = Fixture::new();
    let sql = concat!(
        "SELECT Count(*) FROM T GROUP BY ",
        "DLookup(Choose(Id,\"'a'\",\"'a '\",'A',\"'a'\"),'T','Id=1')"
    );
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let result = plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&DEFAULT),
                collation_resolver: Some(&TrimProvider),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.rows(), [vec![Value::Int64(4)]]);
}

#[test]
fn late_group_policy_separates_keys_that_default_would_merge() {
    let source = Fixture::new();
    let sql = concat!(
        "SELECT Count(*) FROM T GROUP BY ",
        "DLookup(Choose(Id,\"'a'\",\"'a '\",'K',\"'a'\"),'T','Id=1')"
    );
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let result = plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&TrimProvider),
                collation_resolver: Some(&TrimProvider),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        result.rows(),
        [vec![Value::Int64(3)], vec![Value::Int64(1)]]
    );
}

#[test]
fn required_group_policy_does_not_require_an_unused_default_comparer() {
    let source = Fixture::new();
    let parsed = parse_query(
        concat!(
            "SELECT Count(*) FROM T GROUP BY ",
            "DLookup(Choose(Id,\"'a'\",\"'a '\",'A',\"'a'\"),'T','Id=1')"
        ),
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    let result = plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation_resolver: Some(&TrimProvider),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.rows(), [vec![Value::Int64(4)]]);
    assert_eq!(source.scans.get(), 5);
}

#[test]
fn group_key_columns_merge_across_disjoint_numeric_partitions() {
    let source = Fixture::new();
    let parsed = parse_query(
        concat!(
            "SELECT Count(*) FROM T GROUP BY IIf(Id<3,1,2),",
            "DLookup(Choose(Id,\"'a'\",\"'a '\",'A',\"'a'\"),'T','Id=1')"
        ),
        ParseLimits::default(),
    )
    .unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&DEFAULT),
                collation_resolver: Some(&TrimProvider),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        result.rows(),
        [vec![Value::Int64(2)], vec![Value::Int64(2)]]
    );
}

#[test]
fn final_group_policy_is_retained_in_named_outputs_and_crosstab_row_groups() {
    let mut source = Fixture::new();
    let key = r#"DLookup(Choose(Id,"'a'","'a '",'A',"'a'"),'T','Id=1')"#;
    source.named = parse_query(
        &format!("SELECT {key} AS X,Count(*) AS N FROM T GROUP BY {key}"),
        ParseLimits::default(),
    )
    .unwrap()
    .into_query();
    for (sql, expected) in [
        (
            "SELECT X='a ',N FROM Named".into(),
            vec![Value::Boolean(true), Value::Int64(4)],
        ),
        (
            format!("TRANSFORM Count(*) SELECT 1 AS G FROM T GROUP BY {key} PIVOT 1 IN (1)"),
            vec![Value::Int64(1), Value::Int64(4)],
        ),
    ] {
        let parsed = parse_query(&sql, ParseLimits::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(&DEFAULT),
                    collation_resolver: Some(&TrimProvider),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.rows(), [expected]);
    }
}

#[test]
fn retained_group_keys_share_limits_cancellation_and_eager_failures() {
    let source = Fixture::new();
    let parsed = parse_query(
        concat!(
            "SELECT Count(*) FROM T GROUP BY ",
            "DLookup(Choose(Id,\"'a'\",\"'a '\",'A',\"'a'\"),'T','Id=1')"
        ),
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let observed = Cell::new(0);
    let count = || {
        observed.set(observed.get() + 1);
        false
    };
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            collation: Some(&DEFAULT),
            collation_resolver: Some(&TrimProvider),
            cancelled: Some(&count),
            ..Default::default()
        },
    )
    .unwrap();
    for stop in 1..=observed.get() {
        let calls = Cell::new(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        assert!(
            matches!(
                plan.execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        collation: Some(&DEFAULT),
                        collation_resolver: Some(&TrimProvider),
                        cancelled: Some(&cancelled),
                        ..Default::default()
                    },
                ),
                Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "checkpoint {stop}"
        );
    }
    assert!(matches!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                limits: QueryLimits {
                    bytes: 1,
                    ..Default::default()
                },
                collation: Some(&DEFAULT),
                collation_resolver: Some(&TrimProvider),
                ..Default::default()
            },
        ),
        Err(QueryError::Limit(_))
    ));
    let parsed = parse_query(
        concat!(
            "SELECT Count(*) FROM T GROUP BY ",
            "DLookup(Choose(Id,\"'a'\",\"'a '\",'A',\"1/0\"),'T','Id=1')"
        ),
        ParseLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        prepare(parsed.query(), &source).unwrap().execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&DEFAULT),
                collation_resolver: Some(&TrimProvider),
                ..Default::default()
            },
        ),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    ));
}

#[test]
fn explicit_global_group_comparer_keeps_its_existing_override() {
    let source = Fixture::new();
    let sql = concat!(
        "SELECT Count(*) FROM T GROUP BY ",
        "DLookup(Choose(Id,\"'a'\",\"'a '\",'K',\"'a'\"),'T','Id=1')"
    );
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let result = plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&TrimProvider),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.rows(), [vec![Value::Int64(4)]]);
}

#[test]
fn rows_rejected_by_where_do_not_contribute_group_key_policies() {
    let source = Fixture::new();
    let parsed = parse_query(
        concat!(
            "SELECT Count(*) FROM T WHERE Id<3 GROUP BY ",
            "DLookup(Choose(Id,\"'a'\",\"'a '\",'A',\"'a'\"),'T','Id=1')"
        ),
        ParseLimits::default(),
    )
    .unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&DEFAULT),
                collation_resolver: Some(&TrimProvider),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        result.rows(),
        [vec![Value::Int64(1)], vec![Value::Int64(1)]]
    );
    assert_eq!(source.scans.get(), 3);
}
