use super::*;

#[test]
fn full_partition_error_precedes_selected_aggregate_error_and_every_output() {
    let mut source = Data::new();
    source.policies[3] = Some(TextCollationId::from_host(4));
    source.rows[2][3] = Value::Text("bad".into());
    let resolver = Resolver::new(&source);
    let sql = format!("SELECT {KEY},Sum(1/0) FROM T GROUP BY {KEY}");
    let error = stream(&source, &sql, options(&resolver, false)).unwrap_err();
    assert!(matches!(
        error,
        StreamError::Query(QueryError::Evaluation(EvalError::Domain))
    ));
    assert!(source.eof.get());
    assert!(
        resolver
            .trace
            .borrow()
            .iter()
            .any(|event| matches!(event, Event::Compare(4)))
    );
}

#[test]
fn eof_failure_prevents_partition_but_swallowed_immediate_key_failure_stays_first() {
    let mut source = Data::new();
    source.fail_eof = true;
    let resolver = Resolver::new(&source);
    let sql = format!("SELECT {KEY},Sum(1/0) FROM T GROUP BY {KEY}");
    assert!(matches!(
        stream(&source, &sql, options(&resolver, false)),
        Err(StreamError::Query(QueryError::Source))
    ));
    assert!(resolver.trace.borrow().is_empty());

    source.swallow = true;
    let resolver = Resolver::new(&source);
    let key = "CStr(1/(Id-2))";
    let sql = format!("SELECT Count(*) FROM T GROUP BY {key}");
    assert!(matches!(
        stream(&source, &sql, options(&resolver, false)),
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
    assert!(resolver.trace.borrow().is_empty());
}

#[test]
fn original_sink_error_stops_later_union_arms_after_partition_and_eof() {
    let source = Data::new();
    let resolver = Resolver::new(&source);
    let sql = format!(
        "SELECT {KEY} AS K,Count(*) FROM T GROUP BY {KEY} \
         UNION ALL SELECT {KEY},Sum(1/0) FROM T GROUP BY {KEY}"
    );
    let syntax = parse_query(&sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let mut calls = 0;
    let error = plan
        .execute_progressive(&source, &[], options(&resolver, false), &mut |_| {
            assert!(source.eof.get());
            calls += 1;
            Err("text group sink")
        })
        .unwrap_err();
    assert!(matches!(error, StreamError::Sink("text group sink")));
    assert_eq!(calls, 1);
    assert_eq!(source.buffered_scans.get(), 0);
}

#[test]
fn policy_capture_partition_replay_and_having_observe_every_cancel_and_tiny_quota() {
    let source = Data::new();
    let resolver = Resolver::new(&source);
    let sql = format!("SELECT {KEY},Count(*),Min(C) FROM T GROUP BY {KEY} HAVING Count(*)>0");
    let syntax = parse_query(&sql, Default::default()).unwrap();
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
            ..options(&resolver, false)
        },
        &mut |_| Ok::<_, ()>(()),
    )
    .unwrap();
    assert!(observed.get() > 0);
    for stop in 0..observed.get() {
        let current = Cell::new(0);
        let cancel = || {
            let index = current.get();
            current.set(index + 1);
            index == stop
        };
        let result = plan.execute_progressive(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancel),
                ..options(&resolver, false)
            },
            &mut |_| {
                assert!(source.eof.get());
                Ok::<_, ()>(())
            },
        );
        assert!(
            matches!(
                result,
                Err(StreamError::Query(
                    QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled)
                ))
            ),
            "checkpoint {stop}: {result:?}"
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
                ..options(&resolver, false)
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
    let actual = stream(&source, &sql, options(&resolver, false)).unwrap();
    assert_eq!(
        actual,
        [vec![
            Value::Text("10".into()),
            Value::Int64(3),
            Value::Text("yz".into())
        ]]
    );
}
