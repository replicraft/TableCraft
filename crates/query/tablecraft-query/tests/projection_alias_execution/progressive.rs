use super::*;

#[test]
fn direct_and_union_all_aliases_match_buffered_values_and_active_promotions() {
    for sql in [
        "SELECT N AS Seed,Seed+CLng(10) AS NextValue FROM T",
        "SELECT N AS Seed,CLngLng(Seed) AS Amount FROM T UNION ALL SELECT N AS Seed,CCur(Seed) AS Amount FROM T",
    ] {
        for declared in [false, true] {
            let source = Records::new();
            let syntax = parse_query(sql, Default::default()).unwrap();
            let plan = prepare(syntax.query(), &source).unwrap();
            let options = ExecutionOptions {
                declared_integral: declared,
                ..Default::default()
            };
            let buffered = plan.execute(&source, &[], options).unwrap();
            let mut output = Vec::new();
            let summary = plan
                .execute_progressive(&source, &[], options, &mut |row| {
                    output.push(row.to_vec());
                    Ok::<_, ()>(())
                })
                .unwrap();

            assert_eq!(output, buffered.rows(), "{sql}");
            let expected = if sql.contains("UNION") {
                [1, 1, 2, 1, 1, 2]
                    .into_iter()
                    .enumerate()
                    .map(|(index, value)| {
                        let (coefficient, scale) = if index < 3 {
                            (i128::from(value), 0)
                        } else {
                            (i128::from(value) * 10_000, 4)
                        };
                        vec![
                            Value::Int32(value),
                            Value::Decimal(Decimal::new(coefficient, scale).unwrap()),
                        ]
                    })
                    .collect::<Vec<_>>()
            } else {
                [1, 1, 2]
                    .into_iter()
                    .map(|value| vec![Value::Int32(value), Value::Int32(value + 10)])
                    .collect()
            };
            assert_eq!(output, expected, "{sql}");
            assert_eq!(summary.scanned_rows, output.len() as u64);
            assert_eq!(source.visits.get(), output.len());
        }
    }
}

#[test]
fn raw_aliases_are_observed_before_publication_and_top_omits_only_publication() {
    for (sql, expected, count) in [
        (
            "SELECT ChrW(55357)&ChrW(56832) AS Pair,LenB(Pair) AS UnitBytes FROM T",
            vec![Value::Text("😀".into()), Value::Int32(4)],
            3,
        ),
        (
            "SELECT TOP 1 IIf(N=1,'ok',ChrW(55296)) AS RawValue,LenB(RawValue) AS UnitBytes FROM T",
            vec![Value::Text("ok".into()), Value::Int32(4)],
            1,
        ),
        (
            "SELECT TOP 1 IIf(N=1,'ok',ChrB(65)) AS RawValue,LenB(RawValue) AS UnitBytes FROM T",
            vec![Value::Text("ok".into()), Value::Int32(4)],
            1,
        ),
    ] {
        let source = Records::new();
        let syntax = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(syntax.query(), &source).unwrap();
        let mut output = Vec::new();
        let summary = plan
            .execute_progressive(&source, &[], Default::default(), &mut |row| {
                output.push(row.to_vec());
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(output, vec![expected; count], "{sql}");
        assert_eq!(summary.scanned_rows, 3);
        assert_eq!(source.visits.get(), 3);
        assert_eq!(
            output,
            plan.execute(&source, &[], Default::default())
                .unwrap()
                .rows()
        );
    }
}

#[test]
fn top_and_alias_reuse_do_not_hide_later_eager_arithmetic_errors() {
    let source = Records::new();
    let syntax = parse_query(
        "SELECT TOP 1 N AS Seed,1/(Seed-CLng(2)) AS Quotient FROM T",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let mut output = Vec::new();
    let result = plan.execute_progressive(&source, &[], Default::default(), &mut |row| {
        output.push(row.to_vec());
        Ok::<_, ()>(())
    });
    assert_eq!(output.len(), 1);
    assert_eq!(source.visits.get(), 3);
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
}

#[test]
fn first_sink_error_survives_swallowed_callbacks_and_prevents_the_next_arm() {
    let mut source = Records::new();
    source.swallow = true;
    source.fail_eof = true;
    let sql = "SELECT N AS Seed,Seed+CLng(1) AS NextValue FROM T UNION ALL SELECT N AS Seed,Seed+CLng(2) AS NextValue FROM T";
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let mut calls = 0;
    let result = plan.execute_progressive(&source, &[], Default::default(), &mut |_| {
        calls += 1;
        if calls == 2 {
            Err("first sink failure")
        } else {
            Ok(())
        }
    });
    assert!(matches!(
        result,
        Err(StreamError::Sink("first sink failure"))
    ));
    assert_eq!(calls, 2);
    assert_eq!(source.scans.get(), 1);
    assert_eq!(source.visits.get(), 3);
}

#[test]
fn cancellation_and_cumulative_limits_cover_omitted_alias_rows() {
    let source = Records::new();
    let syntax = parse_query(
        "SELECT TOP 1 N AS Seed,Seed+CLng(1) AS NextValue FROM T",
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

    for limits in [
        QueryLimits {
            rows: 4,
            ..Default::default()
        },
        QueryLimits {
            steps: 150,
            ..Default::default()
        },
        QueryLimits {
            bytes: 4096,
            ..Default::default()
        },
    ] {
        let mut one = Records::new();
        one.rows.truncate(1);
        let options = ExecutionOptions {
            limits,
            ..Default::default()
        };
        plan.execute_progressive(&one, &[], options, &mut |_| Ok::<_, ()>(()))
            .unwrap();

        let mut many = Records::new();
        many.rows = (0..200)
            .map(|number| {
                vec![
                    Value::Int32(number),
                    Value::Text("a".into()),
                    Value::Text("a".into()),
                ]
            })
            .collect();
        let result = plan.execute_progressive(&many, &[], options, &mut |_| Ok::<_, ()>(()));
        assert!(matches!(
            result,
            Err(StreamError::Query(
                QueryError::Limit(_) | QueryError::Evaluation(EvalError::Limit(_))
            ))
        ));
    }
}
