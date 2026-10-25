use super::*;

#[test]
fn unused_inner_operations_remain_eager_under_outer_filters_and_publication_caps() {
    let source = Inputs::new(4, &[]);
    let result = run(
        "SELECT X FROM (SELECT Id AS X,1/0 AS Unused FROM Data) Q WHERE False",
        &source,
        &[],
        Default::default(),
    );
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
    assert_eq!(source.visits.get(), 1);
    assert_eq!(*source.projections.borrow(), [vec![1]]);

    let selected_bad = Inputs::new(4, &[]);
    let result = run(
        "SELECT Count(*) FROM (SELECT Id,Unused FROM Data) Q",
        &selected_bad,
        &[],
        Default::default(),
    );
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Schema(_)))
    ));
    assert_eq!(*selected_bad.projections.borrow(), [vec![0, 1]]);

    let source = Inputs::new(5, &[]);
    let syntax = parse_query(
        "SELECT Id FROM (SELECT TOP 2 Id,1/(Id-4) AS Unused FROM Data) Q",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let mut prefix = Vec::new();
    let result = plan.execute_progressive(&source, &[], Default::default(), &mut |row| {
        prefix.push(row.to_vec());
        Ok::<_, ()>(())
    });
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
    assert_eq!(prefix, [vec![Value::Int32(1)], vec![Value::Int32(2)]]);
    assert_eq!(source.visits.get(), 4);
}

#[test]
fn every_forbidden_inner_shape_later_empty_arm_cycle_and_parameter_scope_refuses_before_rows() {
    for inner in [
        "SELECT Last(Id) AS N FROM Data",
        "SELECT 1 AS Id",
        "SELECT TOP 0 Id FROM Data",
        "SELECT Id,Last(Id) AS N FROM Data GROUP BY Id",
        "SELECT Id FROM Data UNION ALL SELECT Id FROM Other",
        "SELECT DISTINCT Id FROM Data",
        "SELECT Id FROM Data ORDER BY Id",
        "SELECT TOP 50 PERCENT Id FROM Data",
        "SELECT A.Id FROM Data A,Other B",
        "SELECT A.Id FROM Data A INNER JOIN Other B ON A.Id=B.Id",
        "SELECT Rnd() AS N FROM Data",
        "SELECT Eval('1') AS N FROM Data",
    ] {
        let source = Inputs::new(0, &[("Bad", inner)]);
        let sql = "SELECT * FROM Bad";
        assert!(
            matches!(
                run(sql, &source, &[], Default::default()),
                Err(StreamError::Query(QueryError::Unsupported))
            ),
            "{inner}"
        );
        assert!(source.scans.borrow().is_empty());
    }
    for (definitions, sql, expected) in [
        (
            vec![(
                "Bad",
                "TRANSFORM Count(*) SELECT Id FROM Data GROUP BY Id PIVOT Label IN ('x')",
            )],
            "SELECT * FROM Bad",
            QueryError::Unsupported,
        ),
        (
            vec![("A", "SELECT Id FROM B"), ("B", "SELECT Id FROM A")],
            "SELECT Id FROM A",
            QueryError::DependencyCycle,
        ),
        (
            vec![("Unbound", "SELECT [P] FROM Data")],
            "PARAMETERS P LONG; SELECT * FROM Unbound",
            QueryError::UnknownName,
        ),
    ] {
        let source = Inputs::new(0, &definitions);
        let parameters = if sql.starts_with("PARAMETERS ") {
            vec![("P", Value::Int32(1))]
        } else {
            Vec::new()
        };
        let result = run(sql, &source, &parameters, Default::default());
        let Err(StreamError::Query(actual)) = result else {
            panic!("forbidden definition must refuse")
        };
        assert_eq!(actual, expected);
        assert!(source.scans.borrow().is_empty());
    }
    let source = Inputs::new(0, &[]);
    let result = run(
        "SELECT Id FROM (SELECT Id FROM Data) A UNION ALL SELECT N FROM (SELECT Last(Id) AS N FROM Other WHERE False) B",
        &source,
        &[],
        Default::default(),
    );
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Unsupported))
    ));
    assert!(source.scans.borrow().is_empty());
}

