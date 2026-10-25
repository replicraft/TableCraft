use super::*;

#[test]
fn empty_inputs_skip_argument_evaluation_but_populated_inputs_keep_eager_errors() {
    for (argument, error) in [
        ("1/0", EvalError::DivisionByZero),
        ("IIf(True,NULL,ChrW(65536))", EvalError::Domain),
        ("IIf(True,NULL,1/(N-2))", EvalError::DivisionByZero),
        ("CStr(N)", EvalError::NumberFormatRequired),
    ] {
        let sql = format!("SELECT Count({argument}) FROM T");
        let empty = Generated::new(0);
        assert_eq!(
            rows(&sql, &empty, Default::default()),
            [vec![Value::Int64(0)]]
        );

        let filtered = Generated::new(3);
        let filtered_sql = format!("{sql} WHERE False");
        let filtered_rows = rows(&filtered_sql, &filtered, Default::default());
        assert_eq!(filtered_rows, [vec![Value::Int64(0)]]);
        assert_eq!(filtered.visits.get(), 3);

        let source = Generated::new(3);
        let syntax = parse_query(&sql, Default::default()).unwrap();
        let plan = prepare(syntax.query(), &source).unwrap();
        let mut emissions = 0;
        let result = plan.execute_progressive(&source, &[], Default::default(), &mut |_| {
            emissions += 1;
            Ok::<_, ()>(())
        });
        assert!(
            matches!(result, Err(StreamError::Query(QueryError::Evaluation(actual))) if actual == error),
            "{sql}"
        );
        assert_eq!(emissions, 0, "{sql}");
        assert!(!source.eof.get(), "{sql}");
    }
}

#[test]
fn argument_errors_remain_sticky_when_a_provider_swallows_visitors_and_fails_eof() {
    let mut source = Generated::new(5);
    source.swallow = true;
    source.fail_eof = true;
    let syntax = parse_query(
        "SELECT Count(N),Count(1/(N-2)),Count(*) FROM T",
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
    assert_eq!(source.visits.get(), 5);
}

#[test]
fn overflow_is_checked_after_completed_counts_and_sink_errors_stop_later_arms() {
    let source = Generated::new(32_768);
    let syntax = parse_query("SELECT CInt(Count(N)) FROM T", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let mut emissions = 0;
    let options = ExecutionOptions {
        limits: QueryLimits {
            steps: 5_000_000,
            ..Default::default()
        },
        ..Default::default()
    };
    let result = plan.execute_progressive(&source, &[], options, &mut |_| {
        emissions += 1;
        Ok::<_, ()>(())
    });
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::Overflow
        )))
    ));
    assert!(source.eof.get());
    assert_eq!(emissions, 0);

    let source = Generated::new(3);
    let syntax = parse_query(
        "SELECT Count(A) FROM T UNION ALL SELECT Count(N) FROM T",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let mut calls = 0;
    let result = plan.execute_progressive(&source, &[], Default::default(), &mut |_| {
        assert!(source.eof.get());
        assert!(!source.active.get());
        calls += 1;
        Err("first sink failure")
    });
    assert!(matches!(
        result,
        Err(StreamError::Sink("first sink failure"))
    ));
    assert_eq!(calls, 1);
    assert_eq!(source.projections.borrow().len(), 1);
}

#[test]
fn every_argument_count_checkpoint_can_cancel_and_a_later_run_is_independent() {
    let source = Generated::new(5);
    let sql = "SELECT Count(A),Count(ChrW(55296)),Count(*) FROM T WHERE N>0";
    let syntax = parse_query(sql, Default::default()).unwrap();
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
    assert_eq!(
        rows(sql, &source, Default::default()),
        [vec![Value::Int64(3), Value::Int64(5), Value::Int64(5)]]
    );
}

#[test]
fn cumulative_operand_allocation_row_and_work_limits_prevent_final_output() {
    let one = Generated::new(1);
    let syntax = parse_query(
        "SELECT Count(N),Count(ChrW(55296)) FROM T",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &one).unwrap();
    for limits in [
        QueryLimits {
            bytes: 4096,
            ..Default::default()
        },
        QueryLimits {
            rows: 4,
            ..Default::default()
        },
        QueryLimits {
            steps: 150,
            ..Default::default()
        },
    ] {
        let options = ExecutionOptions {
            limits,
            ..Default::default()
        };
        plan.execute_progressive(&one, &[], options, &mut |_| Ok::<_, ()>(()))
            .unwrap();

        let source = Generated::new(1000);
        let mut emissions = 0;
        let result = plan.execute_progressive(&source, &[], options, &mut |_| {
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

#[test]
fn operands_cannot_capture_projection_aliases_or_activate_unsupported_shapes() {
    let source = Generated::new(0);
    for sql in [
        "SELECT Count(N) AS Population,Count(Population) FROM T",
        "SELECT Count(Count(N)) FROM T",
    ] {
        let syntax = parse_query(sql, Default::default()).unwrap();
        assert!(prepare(syntax.query(), &source).is_err(), "{sql}");
        assert!(source.projections.borrow().is_empty(), "{sql}");
    }
    for sql in [
        "SELECT Count(Rnd()) FROM T",
        "SELECT Count(Now()) FROM T",
        "SELECT Count((SELECT 1)) FROM T",
        "SELECT Count(DCount('*','T')) FROM T",
    ] {
        let syntax = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(syntax.query(), &source).unwrap();
        let result =
            plan.execute_progressive(&source, &[], Default::default(), &mut |_| Ok::<_, ()>(()));
        assert!(
            matches!(result, Err(StreamError::Query(QueryError::Unsupported))),
            "{sql}"
        );
        assert!(source.projections.borrow().is_empty(), "{sql}");
    }
}
