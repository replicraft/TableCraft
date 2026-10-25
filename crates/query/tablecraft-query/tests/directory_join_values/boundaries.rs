use super::*;

#[test]
fn malformed_selected_rows_and_late_source_failures_are_checked_with_empty_opposite() {
    for empty_side in [0, 1] {
        for malformed in [false, true] {
            let left = if empty_side == 0 {
                vec![]
            } else {
                vec![Some(1)]
            };
            let right = if empty_side == 1 {
                vec![]
            } else {
                vec![Some(1)]
            };
            let mut source = Inputs::new(ScalarType::Int32, &left, &right);
            if malformed {
                source.tables[1 - empty_side].rows[0][1] = Value::Text("bad selected key".into());
            } else {
                source.fail_right = true;
            }
            let result = execute(
                "SELECT L.Id,R.Id FROM L LEFT JOIN R ON L.K=R.K",
                &source,
                Default::default(),
            );
            if malformed {
                assert!(matches!(result, Err(QueryError::Schema(_))));
            } else {
                assert_eq!(result, Err(QueryError::Source));
            }
        }
    }
}

#[test]
fn unqualified_compound_and_mixed_keys_retain_eager_general_semantics() {
    for condition in [
        "L.K=R.K AND 1/0=0",
        "L.K=R.K OR 1/0=0",
        "L.K+0=R.K AND 1/0=0",
    ] {
        let source = Inputs::new(ScalarType::Int32, &[Some(0)], &[Some(1)]);
        let sql = format!("SELECT L.Id FROM L INNER JOIN R ON {condition}");
        assert_eq!(
            execute(&sql, &source, Default::default()),
            Err(QueryError::Evaluation(EvalError::DivisionByZero))
        );
    }

    let keys: Vec<_> = (0..20).map(Some).collect();
    let mut mixed = Inputs::new(ScalarType::Int32, &keys, &keys);
    mixed.tables[1] = table(ScalarType::Int64, &keys, 22);
    for declared_integral in [false, true] {
        let sql = "SELECT L.Id,R.Id FROM L INNER JOIN R ON L.K=R.K";
        let ordinary = execute(
            sql,
            &mixed,
            ExecutionOptions {
                declared_integral,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            ordinary,
            (0..20)
                .map(|value| vec![Value::Int32(value); 2])
                .collect::<Vec<_>>()
        );
        let limited = execute(
            sql,
            &mixed,
            ExecutionOptions {
                declared_integral,
                limits: QueryLimits {
                    rows: 200,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        assert!(matches!(limited, Err(QueryError::Limit(_))));
    }

    let mut text = Inputs::new(ScalarType::Int32, &[Some(1)], &[Some(1)]);
    for side in 0..2 {
        text.tables[side].columns[1] = Column::new("K".into(), ScalarType::Text, true).unwrap();
        text.tables[side].rows[0][1] = Value::Text("01".into());
    }
    let result = execute(
        "SELECT L.Id FROM L INNER JOIN R ON L.K=R.K",
        &text,
        Default::default(),
    );
    assert_eq!(
        result,
        Err(QueryError::Evaluation(EvalError::CollationRequired))
    );
}

#[test]
fn all_observed_cancellation_points_and_resource_errors_leave_prepared_plan_reusable() {
    let source = Inputs::new(
        ScalarType::Int64,
        &[None, Some(2), Some(1), Some(2)],
        &[Some(1), Some(2), None, Some(2)],
    );
    let query = parse_query(
        "SELECT L.Id,R.Id FROM L RIGHT JOIN R ON R.K=L.K",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    let observed = Cell::new(0);
    let count = || {
        observed.set(observed.get() + 1);
        false
    };
    let baseline = plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&count),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(observed.get() > 20);
    for stop in 1..=observed.get() {
        let calls = Cell::new(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let result = plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancelled),
                ..Default::default()
            },
        );
        assert!(
            matches!(result, Err(QueryError::Cancelled)),
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
        let result = plan.execute(
            &source,
            &[],
            ExecutionOptions {
                limits,
                ..Default::default()
            },
        );
        assert!(matches!(result, Err(QueryError::Limit(_))));
    }
    let retry = plan.execute(&source, &[], Default::default()).unwrap();
    assert_eq!(retry.rows(), baseline.rows());
    assert_eq!(retry.columns(), baseline.columns());
}

#[test]
fn selected_source_validation_stops_at_first_bad_record_and_does_not_publish_matches() {
    let mut source = Inputs::new(
        ScalarType::Int32,
        &[Some(1), Some(2)],
        &[Some(1), Some(2), Some(3)],
    );
    source.tables[1].rows[1][1] = Value::Text("late key corruption".into());
    let result = execute(
        "SELECT L.Id,R.Id FROM L INNER JOIN R ON L.K=R.K",
        &source,
        Default::default(),
    );
    assert!(matches!(result, Err(QueryError::Schema(_))));
    assert_eq!(source.visits[1].get(), 2);
    assert_eq!(source.scans[1].get(), 1);
}
