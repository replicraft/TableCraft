use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn retained_null_predicates_share_projection_filter_group_and_subquery_paths() {
    let source = MemorySource::default();
    for (sql, expected) in [
        (
            "SELECT S=NULL,NULL<>S,S IN(NULL),NULL IN(S),S BETWEEN NULL AND NULL FROM (SELECT ChrW(55296) AS S) AS Q",
            vec![vec![Value::Null; 5]],
        ),
        (
            "SELECT LenB(S) FROM (SELECT ChrB(65) AS S) AS Q WHERE IsNull(S=NULL)",
            vec![vec![Value::Int32(1)]],
        ),
        (
            "SELECT S FROM (SELECT ChrW(55296) AS S) AS Q WHERE S=NULL",
            vec![],
        ),
        (
            "SELECT Count(*),S=NULL FROM (SELECT ChrW(55296) AS S UNION ALL SELECT ChrW(55297)) AS Q GROUP BY S=NULL",
            vec![vec![Value::Int64(2), Value::Null]],
        ),
        (
            "SELECT S IN(SELECT NULL),S=ANY(SELECT NULL),S=ALL(SELECT NULL),S=ANY(SELECT NULL WHERE False),S=ALL(SELECT NULL WHERE False) FROM (SELECT ChrW(55296) AS S) AS Q",
            vec![vec![
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Boolean(false),
                Value::Boolean(true),
            ]],
        ),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap();
        assert_eq!(result.rows(), expected, "{sql}");
    }
}

#[test]
fn variant_raw_string_numeric_order_keeps_issued_declarations() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "SELECT CVar(ChrW(55296))>CVar(0),CVar(0)<CVar(ChrB(65)),CVar(ChrW(55296)) IN(CVar(0),NULL),CVar(ChrB(65)) BETWEEN CVar(0) AND CVar(1)",
        Default::default(),
    ).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: true,
                ..Default::default()
            }
        )
        .unwrap()
        .rows(),
        &[vec![
            Value::Boolean(true),
            Value::Boolean(true),
            Value::Null,
            Value::Boolean(false)
        ]]
    );
    assert_eq!(
        plan.execute(&source, &[], Default::default()).err(),
        Some(QueryError::Evaluation(EvalError::UnrepresentableText))
    );
}

#[test]
fn raw_predicates_keep_eager_errors_and_cancel_every_observed_checkpoint() {
    let source = MemorySource::default();
    for (expression, expected) in [
        ("(S=NULL) AND (1/0)", EvalError::DivisionByZero),
        ("S IN(NULL,1/0)", EvalError::DivisionByZero),
        ("S BETWEEN NULL AND 1/0", EvalError::DivisionByZero),
        ("S=CVErr(1)", EvalError::TypeMismatch),
        ("S='a'", EvalError::UnrepresentableText),
    ] {
        let sql = format!("SELECT {expression} FROM (SELECT ChrW(55296) AS S) AS Q");
        let parsed = parse_query(&sql, Default::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default())
                .err(),
            Some(QueryError::Evaluation(expected)),
            "{sql}"
        );
    }

    let parsed = parse_query(
        "SELECT S IN(NULL),S BETWEEN NULL AND NULL FROM (SELECT ChrW(55296) AS S) AS Q",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let observed = Cell::new(0);
    let observe = || {
        observed.set(observed.get() + 1);
        false
    };
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&observe),
            ..Default::default()
        },
    )
    .unwrap();

    for stop in 1..=observed.get() {
        let calls = Cell::new(0);
        let cancel = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        assert!(matches!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    cancelled: Some(&cancel),
                    ..Default::default()
                }
            ),
            Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }
}
