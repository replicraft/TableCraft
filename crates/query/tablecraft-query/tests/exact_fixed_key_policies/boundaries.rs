use super::*;

#[test]
fn currency_cursor_eof_and_swallowed_operand_failures_preserve_first_typed_error() {
    let mut source = Data::new(
        ScalarType::Currency,
        vec![
            (Value::Currency(Currency::from_scaled(1)), 2, "a"),
            (Value::Currency(Currency::from_scaled(2)), 3, "b"),
        ],
    );
    source.fail_eof = true;
    assert!(matches!(
        stream(
            &source,
            "SELECT D,Count(*) FROM T GROUP BY D",
            options(false)
        ),
        Err(StreamError::Query(QueryError::Source))
    ));
    source.swallow = true;
    let error = stream(
        &source,
        "SELECT D,Sum(1/(N-3)) FROM T GROUP BY D",
        options(false),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        StreamError::Query(QueryError::Evaluation(EvalError::DivisionByZero))
    ));
    assert_eq!(source.buffered_scans.get(), 0);
}

#[test]
fn fixed_key_growth_and_finalization_observe_every_cancel_checkpoint_and_all_quotas() {
    let source = Data::decimal_fixture();
    let syntax = parse_query(
        "SELECT D,Count(*) FROM T GROUP BY D HAVING Count(*)>0",
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
            ..options(false)
        },
        &mut |_| Ok::<_, ()>(()),
    )
    .unwrap();
    assert!(observed.get() > 0);
    for stop in 0..observed.get() {
        let calls = Cell::new(0);
        let cancel = || {
            let current = calls.get();
            calls.set(current + 1);
            current == stop
        };
        let error = plan
            .execute_progressive(
                &source,
                &[],
                ExecutionOptions {
                    cancelled: Some(&cancel),
                    ..options(false)
                },
                &mut |_| {
                    assert!(source.eof.get());
                    Ok::<_, ()>(())
                },
            )
            .unwrap_err();
        assert!(
            matches!(
                error,
                StreamError::Query(
                    QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled)
                )
            ),
            "checkpoint {stop}"
        );
        assert!(!source.active.get());
        assert_eq!(source.buffered_scans.get(), 0);
    }
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
        let result = plan.execute_progressive(
            &source,
            &[],
            ExecutionOptions {
                limits,
                ..options(false)
            },
            &mut |_| Ok::<_, ()>(()),
        );
        assert!(matches!(
            result,
            Err(StreamError::Query(QueryError::Limit(_)))
        ));
        assert!(!source.active.get());
        assert_eq!(source.buffered_scans.get(), 0);
    }
    assert_eq!(
        stream(
            &source,
            "SELECT D,Count(*) FROM T GROUP BY D",
            options(false)
        )
        .unwrap(),
        [
            vec![Value::Null, Value::Int64(2)],
            vec![decimal(100, 2), Value::Int64(2)]
        ]
    );
}

#[test]
fn fixed_key_group_sink_failure_stops_before_later_projection_failure() {
    let source = Data::new(
        ScalarType::Decimal,
        vec![(decimal(1, 0), 1, "a"), (decimal(2, 0), 2, "b")],
    );
    let syntax = parse_query(
        "SELECT D,1/(D-CDec(2)) FROM T GROUP BY D",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let mut calls = 0;
    let error = plan
        .execute_progressive(&source, &[], options(false), &mut |_| {
            assert!(source.eof.get());
            calls += 1;
            Err("fixed-key sink")
        })
        .unwrap_err();
    assert!(matches!(error, StreamError::Sink("fixed-key sink")));
    assert_eq!(calls, 1);
    assert_eq!(source.buffered_scans.get(), 0);
}
