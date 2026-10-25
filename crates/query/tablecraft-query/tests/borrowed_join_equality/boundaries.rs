use super::*;

#[test]
fn compound_and_same_side_conditions_keep_the_whole_eager_predicate() {
    for condition in [
        "L.K=R.K AND 1/0=0",
        "L.K=R.K OR 1/0=0",
        "L.K=L.Id AND 1/0=0",
    ] {
        let source = tables(ScalarType::Int32, &[Some(0)], &[Some(1)]);
        let sql = format!("SELECT L.Id FROM L INNER JOIN R ON {condition}");
        let result = execute(&sql, &source, Default::default());
        assert_eq!(
            result,
            Err(QueryError::Evaluation(EvalError::DivisionByZero))
        );
        assert_eq!(source.scans.get(), 2);
    }
    let source = tables(ScalarType::Int32, &[Some(0)], &[Some(1), Some(1)]);
    let rows = execute(
        "SELECT R.Id FROM L INNER JOIN R ON L.K=L.Id",
        &source,
        Default::default(),
    )
    .unwrap();
    assert_eq!(rows, [vec![Value::Int32(0)], vec![Value::Int32(1)]]);
}

#[test]
fn complete_source_failures_are_retained_even_when_keys_cannot_match() {
    for invalid_row in [false, true] {
        let mut source = tables(ScalarType::Int32, &[Some(0)], &[Some(1)]);
        source.fail_right = !invalid_row;
        source.wrong_right = invalid_row;
        let result = execute(
            "SELECT L.Id FROM L INNER JOIN R ON L.K=R.K",
            &source,
            Default::default(),
        );
        if invalid_row {
            assert!(matches!(result, Err(QueryError::Schema(_))));
        } else {
            assert_eq!(result, Err(QueryError::Source));
        }
        assert_eq!(source.scans.get(), 2);
    }
}

#[test]
fn borrowed_pair_checks_remain_cancellable_and_resource_bounded() {
    let source = tables(
        ScalarType::Int64,
        &[None, Some(0), Some(1)],
        &[Some(1), Some(0), None],
    );
    let sql = "SELECT L.Id,R.Id FROM L LEFT JOIN R ON R.K=L.K";
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let calls = Cell::new(0);
    let count_checkpoints = || {
        calls.set(calls.get() + 1);
        false
    };
    let baseline = plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&count_checkpoints),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(baseline.rows().len(), 3);
    for stop in 1..=calls.get() {
        let observed = Cell::new(0);
        let cancelled = || {
            observed.set(observed.get() + 1);
            observed.get() == stop
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
    assert_eq!(
        plan.execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        baseline.rows()
    );
}

#[test]
fn mixed_integral_carriers_keep_the_general_candidate_path() {
    let keys = (0..20).map(Some).collect::<Vec<_>>();
    let source = Tables {
        inner: MemorySource::new(vec![
            table("L", ScalarType::Int32, &keys),
            table("R", ScalarType::Int64, &keys),
        ])
        .unwrap(),
        scans: Cell::new(0),
        fail_right: false,
        wrong_right: false,
    };
    let sql = "SELECT L.K FROM L INNER JOIN R ON L.K=R.K";
    for declared_integral in [false, true] {
        let rows = execute(
            sql,
            &source,
            ExecutionOptions {
                declared_integral,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            rows,
            (0..20)
                .map(|key| vec![Value::Int32(key)])
                .collect::<Vec<_>>()
        );

        // The specialization deliberately requires equal proven carriers.
        // General candidate copies still consume the existing row allowance.
        let result = execute(
            sql,
            &source,
            ExecutionOptions {
                declared_integral,
                limits: QueryLimits {
                    rows: 200,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        assert!(matches!(result, Err(QueryError::Limit(_))));
    }
}
