use super::*;

#[test]
fn source_failures_and_swallowed_operand_errors_never_publish_an_extreme() {
    let mut source = Data::new(
        ScalarType::Int32,
        vec![Value::Int32(1), Value::Int32(2), Value::Int32(3)],
    );
    source.swallow = true;
    source.fail_eof = true;
    let result = stream(&source, "SELECT Min(1/(N-2)),Max(N) FROM T", options(false));
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
    assert_eq!(source.visits.get(), 3);

    let mut source = Data::new(ScalarType::Int32, vec![Value::Int32(1)]);
    source.fail_eof = true;
    assert!(matches!(
        stream(&source, "SELECT Min(N) FROM T", options(false)),
        Err(StreamError::Query(QueryError::Source))
    ));
}

#[test]
fn a_sink_error_after_first_completed_arm_prevents_the_next_scan() {
    let source = Data::new(ScalarType::Int32, vec![Value::Int32(3), Value::Int32(1)]);
    let syntax = parse_query(
        "SELECT Min(N) AS V FROM T UNION ALL SELECT Max(N) FROM T",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let calls = Cell::new(0);
    let result = plan.execute_progressive(&source, &[], options(false), &mut |row| {
        assert!(source.eof.get());
        assert!(!source.active.get());
        assert_eq!(row, [Value::Int32(1)]);
        calls.set(calls.get() + 1);
        Err("closed")
    });
    assert!(matches!(result, Err(StreamError::Sink("closed"))));
    assert_eq!(calls.get(), 1);
    assert_eq!(source.scans.get(), 1);
}

#[test]
fn every_observed_checkpoint_and_cumulative_row_work_ceiling_is_enforced() {
    let source = Data::new(
        ScalarType::Int32,
        vec![Value::Int32(3), Value::Null, Value::Int32(1)],
    );
    let sql = "SELECT Min(N),Max(N),Count(N) FROM T";
    let calls = Cell::new(0);
    let observe = || {
        calls.set(calls.get() + 1);
        false
    };
    stream(
        &source,
        sql,
        ExecutionOptions {
            cancelled: Some(&observe),
            ..options(false)
        },
    )
    .unwrap();
    let total = calls.get();
    assert!(total > 0);

    for stop in 1..=total {
        calls.set(0);
        let cancel = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let result = stream(
            &source,
            sql,
            ExecutionOptions {
                cancelled: Some(&cancel),
                ..options(false)
            },
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
    for limits in [
        QueryLimits {
            rows: 1,
            ..Default::default()
        },
        QueryLimits {
            steps: 1,
            ..Default::default()
        },
        QueryLimits {
            bytes: 1,
            ..Default::default()
        },
    ] {
        assert!(
            stream(
                &source,
                sql,
                ExecutionOptions {
                    limits,
                    ..options(false)
                }
            )
            .is_err()
        );
    }
}
