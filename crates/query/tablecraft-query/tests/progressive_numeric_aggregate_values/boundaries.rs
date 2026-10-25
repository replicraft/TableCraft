use super::*;

#[test]
fn prefix_overflow_and_unsupported_operands_are_not_repaired_by_later_rows() {
    for (kind, values, sql, error) in [
        (
            ScalarType::Int64,
            vec![Value::Int64(i64::MAX), Value::Int64(1), Value::Int64(-1)],
            "SELECT Sum(N) FROM T",
            EvalError::Overflow,
        ),
        (
            ScalarType::Currency,
            vec![
                Value::Currency(Currency::from_scaled(i64::MAX)),
                Value::Currency(Currency::from_scaled(1)),
                Value::Currency(Currency::from_scaled(-1)),
            ],
            "SELECT Sum(N) FROM T",
            EvalError::Overflow,
        ),
        (
            ScalarType::Int32,
            vec![Value::Int32(1)],
            "SELECT Sum(CVErr(7)) FROM T",
            EvalError::TypeMismatch,
        ),
        (
            ScalarType::Int32,
            vec![Value::Int32(1)],
            "SELECT Avg(ChrB(65)) FROM T",
            EvalError::UnrepresentableText,
        ),
    ] {
        let source = Data::new(kind, values);
        let result = stream(sql, &source, Default::default());
        assert!(
            matches!(result, Err(StreamError::Query(QueryError::Evaluation(actual))) if actual == error),
            "{sql}"
        );
        assert!(!source.eof.get(), "{sql}");
    }
}

#[test]
fn first_operand_failure_stays_latched_even_if_source_swallows_and_fails_eof() {
    let mut source = Data::new(
        ScalarType::Int32,
        vec![Value::Int32(1), Value::Int32(0), Value::Int32(2)],
    );
    source.swallow = true;
    source.fail_eof = true;
    let result = stream(
        "SELECT Sum(1/N),Avg(N),Count(*) FROM T",
        &source,
        Default::default(),
    );
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
    assert_eq!(source.visits.get(), 3);

    let mut source = Data::new(ScalarType::Int32, vec![Value::Int32(1)]);
    source.fail_eof = true;
    let result = stream("SELECT Sum(N),Avg(N) FROM T", &source, Default::default());
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Source))
    ));
}

#[test]
fn simultaneous_floating_states_charge_separate_scratch_without_quota_refills() {
    let source = Data::new(ScalarType::Double, vec![double(1.0), double(3.0)]);
    let options = ExecutionOptions {
        limits: QueryLimits {
            bytes: 60 * 1024,
            ..Default::default()
        },
        ..Default::default()
    };
    let single = stream("SELECT Sum(N) FROM T", &source, options).unwrap();
    assert_eq!(single, [vec![double(4.0)]]);

    let result = stream("SELECT Sum(N),Avg(N) FROM T", &source, options);
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Limit(_)))
    ));
}

#[test]
fn every_numeric_aggregate_checkpoint_can_cancel_and_later_runs_are_independent() {
    let source = Data::new(
        ScalarType::Double,
        vec![double(1.0), double(3.0), Value::Null],
    );
    let sql = "SELECT Sum(N),Avg(N),Count(N) FROM T";
    let observed = Cell::new(0);
    let observe = || {
        observed.set(observed.get() + 1);
        false
    };
    stream(
        sql,
        &source,
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
        let result = stream(
            sql,
            &source,
            ExecutionOptions {
                cancelled: Some(&cancel),
                ..Default::default()
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
    let rows = stream(sql, &source, Default::default()).unwrap();
    assert_eq!(rows, [vec![double(4.0), double(2.0), Value::Int64(2)]]);
}

#[test]
fn final_projection_is_eager_and_sink_failure_stops_later_union_arms() {
    let source = Data::new(ScalarType::Int32, vec![Value::Int32(1)]);
    let result = stream(
        "SELECT TOP 1 Sum(N),1/0 FROM T",
        &source,
        Default::default(),
    );
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
    assert!(source.eof.get());

    let syntax = parse_query(
        "SELECT Sum(N) AS V FROM T UNION ALL SELECT Avg(N) FROM T",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    source.projections.borrow_mut().clear();
    let mut calls = 0;
    let result = plan.execute_progressive(&source, &[], Default::default(), &mut |_| {
        assert!(source.eof.get());
        assert!(!source.active.get());
        calls += 1;
        Err("sink stopped")
    });
    assert!(matches!(result, Err(StreamError::Sink("sink stopped"))));
    assert_eq!(calls, 1);
    assert_eq!(source.projections.borrow().len(), 1);
}
