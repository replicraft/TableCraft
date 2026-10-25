use super::*;

#[test]
fn source_fields_take_precedence_over_same_named_prior_projection_aliases() {
    let source = Records::new();
    let result = run(
        "SELECT N+CLng(10) AS N,N AS SourceValue FROM T",
        &source,
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        result.rows(),
        [
            vec![Value::Int32(11), Value::Int32(1)],
            vec![Value::Int32(12), Value::Int32(2)],
            vec![Value::Int32(13), Value::Int32(3)],
        ]
    );
}

#[test]
fn forward_self_and_non_projection_alias_references_fail_before_source_scans() {
    let source = Records::new();
    for sql in [
        "SELECT LaterValue AS EarlyValue,CLng(1) AS LaterValue FROM T",
        "SELECT MissingValue AS MissingValue FROM T",
        "SELECT N AS ProjectionOnly FROM T WHERE ProjectionOnly>0",
        "SELECT N AS ProjectionOnly,Count(*) FROM T GROUP BY ProjectionOnly",
        "SELECT N AS ProjectionOnly,(SELECT ProjectionOnly) AS NestedValue FROM T",
        "SELECT N AS Secret,(SELECT 2 AS X UNION ALL SELECT 3 ORDER BY Secret) AS V FROM T",
    ] {
        let syntax = parse_query(sql, Default::default()).unwrap();
        assert!(prepare(syntax.query(), &source).is_err(), "{sql}");
        assert_eq!(source.scans.get(), 0, "{sql}");
    }
}

#[test]
fn alias_definition_errors_remain_eager_and_cannot_be_hidden_by_a_later_consumer() {
    let source = Records::new();
    for sql in [
        "SELECT 1/0 AS Broken,Broken+1 AS LaterValue",
        "SELECT IIf(True,1,1/0) AS Broken,IsNull(Broken) AS LaterValue",
        "SELECT LenB(Broken) FROM (SELECT IIf(True,ChrW(55296),1/0) AS Broken) Q",
    ] {
        assert_eq!(
            run(sql, &source, Default::default()).err(),
            Some(QueryError::Evaluation(EvalError::DivisionByZero)),
            "{sql}"
        );
    }
}

#[test]
fn every_observed_alias_execution_checkpoint_can_cancel() {
    let source = Records::new();
    let syntax = parse_query(
        "SELECT N AS Seed,Seed+CLng(1) AS NextValue FROM T",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let observed = Cell::new(0);
    let observe = || {
        observed.set(observed.get() + 1);
        false
    };
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&observe),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(observed.get() > 3);
    for stop in 1..=observed.get() {
        let calls = Cell::new(0);
        let cancel = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        assert!(
            matches!(
                plan.execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        cancelled: Some(&cancel),
                        ..Default::default()
                    }
                ),
                Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "checkpoint {stop}"
        );
    }
}
