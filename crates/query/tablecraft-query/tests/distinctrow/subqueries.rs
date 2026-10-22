use super::*;
use std::{
    cell::Cell,
    sync::atomic::{AtomicUsize, Ordering},
};
use tablecraft_core::values::{ExpressionValue, ExtendedDateTime};
use tablecraft_expressions::functions::{
    dates::clock::{Clock, ClockReading},
    math::random::RndState,
};
use tablecraft_query::QueryLimits;

#[test]
fn scalar_correlations_select_outer_records_across_nested_query_frames() {
    let source = source();
    for sql in [
        "SELECT DISTINCTROW (SELECT A.Shown) AS V FROM A INNER JOIN B ON A.Id=B.Owner ORDER BY V",
        "SELECT DISTINCTROW (SELECT (SELECT A.Shown)) AS V FROM A INNER JOIN B ON A.Id=B.Owner ORDER BY V",
        "SELECT DISTINCTROW (SELECT A.Shown UNION ALL SELECT A.Shown WHERE False) AS V FROM A INNER JOIN B ON A.Id=B.Owner ORDER BY V",
    ] {
        assert_eq!(
            run(sql, &source).unwrap(),
            [
                vec![Value::Int32(7)],
                vec![Value::Int32(7)],
                vec![Value::Int32(8)]
            ]
        );
    }
    assert_eq!(
        run("SELECT DISTINCTROW (SELECT Count(*) FROM B I INNER JOIN B J ON I.Id=J.Id AND I.Owner=A.Id) AS N FROM A INNER JOIN B ON A.Id=B.Owner ORDER BY N", &source).unwrap(),
        [vec![Value::Int64(1)],vec![Value::Int64(2)],vec![Value::Int64(2)]],
    );
}

#[test]
fn inner_local_fields_do_not_select_outer_sources_but_outer_captures_do() {
    let source = source();
    assert_eq!(
        run("SELECT DISTINCTROW A.Shown,(SELECT Count(*) FROM B I WHERE I.Id>0) AS N FROM A INNER JOIN B ON A.Id=B.Owner ORDER BY A.Shown", &source).unwrap(),
        [vec![Value::Int32(7),Value::Int64(3)],vec![Value::Int32(7),Value::Int64(3)],vec![Value::Int32(8),Value::Int64(3)]],
    );
    // Capturing both outer sources retains the existing ignored DISTINCTROW case.
    let rows = run("SELECT DISTINCTROW (SELECT A.Shown+B.Owner) AS V FROM A INNER JOIN B ON A.Id=B.Owner ORDER BY V", &source).unwrap();
    assert_eq!(rows.len(), 7);
    assert_eq!(
        rows.iter().filter(|row| row[0] == Value::Int32(8)).count(),
        6
    );
    assert_eq!(rows.last(), Some(&vec![Value::Int32(10)]));
}

#[test]
fn predicate_subqueries_and_empty_scalar_results_keep_their_existing_values() {
    let source = source();
    for expression in [
        "EXISTS(SELECT 1 FROM B I WHERE I.Owner=A.Id)",
        "A.Id IN(SELECT I.Owner FROM B I)",
        "A.Id=ANY(SELECT I.Owner FROM B I)",
    ] {
        let sql = format!(
            "SELECT DISTINCTROW A.Shown,{expression} AS P FROM A INNER JOIN B ON A.Id=B.Owner ORDER BY A.Shown"
        );
        assert_eq!(
            run(&sql, &source).unwrap(),
            [
                vec![Value::Int32(7), Value::Boolean(true)],
                vec![Value::Int32(7), Value::Boolean(true)],
                vec![Value::Int32(8), Value::Boolean(true)]
            ]
        );
    }
    assert_eq!(
        run("SELECT DISTINCTROW A.Shown,(SELECT 1 WHERE False) AS P FROM A INNER JOIN B ON A.Id=B.Owner ORDER BY A.Shown", &source).unwrap(),
        [vec![Value::Int32(7),Value::Null],vec![Value::Int32(7),Value::Null],vec![Value::Int32(8),Value::Null]],
    );
}

