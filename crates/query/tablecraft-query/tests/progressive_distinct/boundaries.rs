use super::*;

#[test]
fn unsupported_profiles_and_inner_distinct_refuse_before_any_cursor() {
    for kind in [
        ScalarType::Text,
        ScalarType::Single,
        ScalarType::Double,
        ScalarType::DateTime,
        ScalarType::Guid,
        ScalarType::Binary,
    ] {
        let source = Data::new(kind, Vec::new());
        assert!(matches!(
            stream(
                &source,
                "SELECT DISTINCT N FROM T WHERE False",
                ExecutionOptions::default()
            ),
            Err(StreamError::Query(QueryError::Unsupported))
        ));
        assert_eq!(source.scans.get(), 0);
    }
    let source = Data::new(ScalarType::Int32, Vec::new());
    for sql in [
        "SELECT DISTINCT IIf(N=1,N,'x') FROM T WHERE False",
        "SELECT N FROM (SELECT DISTINCT N FROM T) Q",
        "SELECT DISTINCT N FROM T ORDER BY N",
        "SELECT DISTINCT TOP 10 PERCENT N FROM T",
        "SELECT N FROM T UNION ALL SELECT DISTINCT CDbl(N) FROM T WHERE False",
    ] {
        assert!(
            matches!(
                stream(&source, sql, ExecutionOptions::default()),
                Err(StreamError::Query(QueryError::Unsupported))
            ),
            "{sql}"
        );
        assert_eq!(source.scans.get(), 0);
    }
}

#[test]
fn physical_and_logical_eof_failures_never_publish_a_distinct_prefix() {
    let mut source = Data::new(ScalarType::Int32, vec![Value::Int32(1), Value::Int32(2)]);
    source.fail_eof = true;
    let parsed = parse_query("SELECT DISTINCT N FROM T", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let mut calls = 0;
    let error = plan.execute_progressive(&source, &[], ExecutionOptions::default(), &mut |_| {
        calls += 1;
        Ok::<_, ()>(())
    });
    assert!(matches!(error, Err(StreamError::Query(QueryError::Source))));
    assert_eq!(calls, 0);

    source.fail_eof = false;
    let sql = "SELECT DISTINCT N FROM (SELECT N,1/(N-2) AS Bad FROM T GROUP BY N) Q";
    let parsed = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let error = plan.execute_progressive(&source, &[], ExecutionOptions::default(), &mut |_| {
        calls += 1;
        Ok::<_, ()>(())
    });
    assert!(matches!(
        error,
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
    assert!(source.eof.get());
    assert_eq!(calls, 0);
    assert_eq!(source.buffered.get(), 0);
}

#[test]
fn swallowed_input_errors_and_the_first_final_sink_failure_stay_sticky() {
    let mut source = Data::new(
        ScalarType::Int32,
        vec![Value::Int32(1), Value::Int32(0), Value::Int32(2)],
    );
    source.swallow = true;
    source.fail_eof = true;
    assert!(matches!(
        stream(
            &source,
            "SELECT DISTINCT CLng(1\\N) FROM T",
            ExecutionOptions::default()
        ),
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));

    source.swallow = false;
    source.fail_eof = false;
    source.scans.set(0);
    let parsed = parse_query(
        "SELECT DISTINCT N FROM T UNION ALL SELECT N FROM T",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let mut calls = 0;
    let error = plan.execute_progressive(&source, &[], ExecutionOptions::default(), &mut |_| {
        assert!(source.eof.get());
        assert!(!source.active.get());
        calls += 1;
        Err("distinct sink")
    });
    assert!(matches!(error, Err(StreamError::Sink("distinct sink"))));
    assert_eq!(calls, 1);
    assert_eq!(source.scans.get(), 1);
    assert_eq!(source.buffered.get(), 0);
}

#[test]
fn every_observed_checkpoint_and_cumulative_quota_preserves_typed_failures() {
    let source = Data::new(
        ScalarType::Int32,
        vec![Value::Int32(2), Value::Int32(1), Value::Int32(2)],
    );
    let parsed = parse_query("SELECT DISTINCT N FROM T", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let observed = Cell::new(0);
    let count = || {
        observed.set(observed.get() + 1);
        false
    };
    plan.execute_progressive(
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&count),
            ..Default::default()
        },
        &mut |_| Ok::<_, ()>(()),
    )
    .unwrap();
    let total = observed.get();
    for stop in 1..=total {
        observed.set(0);
        let cancel = || {
            observed.set(observed.get() + 1);
            observed.get() == stop
        };
        let error = plan.execute_progressive(
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
                error,
                Err(StreamError::Query(
                    QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled)
                ))
            ),
            "checkpoint {stop}"
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
        let error = plan.execute_progressive(
            &source,
            &[],
            ExecutionOptions {
                limits,
                ..Default::default()
            },
            &mut |_| Ok::<_, ()>(()),
        );
        assert!(matches!(
            error,
            Err(StreamError::Query(
                QueryError::Limit(_) | QueryError::Evaluation(EvalError::Limit(_))
            ))
        ));
    }
    assert_eq!(
        stream(
            &source,
            "SELECT DISTINCT N FROM T",
            ExecutionOptions::default()
        )
        .unwrap(),
        [vec![Value::Int32(2)], vec![Value::Int32(1)]]
    );
}

#[test]
fn generated_input_retains_only_the_small_distinct_population() {
    let mut source = Data::new(ScalarType::Int32, Vec::new());
    source.generated = Some(100_003);
    let options = ExecutionOptions {
        limits: QueryLimits {
            bytes: 32 * 1024 * 1024,
            rows: 500_000,
            steps: 20_000_000,
        },
        ..Default::default()
    };
    let actual = stream(&source, "SELECT DISTINCT CInt(N Mod 17) FROM T", options).unwrap();
    let expected: Vec<_> = (0..17).map(|number| vec![Value::Int16(number)]).collect();
    assert_eq!(actual, expected);
    assert_eq!(source.buffered.get(), 0);
}
