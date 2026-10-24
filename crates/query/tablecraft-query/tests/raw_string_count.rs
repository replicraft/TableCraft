use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "Codes".into(),
            vec![Column::new("N".into(), ScalarType::Int32, true).unwrap()],
            vec![
                vec![Value::Int32(55296)],
                vec![Value::Int32(65)],
                vec![Value::Int32(57343)],
            ],
        )
        .unwrap(),
    ])
    .unwrap()
}

#[test]
fn count_observes_raw_string_nullness_without_unicode_publication() {
    let source = source();
    for sql in [
        "SELECT Count(ChrW(N)),Count(CVar(ChrW(N))),Count(ChrB(65)),Count(*),Count(ChrW(N)+NULL) FROM Codes",
        "SELECT DCount('ChrW(N)','Codes'),DCount('CVar(ChrW(N))','Codes'),DCount('ChrB(65)','Codes'),DCount('*','Codes'),DCount('ChrW(N)+NULL','Codes')",
        "SELECT Count(ChrW(N)),Count(CVar(ChrW(N))),Count(ChrB(65)),Count(*),Count(ChrW(N)+NULL) FROM Codes HAVING Count(ChrW(N))=3",
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let result =
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default());

        assert_eq!(
            result.map(|result| result.rows().to_vec()),
            Ok(vec![vec![
                Value::Int64(3),
                Value::Int64(3),
                Value::Int64(3),
                Value::Int64(3),
                Value::Int64(0),
            ]]),
            "{sql}"
        );
    }
}

#[test]
fn grouped_filtered_and_empty_counts_keep_row_membership() {
    let source = source();
    for (sql, expected) in [
        (
            "SELECT N Mod 2 AS K,Count(LeftB(ChrW(N),1)),Count(ChrW(N)+NULL) FROM Codes GROUP BY N Mod 2 ORDER BY N Mod 2",
            vec![
                vec![Value::Int64(0), Value::Int64(1), Value::Int64(0)],
                vec![Value::Int64(1), Value::Int64(2), Value::Int64(0)],
            ],
        ),
        (
            "SELECT Count(CStr(ChrW(N))),Count(ChrB(65)) FROM Codes WHERE False",
            vec![vec![Value::Int64(0), Value::Int64(0)]],
        ),
        (
            "SELECT DCount('LeftB(ChrW(N),1)','Codes','N>65'),DCount('ChrW(N)','Codes','False')",
            vec![vec![Value::Int64(2), Value::Int64(0)]],
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
fn count_keeps_constructor_and_eager_errors_visible() {
    let source = source();
    for (expression, expected) in [
        ("ChrW(65536)", EvalError::Domain),
        ("CStr(NULL)", EvalError::InvalidNullConversion),
        ("ChrW(NULL)", EvalError::InvalidNullConversion),
        ("ChrW(N)+(1/0)", EvalError::DivisionByZero),
        ("LeftB(ChrW(N),-1)", EvalError::Domain),
    ] {
        let sql = format!("SELECT Count({expression}) FROM Codes");
        let parsed = parse_query(&sql, Default::default()).unwrap();
        let result =
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default());

        assert_eq!(
            result.err(),
            Some(QueryError::Evaluation(expected)),
            "{sql}"
        );
    }
}

#[test]
fn raw_count_cancels_every_observed_checkpoint_and_retains_limits() {
    let source = source();
    let parsed = parse_query("SELECT Count(CVar(ChrW(N))) FROM Codes", Default::default()).unwrap();
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

    assert!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                limits: QueryLimits {
                    bytes: 0,
                    ..Default::default()
                },
                ..Default::default()
            }
        )
        .is_err()
    );
}