struct CountingClock {
    calls: AtomicUsize,
    reading: ClockReading,
}

impl Clock for CountingClock {
    fn read(&self) -> Result<ClockReading, EvalError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(self.reading)
    }
}

#[test]
fn projections_evaluate_for_all_joined_rows_before_record_deduplication() {
    let source = source();
    let parsed = parse_query("SELECT DISTINCTROW A.Shown,(SELECT Rnd()),(SELECT Now()) FROM A INNER JOIN B ON A.Id=B.Owner",ParseLimits::default()).unwrap();
    let clock = CountingClock {
        calls: AtomicUsize::new(0),
        reading: ClockReading::new(ExtendedDateTime::new(2026, 10, 22, 12, 0, 0, 0).unwrap())
            .unwrap(),
    };
    let state = RndState::new(123).unwrap();
    let expected = RndState::new(123).unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                clock: Some(&clock),
                random: Some(&state),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.rows().len(), 3);
    assert_eq!(clock.calls.load(Ordering::Relaxed), 7);
    for _ in 0..7 {
        expected.apply(None).unwrap();
    }
    assert_eq!(state.apply(None).unwrap(), expected.apply(None).unwrap());
}

#[test]
fn cardinality_late_errors_and_first_class_error_data_are_not_hidden() {
    let source = source();
    assert_eq!(
        run(
            "SELECT DISTINCTROW A.Shown,(SELECT Id FROM B) FROM A INNER JOIN B ON A.Id=B.Owner",
            &source
        )
        .err(),
        Some(QueryError::Cardinality)
    );
    assert_eq!(run("SELECT DISTINCTROW TOP 1 A.Shown,(SELECT 1/(2-A.Id)) FROM A INNER JOIN B ON A.Id=B.Owner",&source).err(),Some(QueryError::Evaluation(EvalError::DivisionByZero)));
    let parsed = parse_query("SELECT DISTINCTROW A.Shown,(SELECT CVErr(5)) FROM A INNER JOIN B ON A.Id=B.Owner ORDER BY A.Shown",ParseLimits::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let result = plan
        .execute_expression(&source, &[], ExecutionOptions::default())
        .unwrap();
    assert_eq!(result.rows().len(), 3);
    assert!(
        result
            .rows()
            .iter()
            .all(|row| row[1] == ExpressionValue::Error(5))
    );
    assert_eq!(
        plan.execute(&source, &[], ExecutionOptions::default())
            .err(),
        Some(QueryError::Evaluation(EvalError::NonScalarExpression))
    );
}

struct MetadataOnly(MemorySource);

impl Source for MetadataOnly {
    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.0.columns(name)
    }

    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("DISTINCTROW capture analysis must not scan records");
    }
}

#[test]
fn subquery_record_keys_bind_without_scans_and_keep_limits_and_cancellation() {
    let source = source();
    let parsed = parse_query(
        "SELECT DISTINCTROW (SELECT A.Shown) AS V FROM A INNER JOIN B ON A.Id=B.Owner ORDER BY V",
        ParseLimits::default(),
    )
    .unwrap();
    prepare(parsed.query(), &MetadataOnly(self::source())).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for limits in [
        QueryLimits {
            bytes: 1,
            ..Default::default()
        },
        QueryLimits {
            rows: 1,
            ..Default::default()
        },
        QueryLimits {
            steps: 1,
            ..Default::default()
        },
    ] {
        assert!(matches!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    limits,
                    ..Default::default()
                }
            ),
            Err(QueryError::Limit(_))
        ));
    }
    let calls = Cell::new(0);
    let stop = Cell::new(usize::MAX);
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() == stop.get()
    };
    let execute = || {
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancelled),
                ..Default::default()
            },
        )
    };
    execute().unwrap();
    for checkpoint in 1..=calls.get() {
        calls.set(0);
        stop.set(checkpoint);
        assert!(matches!(
            execute().err(),
            Some(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }
}
