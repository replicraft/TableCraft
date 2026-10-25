use super::*;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::QueryLimits;

#[test]
fn unsupported_shapes_are_rejected_before_scan_even_when_empty_or_false() {
    for values in [vec![], vec![1]] {
        for sql in [
            "SELECT DISTINCT N FROM T WHERE False",
            "SELECT TOP 1 PERCENT N FROM T WHERE False",
            "SELECT TOP 1 N FROM T WHERE False ORDER BY N",
            "SELECT N FROM T WHERE False ORDER BY N",
            "SELECT Last(N) FROM T WHERE False",
            "SELECT N FROM T WHERE False GROUP BY N",
            "SELECT N FROM T WHERE False UNION SELECT N FROM T",
            "SELECT A.N FROM T AS A INNER JOIN T AS B ON A.N=B.N WHERE False",
            "SELECT (SELECT 1) FROM T WHERE False",
        ] {
            let source = Rows::new(values.clone());
            let parsed = parse_query(sql, Default::default()).unwrap();
            let plan = prepare(parsed.query(), &source).unwrap();
            let result = plan
                .execute_progressive(&source, &[], Default::default(), &mut |_| Ok::<_, ()>(()));
            assert!(
                matches!(result, Err(StreamError::Query(QueryError::Unsupported))),
                "{sql}"
            );
            assert_eq!(source.scans.get(), 0, "{sql}");
        }
    }
}

#[test]
fn metadata_and_parameter_changes_fail_before_any_source_scan() {
    let source = Rows::new([1, 2]);
    let parsed = parse_query("PARAMETERS P Long; SELECT N+P FROM T", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let result = plan.execute_progressive(
        &source,
        &[("P", Value::Text("1".into()))],
        Default::default(),
        &mut |_| Ok::<_, ()>(()),
    );
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Parameter))
    ));
    assert_eq!(source.scans.get(), 0);

    source.changed.set(true);
    let result = plan.execute_progressive(
        &source,
        &[("P", Value::Int32(1))],
        Default::default(),
        &mut |_| Ok::<_, ()>(()),
    );
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::SchemaChanged))
    ));
    assert_eq!(source.scans.get(), 0);
}

#[test]
fn every_observed_cancellation_checkpoint_prevents_successful_eof() {
    let source = Rows::new([1, 2, 3]);
    let parsed = parse_query("SELECT N,N+1 FROM T WHERE N>0", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let observed = Cell::new(0);
    let observe = || {
        observed.set(observed.get() + 1);
        false
    };
    plan.execute_progressive(
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&observe),
            ..Default::default()
        },
        &mut |_| Ok::<_, ()>(()),
    )
    .unwrap();
    assert!(observed.get() > 3);

    for stop in 1..=observed.get() {
        let calls = Cell::new(0);
        let cancel = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let result = plan.execute_progressive(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancel),
                ..Default::default()
            },
            &mut |_| Ok::<_, ()>(()),
        );
        assert!(
            matches!(
                result,
                Err(StreamError::Query(
                    QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled)
                ))
            ),
            "checkpoint {stop}"
        );
    }
}

#[test]
fn cumulative_limits_are_not_refilled_for_each_emitted_row() {
    for limits in [
        QueryLimits {
            rows: 4,
            ..Default::default()
        },
        QueryLimits {
            steps: 100,
            ..Default::default()
        },
        QueryLimits {
            bytes: 4096,
            ..Default::default()
        },
    ] {
        let source = Rows::new(0..200);
        let parsed = parse_query("SELECT N,CStr(N) FROM T", Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        let single = Rows::new([0]);
        plan.execute_progressive(
            &single,
            &[],
            ExecutionOptions {
                limits,
                number_format: Some(NumberFormat::Invariant),
                ..Default::default()
            },
            &mut |_| Ok::<_, ()>(()),
        )
        .expect("each quota admits one complete row independently");

        let emitted = Cell::new(0);
        let result = plan.execute_progressive(
            &source,
            &[],
            ExecutionOptions {
                limits,
                number_format: Some(NumberFormat::Invariant),
                ..Default::default()
            },
            &mut |_| {
                emitted.set(emitted.get() + 1);
                Ok::<_, ()>(())
            },
        );
        assert!(matches!(
            result,
            Err(StreamError::Query(
                QueryError::Limit(_) | QueryError::Evaluation(EvalError::Limit(_))
            ))
        ));
        assert!(emitted.get() < 200);
    }
}

#[test]
fn a_source_without_explicit_progressive_capability_never_uses_buffered_scan() {
    struct BufferedOnly(Rows);

    impl Source for BufferedOnly {
        fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
            self.0.columns(table)
        }

        fn scan(
            &self,
            table: &str,
            visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
        ) -> Result<(), QueryError> {
            self.0.scan(table, visitor)
        }
    }

    let source = BufferedOnly(Rows::new([1, 2]));
    let parsed = parse_query("SELECT N FROM T", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let result =
        plan.execute_progressive(&source, &[], Default::default(), &mut |_| Ok::<_, ()>(()));
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Unsupported))
    ));
    assert_eq!(source.0.scans.get(), 0);
}

#[test]
fn a_swallowed_row_evaluation_error_cannot_become_successful_eof() {
    let mut source = Rows::new([1, 0, 2]);
    source.swallow = true;
    let parsed = parse_query("SELECT 1/N FROM T", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let emitted = Cell::new(0);
    let result = plan.execute_progressive(&source, &[], Default::default(), &mut |_| {
        emitted.set(emitted.get() + 1);
        Ok::<_, ()>(())
    });
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
    assert_eq!(emitted.get(), 1);
}
