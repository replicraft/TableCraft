use super::*;

#[test]
fn having_scalar_branches_remain_eager_after_eof_and_operands_fail_during_scan() {
    for predicate in [
        "False AND (1/0>0)",
        "True OR (1/0>0)",
        "IIf(True,True,1/0>0)",
    ] {
        let source = Data::fixture();
        let sql = format!("SELECT Count(*) FROM T HAVING {predicate}");
        assert!(matches!(
            stream(&source, &sql, options(false)),
            Err(StreamError::Query(QueryError::Evaluation(
                EvalError::DivisionByZero
            )))
        ));
        assert!(source.eof.get(), "{sql}");
    }
    let source = Data::fixture();
    let sql = "SELECT Count(*) FROM T HAVING False AND Count(1/0)>0";
    assert!(matches!(
        stream(&source, sql, options(false)),
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
    assert!(
        !source.eof.get(),
        "HAVING aggregate operands use the scan phase"
    );
}

#[test]
fn top_counts_only_accepted_groups_but_does_not_suppress_later_accepted_errors() {
    let source = Data::fixture();
    let accepted = "SELECT TOP 1 K,Count(*) FROM T GROUP BY K HAVING K<>2 AND K IS NOT NULL";
    assert_eq!(
        stream(&source, accepted, options(false)).unwrap(),
        [vec![Value::Int32(1), Value::Int64(2)]]
    );
    let rejected = "SELECT TOP 1 K,Count(*),1/(K-3) FROM T GROUP BY K HAVING K=1";
    assert_eq!(
        stream(&source, rejected, options(false)).unwrap(),
        [vec![
            Value::Int32(1),
            Value::Int64(2),
            Value::Double(Double::from_float(-0.5)),
        ]]
    );
    let failing = "SELECT TOP 1 K,Count(*),1/(K-3) FROM T GROUP BY K HAVING K<>2 AND K IS NOT NULL";
    let syntax = parse_query(failing, Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let mut prefix = Vec::new();
    let error = plan
        .execute_progressive(&source, &[], options(false), &mut |row| {
            assert!(source.eof.get());
            prefix.push(row.to_vec());
            Ok::<_, ()>(())
        })
        .unwrap_err();
    assert_eq!(source.buffered_scans.get(), 0);
    assert!(matches!(
        error,
        StreamError::Query(QueryError::Evaluation(EvalError::DivisionByZero))
    ));
    assert_eq!(
        prefix,
        [vec![
            Value::Int32(1),
            Value::Int64(2),
            Value::Double(Double::from_float(-0.5)),
        ]]
    );
}

#[test]
fn eof_failure_wins_before_having_and_sink_error_stops_later_groups() {
    let mut source = Data::fixture();
    source.fail_eof = true;
    let failing = "SELECT Sum(1/0) FROM T HAVING 1/0>0";
    assert!(matches!(
        stream(&source, failing, options(false)),
        Err(StreamError::Query(QueryError::Source))
    ));

    let source = Data::fixture();
    let sql = "SELECT K,Count(*),1/(K-3) FROM T GROUP BY K HAVING K<>2 AND K IS NOT NULL";
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let mut calls = 0;
    let error = plan
        .execute_progressive(&source, &[], options(false), &mut |_| {
            assert!(source.eof.get());
            calls += 1;
            Err("first sink failure")
        })
        .unwrap_err();
    assert_eq!(source.buffered_scans.get(), 0);
    assert!(matches!(error, StreamError::Sink("first sink failure")));
    assert_eq!(calls, 1);
}

#[test]
fn repeated_prepared_execution_and_union_arms_start_with_fresh_having_states() {
    let source = Data::fixture();
    let sql = "SELECT Sum(CLng(1/0)) AS V FROM T HAVING False \
               UNION ALL SELECT Count(*) FROM T HAVING Count(*)=6";
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    for declared in [false, true, false] {
        let mut rows = Vec::new();
        let summary = plan
            .execute_progressive(&source, &[], options(declared), &mut |row| {
                assert!(source.eof.get());
                rows.push(row.to_vec());
                Ok::<_, ()>(())
            })
            .unwrap();
        // Existing integral Sum and Count descriptors preserve the Int64 arm.
        assert_eq!(rows, [vec![Value::Int64(6)]]);
        assert_eq!(source.buffered_scans.get(), 0);
        assert_eq!(summary.scanned_rows, 12);
        assert_eq!(summary.emitted_rows, 1);
    }
}

#[test]
fn every_observed_cancellation_checkpoint_keeps_replay_and_final_states_local() {
    let source = Data::fixture();
    let syntax = parse_query("SELECT Sum(N) FROM T HAVING Count(*)>0", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let observed = Cell::new(0);
    let observe = || {
        observed.set(observed.get() + 1);
        false
    };
    let mut baseline = Vec::new();
    plan.execute_progressive(
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&observe),
            ..options(false)
        },
        &mut |row| {
            baseline.push(row.to_vec());
            Ok::<_, ()>(())
        },
    )
    .unwrap();
    assert_eq!(source.buffered_scans.get(), 0);
    assert_eq!(baseline, [vec![Value::Int64(20)]]);
    assert!(observed.get() > 0);

    for stop in 0..observed.get() {
        let calls = Cell::new(0);
        let cancel = || {
            let position = calls.get();
            calls.set(position + 1);
            position == stop
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
}
