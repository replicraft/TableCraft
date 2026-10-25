use super::*;

#[test]
fn physical_eof_and_partial_inner_rows_do_not_finish_any_downstream_stateful_stage() {
    let capped = FAILING_INNER.replacen("(SELECT G", "(SELECT TOP 1 G", 1);

    for inner in [FAILING_INNER.to_owned(), capped] {
        let host = Host::new();
        let syntax = parse_query(&format!("SELECT G FROM {inner}"), Default::default()).unwrap();
        let plan = prepare(syntax.query(), &host).unwrap();
        let mut prefix = Vec::new();
        let error = plan
            .execute_progressive(&host, &[], options(false), &mut |row| {
                assert!(host.root.eof.get());
                prefix.push(row.to_vec());
                Ok::<_, ()>(())
            })
            .unwrap_err();
        assert!(matches!(
            error,
            StreamError::Query(QueryError::Evaluation(EvalError::DivisionByZero))
        ));
        assert_eq!(prefix, [vec![Value::Int32(1)]]);

        for (projection, suffix) in [
            ("SELECT Count(*) FROM", ""),
            ("SELECT Sum(N) FROM", ""),
            ("SELECT G,Count(*) FROM", " GROUP BY G"),
            ("SELECT CStr(G),Count(*) FROM", " GROUP BY CStr(G)"),
            ("SELECT Count(*) FROM", " HAVING False"),
            ("SELECT Min(S)='x' FROM", ""),
        ] {
            let host = Host::new();
            let resolver = Resolver {
                host: &host,
                hits: Cell::new(0),
            };
            let sql = format!("{projection} {inner}{suffix}");
            let syntax = parse_query(&sql, Default::default()).unwrap();
            let plan = prepare(syntax.query(), &host).unwrap();
            let mut calls = 0;
            let error = plan
                .execute_progressive(
                    &host,
                    &[],
                    ExecutionOptions {
                        collation_resolver: Some(&resolver),
                        ..options(false)
                    },
                    &mut |_| {
                        calls += 1;
                        Ok::<_, ()>(())
                    },
                )
                .unwrap_err();
            assert!(
                matches!(
                    error,
                    StreamError::Query(QueryError::Evaluation(EvalError::DivisionByZero))
                ),
                "{sql}: {error:?}"
            );
            assert!(host.root.eof.get());
            assert_eq!(calls, 0, "{sql}");
            assert_eq!(host.root.scans.get(), 1);
            assert_eq!(
                resolver.hits.get(),
                0,
                "outer final provider must wait for logical EOF"
            );
            host.assert_no_snapshot();
        }
    }
}

#[test]
fn outer_rejection_cannot_suppress_upstream_work_but_inner_having_can() {
    let host = Host::new();
    let sql = format!("SELECT Count(*) FROM {FAILING_INNER} WHERE False HAVING False");
    assert!(matches!(
        stream(&host, &sql, options(false)),
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
    let sql = "SELECT Count(*) FROM (SELECT Sum(1/0) AS N FROM T HAVING False) Q";
    assert_eq!(
        stream(&host, sql, options(false)).unwrap(),
        [vec![Value::Int64(0)]]
    );
    let sql = "SELECT Count(*) FROM (SELECT Count(*) AS N FROM T WHERE False) Q WHERE False";
    assert_eq!(
        stream(&host, sql, options(false)).unwrap(),
        [vec![Value::Int64(0)]]
    );
}

#[test]
fn final_sink_stops_pending_inner_finish_work_and_later_root_arms() {
    let host = Host::new();
    let sql = format!("SELECT G FROM {FAILING_INNER} UNION ALL SELECT M FROM [two].Nested");
    let syntax = parse_query(&sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), &host).unwrap();
    let mut calls = 0;
    let error = plan
        .execute_progressive(&host, &[], options(false), &mut |_| {
            assert!(host.root.eof.get());
            calls += 1;
            Err("stateful sink")
        })
        .unwrap_err();
    assert!(matches!(error, StreamError::Sink("stateful sink")));
    assert_eq!(calls, 1);
    assert_eq!(host.root.scans.get(), 1);
    assert_eq!(host.two.scans.get(), 0);
    host.assert_no_snapshot();
}

#[test]
fn late_source_error_blocks_inner_finish_and_swallowed_operand_error_stays_first() {
    let mut host = Host::new();
    host.root.fail_eof = true;
    let resolver = Resolver {
        host: &host,
        hits: Cell::new(0),
    };
    let sql = "SELECT Min(S)='x' FROM (SELECT G,Min(A) AS S FROM T GROUP BY G) Q";
    assert!(matches!(
        stream(
            &host,
            sql,
            ExecutionOptions {
                collation_resolver: Some(&resolver),
                ..options(false)
            }
        ),
        Err(StreamError::Query(QueryError::Source))
    ));
    assert_eq!(resolver.hits.get(), 0);

    host.root.swallow = true;
    let sql = "SELECT Count(*) FROM (SELECT Sum(1/0) AS S FROM T) Q";
    assert!(matches!(
        stream(&host, sql, options(false)),
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
}

#[test]
fn every_checkpoint_and_cumulative_quota_keeps_stage_states_scoped_to_one_run() {
    let host = Host::new();
    let sql = "SELECT Sum(N) FROM (SELECT G,Count(*) AS N FROM T GROUP BY G) Q";
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), &host).unwrap();
    let observed = Cell::new(0);
    let observe = || {
        observed.set(observed.get() + 1);
        false
    };
    let summary = plan
        .execute_progressive(
            &host,
            &[],
            ExecutionOptions {
                cancelled: Some(&observe),
                ..options(false)
            },
            &mut |row| {
                assert_eq!(row, [Value::Int64(2)]);
                Ok::<_, ()>(())
            },
        )
        .unwrap();
    assert_eq!(summary.scanned_rows, 2);
    assert_eq!(summary.emitted_rows, 1);
    assert!(observed.get() > 0);
    for stop in 0..observed.get() {
        let calls = Cell::new(0);
        let cancel = || {
            let index = calls.get();
            calls.set(index + 1);
            index == stop
        };
        let result = plan.execute_progressive(
            &host,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancel),
                ..options(false)
            },
            &mut |_| {
                assert!(host.root.eof.get());
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
        assert!(!host.active.get());
        host.assert_no_snapshot();
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
            &host,
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
        assert!(!host.active.get());
        host.assert_no_snapshot();
    }
    assert_eq!(
        stream(&host, sql, options(false)).unwrap(),
        [vec![Value::Int64(2)]]
    );
}
