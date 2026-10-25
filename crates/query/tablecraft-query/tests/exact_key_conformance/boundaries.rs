use super::*;

#[test]
fn error_and_raw_string_keys_stay_on_their_existing_checked_paths() {
    let source = MemorySource::default();
    let relation = "(SELECT CVErr(5) AS K UNION ALL SELECT CVErr(0) UNION ALL SELECT CVErr(5)) Q";
    let sql = format!("SELECT CLng(K),Count(*) FROM {relation} GROUP BY K");
    let actual = run(&sql, &source, Default::default()).unwrap();
    assert_eq!(
        actual,
        [
            vec![Value::Int32(5), Value::Int64(2)],
            vec![Value::Int32(0), Value::Int64(1)]
        ]
    );

    for sql in [
        "SELECT Count(*) FROM (SELECT CVErr(0) AS K UNION ALL SELECT NULL) Q GROUP BY K",
        "SELECT DISTINCT K FROM (SELECT CVErr(0) AS K UNION ALL SELECT CLng(0)) Q",
    ] {
        assert_eq!(
            run(sql, &source, Default::default()),
            Err(QueryError::Evaluation(EvalError::TypeMismatch)),
            "{sql}"
        );
    }
    let sql = "SELECT Count(*) FROM (SELECT ChrB(65) AS K UNION SELECT ChrB(65)) Q";
    assert_eq!(
        run(sql, &source, Default::default()),
        Err(QueryError::Evaluation(EvalError::UnrepresentableText))
    );
}

#[test]
fn every_key_operation_checkpoint_can_cancel_without_publishing_and_later_runs_recover() {
    let source = Unique::new(9);
    for sql in [
        "SELECT K,Count(*) FROM T GROUP BY K",
        "SELECT DISTINCT K FROM T",
    ] {
        let observed = Cell::new(0);
        let observe = || {
            observed.set(observed.get() + 1);
            false
        };
        let expected = run(
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
            let actual = run(
                sql,
                &source,
                ExecutionOptions {
                    cancelled: Some(&cancel),
                    ..Default::default()
                },
            );
            assert!(
                matches!(
                    actual,
                    Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
                ),
                "{sql}, checkpoint {stop}"
            );
        }
        assert_eq!(run(sql, &source, Default::default()).unwrap(), expected);
    }
}

#[test]
fn cumulative_limits_and_late_operand_failures_remain_visible() {
    let source = Unique::new(30);
    for sql in [
        "SELECT K,Count(*) FROM T GROUP BY K",
        "SELECT DISTINCT K FROM T",
    ] {
        assert!(!run(sql, &source, Default::default()).unwrap().is_empty());
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
            let actual = run(
                sql,
                &source,
                ExecutionOptions {
                    limits,
                    ..Default::default()
                },
            );
            assert!(matches!(actual, Err(QueryError::Limit(_))), "{sql}");
        }
    }

    for sql in [
        "SELECT K,Sum(1/(K-1)) FROM T GROUP BY K",
        "SELECT DISTINCT K,1/(K-1) FROM T",
    ] {
        let actual = run(sql, &source, Default::default());
        assert_eq!(
            actual,
            Err(QueryError::Evaluation(EvalError::DivisionByZero)),
            "{sql}"
        );
    }
}
