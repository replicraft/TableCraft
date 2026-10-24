use super::{
    raw_support::{RawResolver, Units},
    support::*,
};
use std::cell::Cell;
use tablecraft_expressions::EvalError;
use tablecraft_query::source::MemorySource;

#[test]
fn raw_comparisons_keep_field_policies_and_refuse_unknown_or_conflicting_ones() {
    let source = Fixture::new();
    let options = ExecutionOptions {
        collation_resolver: Some(&RawResolver),
        ..Default::default()
    };
    let parsed = parse_query(
        "SELECT A&ChrW(55296)='a'&ChrW(55296),B&ChrW(55296)='A'&ChrW(55296) FROM T WHERE Id=1",
        Default::default(),
    )
    .unwrap();
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
            "SELECT A&ChrW(55296)=B&ChrW(55296) FROM T",
            QueryError::CollationConflict,
        ),
        (
            "SELECT U&ChrW(55296)=ChrW(55296) FROM T",
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

#[test]
fn raw_comparison_consumers_share_cancellation_and_keep_eager_failures() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "SELECT S=ChrW(55296),S IN(ChrW(55296)),S BETWEEN ChrW(55296) AND ChrW(55297) FROM (SELECT ChrW(55296) AS S) AS Q",
        Default::default(),
    ).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let calls = Cell::new(0);
    let observe = || {
        calls.set(calls.get() + 1);
        false
    };
    let result = plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&Units(false)),
                cancelled: Some(&observe),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.rows(), &[vec![Value::Boolean(true); 3]]);

    for stop in 1..=calls.get() {
        let seen = Cell::new(0);
        let cancel = || {
            seen.set(seen.get() + 1);
            seen.get() == stop
        };
        assert!(matches!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(&Units(false)),
                    cancelled: Some(&cancel),
                    ..Default::default()
                }
            ),
            Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }

    for (expression, expected) in [
        ("ChrW(55296) IN(NULL,1/0)", EvalError::DivisionByZero),
        ("ChrB(65)=ChrW(55296)", EvalError::UnrepresentableText),
        ("ChrW(55296)=CVErr(1)", EvalError::TypeMismatch),
    ] {
        let sql = format!("SELECT {expression}");
        let parsed = parse_query(&sql, Default::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        collation: Some(&Units(false)),
                        ..Default::default()
                    }
                )
                .err(),
            Some(QueryError::Evaluation(expected))
        );
    }
}