#[test]
fn repeated_named_uses_own_fresh_top_counters_and_never_reuse_a_complete_prefix() {
    let source = Inputs::new(4, &[("Two", "SELECT TOP 2 Id FROM Data")]);
    let syntax = parse_query(
        "SELECT Id FROM Two UNION ALL SELECT Id FROM Two",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    for _ in 0..2 {
        let mut rows = Vec::new();
        let summary = plan
            .execute_progressive(&source, &[], Default::default(), &mut |row| {
                rows.push(row.to_vec());
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(
            rows,
            [1, 2, 1, 2]
                .into_iter()
                .map(|id| vec![Value::Int32(id)])
                .collect::<Vec<_>>()
        );
        assert_eq!(summary.scanned_rows, 8);
        assert_eq!(summary.emitted_rows, 4);
    }
    assert_eq!(*source.scans.borrow(), ["Data", "Data", "Data", "Data"]);
}

#[test]
fn eof_and_late_selected_payload_failures_keep_aggregate_atomicity_and_row_prefixes() {
    let mut source = Inputs::new(4, &[("Two", "SELECT TOP 2 Id FROM Data")]);
    source.fail_eof = true;
    let syntax = parse_query("SELECT Count(*) FROM Two", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let calls = Cell::new(0);
    let result = plan.execute_progressive(&source, &[], Default::default(), &mut |_| {
        calls.set(calls.get() + 1);
        Ok::<_, ()>(())
    });
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Source))
    ));
    assert_eq!(calls.get(), 0);
    assert_eq!(source.visits.get(), 4);

    let mut source = Inputs::new(4, &[("A", "SELECT Id FROM Data")]);
    source.corrupt = Some(2);
    let syntax = parse_query("SELECT Id FROM A", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let mut prefix = Vec::new();
    let result = plan.execute_progressive(&source, &[], Default::default(), &mut |row| {
        prefix.push(row.to_vec());
        Ok::<_, ()>(())
    });
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Schema(_)))
    ));
    assert_eq!(prefix, [vec![Value::Int32(1)], vec![Value::Int32(2)]]);
    assert_eq!(source.visits.get(), 3);
}

#[test]
fn sticky_sink_failure_and_selected_leaf_schema_drift_stop_later_stage_work() {
    let mut source = Inputs::new(4, &[("A", "SELECT Id FROM Data")]);
    source.swallow = true;
    source.corrupt = Some(1);
    source.fail_eof = true;
    let syntax = parse_query(
        "SELECT Id FROM A UNION ALL SELECT Id FROM Other",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let calls = Cell::new(0);
    let result = plan.execute_progressive(&source, &[], Default::default(), &mut |_| {
        calls.set(calls.get() + 1);
        Err("original sink rejection")
    });
    assert!(matches!(
        result,
        Err(StreamError::Sink("original sink rejection"))
    ));
    assert_eq!(calls.get(), 1);
    assert_eq!(*source.scans.borrow(), ["Data"]);

    let mut source = Inputs::new(
        2,
        &[("A", "SELECT Id FROM Data"), ("B", "SELECT Id FROM Other")],
    );
    source.drift_after_data = true;
    let syntax = parse_query(
        "SELECT Id FROM A UNION ALL SELECT Id FROM B",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let mut prefix = Vec::new();
    let result = plan.execute_progressive(&source, &[], Default::default(), &mut |row| {
        prefix.push(row.to_vec());
        Ok::<_, ()>(())
    });
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::SchemaChanged))
    ));
    assert_eq!(prefix, [vec![Value::Int32(1)], vec![Value::Int32(2)]]);
    assert_eq!(*source.scans.borrow(), ["Data"]);
}

#[test]
fn cumulative_limits_all_cancellation_points_and_reuse_keep_frames_local_to_one_run() {
    let source = Inputs::new(
        6,
        &[
            ("A", "SELECT Id,Score FROM Data WHERE Id>=2"),
            ("B", "SELECT TOP 3 Id,CLng(Id+1) AS N FROM A"),
        ],
    );
    let syntax = parse_query("SELECT Id,N FROM B", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let observed = Cell::new(0);
    let count = || {
        observed.set(observed.get() + 1);
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
    for stop in 1..=observed.get() {
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
    let mut actual = Vec::new();
    plan.execute_progressive(&source, &[], Default::default(), &mut |row| {
        actual.push(row.to_vec());
        Ok::<_, ()>(())
    })
    .unwrap();
    assert_eq!(actual, expected);
}
