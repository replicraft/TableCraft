use super::*;

#[test]
fn all_schemas_are_preflighted_and_selected_arms_are_rechecked() {
    for before_scan in [true, false] {
        let mut source = Tables::new(&[1, 2]);
        let syntax = parse_query(
            "SELECT N FROM A UNION ALL SELECT N FROM B",
            Default::default(),
        )
        .unwrap();
        let plan = prepare(syntax.query(), &source).unwrap();
        source.changed_b.set(before_scan);
        source.change_after_a = !before_scan;
        let mut output = Vec::new();
        let result = plan.execute_progressive(&source, &[], Default::default(), &mut |row| {
            output.push(row.to_vec());
            Ok::<_, ()>(())
        });
        assert!(matches!(
            result,
            Err(StreamError::Query(QueryError::SchemaChanged))
        ));
        if before_scan {
            assert!(source.scans.borrow().is_empty());
            assert!(output.is_empty());
        } else {
            assert_eq!(*source.scans.borrow(), ["A"]);
            assert_eq!(output, vec![vec![Value::Int32(1)], vec![Value::Int32(2)]]);
        }
    }
}

#[test]
fn later_arm_errors_preserve_prefix_and_stop_before_a_following_arm() {
    for expression in ["1/(N-2)", "ChrW(55296)", "CVErr(7)"] {
        let source = Tables::new(&[1, 2, 3]);
        let sql = format!(
            "SELECT N AS V FROM A UNION ALL SELECT {expression} FROM B UNION ALL SELECT N FROM A"
        );
        let syntax = parse_query(&sql, Default::default()).unwrap();
        let plan = prepare(syntax.query(), &source).unwrap();
        let mut output = Vec::new();
        let result = plan.execute_progressive(
            &source,
            &[],
            ExecutionOptions {
                number_format: Some(NumberFormat::Invariant),
                ..Default::default()
            },
            &mut |row| {
                output.push(row.to_vec());
                Ok::<_, ()>(())
            },
        );
        assert!(matches!(result, Err(StreamError::Query(_))), "{sql}");
        assert!(output.len() >= 3, "{sql}");
        assert_eq!(*source.scans.borrow(), ["A", "B"], "{sql}");
    }
}

#[test]
fn eof_failure_and_swallowed_sink_failure_remain_terminal() {
    for swallowed in [false, true] {
        let mut source = Tables::new(&[1, 2, 3]);
        source.swallow = swallowed;
        source.fail_eof = Some("B");
        let syntax = parse_query(
            "SELECT N FROM A UNION ALL SELECT N FROM B UNION ALL SELECT N FROM A",
            Default::default(),
        )
        .unwrap();
        let plan = prepare(syntax.query(), &source).unwrap();
        let mut calls = 0;
        let result = plan.execute_progressive(&source, &[], Default::default(), &mut |_| {
            calls += 1;
            if calls == 4 {
                Err("first sink failure")
            } else {
                Ok(())
            }
        });
        assert!(matches!(
            result,
            Err(StreamError::Sink("first sink failure"))
        ));
        assert_eq!(calls, 4);
        assert_eq!(*source.scans.borrow(), ["A", "B"]);
    }

    let mut source = Tables::new(&[1, 2]);
    source.fail_eof = Some("B");
    let syntax = parse_query(
        "SELECT N FROM A UNION ALL SELECT N FROM B",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let mut calls = 0;
    let result = plan.execute_progressive(&source, &[], Default::default(), &mut |_| {
        calls += 1;
        Ok::<_, ()>(())
    });
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Source))
    ));
    assert_eq!(calls, 4);
}

#[test]
fn excluded_raw_cells_skip_publication_but_not_eager_evaluation_or_eof() {
    let source = Tables::new(&[1, 2, 3]);
    let rows = stream(
        "SELECT TOP 1 IIf(N=1,'ok',ChrW(55296)) AS V FROM A UNION ALL SELECT TOP 1 'end' FROM B",
        &source,
        Default::default(),
    );
    assert_eq!(
        rows,
        vec![
            vec![Value::Text("ok".into())],
            vec![Value::Text("end".into())]
        ]
    );
    assert_eq!(source.visits.get(), 6);

    let syntax = parse_query(
        "SELECT TOP 1 N FROM A UNION ALL SELECT TOP 1 1/(N-2) FROM B",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let result =
        plan.execute_progressive(&source, &[], Default::default(), &mut |_| Ok::<_, ()>(()));
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
}

#[test]
fn cancellation_and_resource_limits_span_arm_boundaries() {
    let source = Tables::new(&[1, 2]);
    let syntax = parse_query(
        "SELECT N FROM A UNION ALL SELECT N FROM B",
        Default::default(),
    )
    .unwrap();
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
            ..Default::default()
        },
        &mut |_| Ok::<_, ()>(()),
    )
    .unwrap();

    for stop in 1..=observed.get() {
        let calls = Cell::new(0);
        let cancel = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let result = plan.execute_progressive(
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
                result,
                Err(StreamError::Query(
                    QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled)
                ))
            ),
            "checkpoint {stop}"
        );
    }

    let result = plan.execute_progressive(
        &source,
        &[],
        ExecutionOptions {
            limits: QueryLimits {
                rows: 3,
                ..Default::default()
            },
            ..Default::default()
        },
        &mut |_| Ok::<_, ()>(()),
    );
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Limit(_)))
    ));
}
