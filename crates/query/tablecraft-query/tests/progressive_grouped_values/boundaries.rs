use super::*;

#[test]
fn unsupported_shapes_and_later_key_families_refuse_before_every_cursor() {
    for sql in [
        "SELECT DISTINCT Id,Count(*) FROM T GROUP BY Id",
        "SELECT Id,Count(*) FROM T GROUP BY Id ORDER BY Id",
        "SELECT Id,Count(*) FROM T GROUP BY Id HAVING Last(Id)>0",
        "SELECT TOP 50 PERCENT Id,Count(*) FROM T GROUP BY Id",
        "SELECT CStr(Id),Count(*) FROM T GROUP BY CStr(Id)",
        "SELECT CDbl(Id),Count(*) FROM T GROUP BY CDbl(Id)",
        "SELECT CSng(Id),Count(*) FROM T GROUP BY CSng(Id)",
        "SELECT ChrW(Id),Count(*) FROM T GROUP BY ChrW(Id)",
        "SELECT Id,Count(*) FROM T GROUP BY Id UNION ALL SELECT CStr(Id),Count(*) FROM Later GROUP BY CStr(Id)",
    ] {
        let source = Inputs::generated(1, 0);
        let result = stream(sql, &source, Default::default());
        assert!(
            matches!(result, Err(StreamError::Query(QueryError::Unsupported))),
            "{sql}"
        );
        assert!(source.scans.borrow().is_empty());
        assert_eq!(source.buffered_calls.get(), 0);
    }
}

#[test]
fn active_profile_admission_uses_proven_types_without_guessing_from_small_actual_values() {
    let sql = "SELECT Id+Id,Count(*) FROM T GROUP BY Id+Id";
    let source = Inputs::generated(1, 3);
    assert!(matches!(
        stream(sql, &source, Default::default()),
        Err(StreamError::Query(QueryError::Unsupported))
    ));
    assert!(source.scans.borrow().is_empty());
    let result = stream(
        sql,
        &source,
        ExecutionOptions {
            declared_integral: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        result,
        [2, 4, 6]
            .into_iter()
            .map(|key| vec![Value::Int32(key), Value::Int64(1)])
            .collect::<Vec<_>>()
    );
}

#[test]
fn complete_eof_and_first_callback_failure_precede_any_group_publication_or_later_arm() {
    for swallow in [false, true] {
        let mut source = Inputs::generated(1, 5);
        source.fail_eof = true;
        source.swallow = swallow;
        source.corrupt_after = Some(1);
        let sql = "SELECT CLng(Id Mod 2),Sum(1/(Id-1)) FROM T GROUP BY Id Mod 2
                   UNION ALL SELECT Id,CDbl(Id) FROM Later";
        let query = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(query.query(), &source).unwrap();
        let calls = Cell::new(0);
        let result = plan.execute_progressive(&source, &[], Default::default(), &mut |_| {
            calls.set(calls.get() + 1);
            Ok::<_, ()>(())
        });
        assert!(matches!(
            result,
            Err(StreamError::Query(QueryError::Evaluation(
                EvalError::DivisionByZero
            )))
        ));
        assert_eq!(calls.get(), 0);
        assert_eq!(*source.scans.borrow(), ["T"]);
    }

    let mut source = Inputs::generated(1, 5);
    source.fail_eof = true;
    let query = parse_query(
        "SELECT Count(*) FROM Later UNION ALL SELECT Count(*) FROM T GROUP BY G",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    let mut accepted = Vec::new();
    let result = plan.execute_progressive(&source, &[], Default::default(), &mut |row| {
        assert!(source.eof.get());
        accepted.push(row.to_vec());
        Ok::<_, ()>(())
    });
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Source))
    ));
    assert_eq!(accepted, [vec![Value::Int64(2)]]);
    assert_eq!(*source.scans.borrow(), ["Later", "T"]);
}

#[test]
fn sink_failure_stops_later_groups_and_arms_and_top_does_not_hide_final_errors() {
    let source = Inputs::generated(1, 6);
    let sql = "SELECT CLng(Id Mod 2),Count(*) FROM T GROUP BY Id Mod 2
               UNION ALL SELECT Id,1 FROM Later";
    let query = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    let mut accepted = Vec::new();
    let result = plan.execute_progressive(&source, &[], Default::default(), &mut |row| {
        assert!(source.eof.get());
        assert!(!source.active.get());
        if !accepted.is_empty() {
            return Err("original sink error");
        }
        accepted.push(row.to_vec());
        Ok(())
    });
    assert!(matches!(
        result,
        Err(StreamError::Sink("original sink error"))
    ));
    assert_eq!(accepted, [vec![Value::Int32(1), Value::Int64(3)]]);
    assert_eq!(*source.scans.borrow(), ["T"]);

    let source = Inputs::generated(1, 2);
    let query = parse_query(
        "SELECT TOP 1 CLng(Id Mod 2),1/(Id Mod 2) FROM T GROUP BY Id Mod 2",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    let mut accepted = Vec::new();
    let result = plan.execute_progressive(&source, &[], Default::default(), &mut |row| {
        assert!(source.eof.get());
        accepted.push(row.to_vec());
        Ok::<_, ()>(())
    });
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
    assert_eq!(
        accepted,
        [vec![
            Value::Int32(1),
            Value::Double(Double::from_float(1.0))
        ]]
    );
}

#[test]
fn query_limits_all_observed_cancellation_points_and_retry_keep_no_snapshot_cache() {
    let source = Inputs::generated(1, 9);
    let query = parse_query(
        "SELECT CLng(Id Mod 3),Count(*),Sum(Score) FROM T GROUP BY Id Mod 3",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    let checkpoints = Cell::new(0);
    let count = || {
        checkpoints.set(checkpoints.get() + 1);
        false
    };
    let mut expected = Vec::new();
    plan.execute_progressive(
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&count),
            ..Default::default()
        },
        &mut |row| {
            expected.push(row.to_vec());
            Ok::<_, ()>(())
        },
    )
    .unwrap();
    for stop in 1..=checkpoints.get() {
        let calls = Cell::new(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let result = plan.execute_progressive(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancelled),
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
            "checkpoint {stop}: {result:?}"
        );
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
                ..Default::default()
            },
            &mut |_| Ok::<_, ()>(()),
        );
        assert!(matches!(
            result,
            Err(StreamError::Query(QueryError::Limit(_)))
        ));
    }
    let mut retry = Vec::new();
    plan.execute_progressive(&source, &[], Default::default(), &mut |row| {
        retry.push(row.to_vec());
        Ok::<_, ()>(())
    })
    .unwrap();
    assert_eq!(retry, expected);
    assert_eq!(source.buffered_calls.get(), 0);

    let many = Inputs::generated(1, 1000);
    let result = stream(
        "SELECT Id,Count(*) FROM T GROUP BY Id",
        &many,
        ExecutionOptions {
            limits: QueryLimits {
                bytes: 64 * 1024,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Limit(_)))
    ));
    assert!(!many.eof.get());
    assert_eq!(many.buffered_calls.get(), 0);
}
