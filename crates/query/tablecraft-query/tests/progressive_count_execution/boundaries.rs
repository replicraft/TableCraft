use super::*;

#[test]
fn final_projection_errors_and_overflow_happen_after_eof_without_output() {
    for (count, sql, error) in [
        (32768, "SELECT CInt(Count(*)) FROM T", EvalError::Overflow),
        (
            3,
            "SELECT TOP 1 Count(*),1/0 FROM T",
            EvalError::DivisionByZero,
        ),
        (
            1,
            "SELECT IIf(Count(*)=0,'empty',ChrW(55296)) FROM T",
            EvalError::UnrepresentableText,
        ),
    ] {
        let source = Generated::new(count);
        let syntax = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(syntax.query(), &source).unwrap();
        let emissions = Cell::new(0);
        let result = plan.execute_progressive(&source, &[], wide_options(), &mut |_| {
            emissions.set(emissions.get() + 1);
            Ok::<_, ()>(())
        });
        assert!(
            matches!(result, Err(StreamError::Query(QueryError::Evaluation(actual))) if actual == error),
            "{sql}"
        );
        assert_eq!(emissions.get(), 0, "{sql}");
        assert!(source.eof.get(), "{sql}");
        assert_eq!(source.visits.get(), count, "{sql}");
    }
}

#[test]
fn failed_input_or_eof_never_publishes_a_prefix_count() {
    for fail_eof in [false, true] {
        let mut source = Generated::new(5);
        source.fail_eof = fail_eof;
        source.fail_at = (!fail_eof).then_some(3);
        let syntax = parse_query("SELECT Count(*) FROM T", Default::default()).unwrap();
        let plan = prepare(syntax.query(), &source).unwrap();
        let mut emissions = 0;
        let result = plan.execute_progressive(&source, &[], Default::default(), &mut |_| {
            emissions += 1;
            Ok::<_, ()>(())
        });
        assert!(matches!(
            result,
            Err(StreamError::Query(QueryError::Source))
        ));
        assert_eq!(emissions, 0);
        assert!(!source.eof.get());
    }

    let source = Generated::new(3);
    let syntax = parse_query(
        "SELECT Count(*) FROM T WHERE N<2 OR 1/(N-2)>0",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let mut emissions = 0;
    let result = plan.execute_progressive(&source, &[], Default::default(), &mut |_| {
        emissions += 1;
        Ok::<_, ()>(())
    });
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
    assert_eq!(emissions, 0);
    assert_eq!(source.visits.get(), 2);
}

#[test]
fn swallowed_visitor_limit_errors_stay_latched_before_any_final_count() {
    let mut source = Generated::new(10);
    source.swallow = true;
    source.fail_eof = true;
    let syntax = parse_query("SELECT Count(*) FROM T", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let options = ExecutionOptions {
        limits: QueryLimits {
            rows: 4,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut emissions = 0;
    let result = plan.execute_progressive(&source, &[], options, &mut |_| {
        emissions += 1;
        Ok::<_, ()>(())
    });
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Limit(_)))
    ));
    assert_eq!(emissions, 0);
    assert_eq!(source.visits.get(), 10);
}

#[test]
fn final_sink_failure_is_after_cursor_release_and_stops_the_next_arm() {
    let mut source = Generated::new(3);
    source.swallow = true;
    let syntax = parse_query(
        "SELECT Count(*) FROM T UNION ALL SELECT Count(*) FROM T",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let mut calls = 0;
    let result = plan.execute_progressive(&source, &[], Default::default(), &mut |_| {
        assert!(!source.active.get());
        assert!(source.eof.get());
        calls += 1;
        Err("final sink failure")
    });
    assert!(matches!(
        result,
        Err(StreamError::Sink("final sink failure"))
    ));
    assert_eq!(calls, 1);
    assert_eq!(source.projections.borrow().len(), 1);
    assert_eq!(source.visits.get(), 3);
}

#[test]
fn ineligible_aggregate_shapes_are_refused_before_any_cursor() {
    let source = Generated::new(0);
    for sql in [
        "SELECT First(N) FROM T",
        "SELECT Last(N) FROM T",
        "SELECT Count(*) FROM T GROUP BY CDbl(N)",
        "SELECT Count(*) FROM T HAVING Last(N)>0",
        "SELECT DISTINCT CDbl(Count(*)) FROM T",
        "SELECT Count(*) FROM T ORDER BY 1",
        "SELECT Count(*),(SELECT 1) FROM T",
        "SELECT N FROM T UNION ALL SELECT First(N) FROM T",
    ] {
        let syntax = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(syntax.query(), &source).unwrap();
        let mut emissions = 0;
        let result = plan.execute_progressive(&source, &[], Default::default(), &mut |_| {
            emissions += 1;
            Ok::<_, ()>(())
        });
        assert!(
            matches!(result, Err(StreamError::Query(QueryError::Unsupported))),
            "{sql}"
        );
        assert_eq!(emissions, 0, "{sql}");
        assert!(source.projections.borrow().is_empty(), "{sql}");
    }
}

#[test]
fn zero_top_preserves_the_existing_binding_refusal_without_scanning() {
    let source = Generated::new(3);
    let syntax = parse_query("SELECT TOP 0 Count(*),1/0 FROM T", Default::default()).unwrap();
    assert!(matches!(
        prepare(syntax.query(), &source),
        Err(QueryError::Unsupported)
    ));
    assert!(source.projections.borrow().is_empty());
    assert_eq!(source.buffered_calls.get(), 0);
}

#[test]
fn every_count_checkpoint_and_post_eof_checkpoint_can_cancel_without_output() {
    let source = Generated::new(5);
    let syntax = parse_query(
        "SELECT Count(*) AS Population,Population+CLngLng(1) FROM T WHERE N>0",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
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

    source.eof.set(false);
    let cancel_after_eof = || source.eof.get();
    let mut emissions = 0;
    let result = plan.execute_progressive(
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&cancel_after_eof),
            ..Default::default()
        },
        &mut |_| {
            emissions += 1;
            Ok::<_, ()>(())
        },
    );
    assert!(matches!(
        result,
        Err(StreamError::Query(
            QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled)
        ))
    ));
    assert_eq!(emissions, 0);
}

#[test]
fn cumulative_rows_bytes_and_steps_are_not_refilled_during_counting() {
    let syntax = parse_query("SELECT Count(*) FROM T WHERE N>0", Default::default()).unwrap();
    let one = Generated::new(1);
    let plan = prepare(syntax.query(), &one).unwrap();
    for limits in [
        QueryLimits {
            rows: 4,
            ..Default::default()
        },
        QueryLimits {
            bytes: 4096,
            ..Default::default()
        },
        QueryLimits {
            steps: 100,
            ..Default::default()
        },
    ] {
        let options = ExecutionOptions {
            limits,
            ..Default::default()
        };
        plan.execute_progressive(&one, &[], options, &mut |_| Ok::<_, ()>(()))
            .unwrap();

        let many = Generated::new(1000);
        let mut emissions = 0;
        let result = plan.execute_progressive(&many, &[], options, &mut |_| {
            emissions += 1;
            Ok::<_, ()>(())
        });
        assert!(matches!(
            result,
            Err(StreamError::Query(
                QueryError::Limit(_) | QueryError::Evaluation(EvalError::Limit(_))
            ))
        ));
        assert_eq!(emissions, 0);
    }
}
