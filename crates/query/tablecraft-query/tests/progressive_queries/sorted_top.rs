//! Sorted TOP completes its population before stable ordering and publication.
use super::*;
use std::cmp::Ordering;
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::{QueryLimits, source::CollationResolver};

struct Folded;

impl Collation for Folded {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.to_ascii_lowercase().cmp(&right.to_ascii_lowercase()))
    }
}

impl CollationResolver for Folded {
    fn resolve(&self, _: TextCollationId) -> Result<&dyn Collation, QueryError> {
        Ok(self)
    }
}

#[test]
fn sorted_top_keeps_all_boundary_ties_in_stable_source_order_after_eof() {
    let mut source = Rows::new([3, 1, 2, 2, 1]);
    source
        .columns
        .push(Column::new("Position".into(), ScalarType::Int32, false).unwrap());
    for (position, row) in source.rows.iter_mut().enumerate() {
        row.push(Value::Int32(position as i32));
    }
    for (order, expected) in [
        ("N", vec![(1, 1), (1, 4), (2, 2), (2, 3)]),
        ("N DESC", vec![(3, 0), (2, 2), (2, 3)]),
        ("N,[Position] DESC", vec![(1, 4), (1, 1), (2, 3)]),
    ] {
        let sql = format!("SELECT TOP 3 N,[Position] FROM T ORDER BY {order}");
        let syntax = parse_query(&sql, Default::default()).unwrap();
        let prepared = prepare(syntax.query(), &source).unwrap();
        let expected: Vec<_> = expected
            .into_iter()
            .map(|(n, position)| vec![Value::Int32(n), Value::Int32(position)])
            .collect();
        for declared_integral in [false, true] {
            let options = ExecutionOptions {
                declared_integral,
                ..Default::default()
            };
            assert_eq!(
                prepared.execute(&source, &[], options).unwrap().rows(),
                expected
            );
            source.visits.set(0);
            let mut actual = Vec::new();
            let summary = prepared
                .execute_progressive(&source, &[], options, &mut |row| {
                    assert_eq!(source.visits.get(), 5);
                    actual.push(row.to_vec());
                    Ok::<_, ()>(())
                })
                .unwrap();
            assert_eq!(actual, expected);
            assert_eq!(summary.scanned_rows, 5);
            assert_eq!(summary.emitted_rows, expected.len() as u64);
        }
    }
}

#[test]
fn sorted_expressions_aliases_distinct_and_derived_inputs_reuse_buffered_semantics() {
    for values in [vec![], vec![3], vec![3, 1, 2, 2, 1]] {
        for sql in [
            "SELECT TOP 2 N+1 AS V FROM T ORDER BY V DESC",
            "SELECT TOP 2 N FROM T ORDER BY -N,N+1",
            "SELECT TOP 2 N FROM T ORDER BY CDbl(N) DESC",
            "SELECT DISTINCT TOP 2 N FROM T ORDER BY N",
            "SELECT TOP 2 N FROM T WHERE False ORDER BY N",
            "SELECT TOP 2 N FROM (SELECT N FROM T WHERE N>0) Q ORDER BY N",
        ] {
            let source = Rows::new(values.clone());
            let syntax = parse_query(sql, Default::default()).unwrap();
            let prepared = prepare(syntax.query(), &source).unwrap();
            let expected = prepared.execute(&source, &[], Default::default()).unwrap();
            let mut actual = Vec::new();
            let summary = prepared
                .execute_progressive(&source, &[], Default::default(), &mut |row| {
                    actual.push(row.to_vec());
                    Ok::<_, ()>(())
                })
                .unwrap();
            assert_eq!(actual, expected.rows(), "{sql}");
            assert_eq!(summary.scanned_rows, values.len() as u64, "{sql}");
        }
    }
}

#[test]
fn sorted_text_uses_explicit_collation_and_retains_the_complete_policy_cohort() {
    let mut source = Rows::new([]);
    source.columns = vec![Column::new("K".into(), ScalarType::Text, false).unwrap()];
    source.rows = ["b", "A", "a", "c"]
        .into_iter()
        .map(|key| vec![Value::Text(key.into())])
        .collect();
    let syntax = parse_query("SELECT TOP 1 K FROM T ORDER BY K", Default::default()).unwrap();
    let prepared = prepare(syntax.query(), &source).unwrap();
    let options = ExecutionOptions {
        collation: Some(&Folded),
        ..Default::default()
    };
    let expected = vec![vec![Value::Text("A".into())], vec![Value::Text("a".into())]];
    assert_eq!(
        prepared.execute(&source, &[], options).unwrap().rows(),
        expected
    );
    let mut actual = Vec::new();
    prepared
        .execute_progressive(&source, &[], options, &mut |row| {
            actual.push(row.to_vec());
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(actual, expected);

    // The last key contributes another field policy even though TOP would drop
    // its value. Neither comparer identity nor an early candidate can erase it.
    source.columns = vec![
        Column::new("A".into(), ScalarType::Text, false).unwrap(),
        Column::new("B".into(), ScalarType::Text, false).unwrap(),
        Column::new("N".into(), ScalarType::Int32, false).unwrap(),
    ];
    source.policies = vec![
        Some(TextCollationId::from_host(1)),
        Some(TextCollationId::from_host(2)),
        None,
    ];
    source.rows = [1, 1, 2]
        .into_iter()
        .map(|n| {
            vec![
                Value::Text("a".into()),
                Value::Text("z".into()),
                Value::Int32(n),
            ]
        })
        .collect();
    let syntax = parse_query(
        "SELECT TOP 1 N FROM T ORDER BY IIf(N=1,A,B)",
        Default::default(),
    )
    .unwrap();
    let prepared = prepare(syntax.query(), &source).unwrap();
    let options = ExecutionOptions {
        collation_resolver: Some(&Folded),
        ..Default::default()
    };
    let expected = prepared.execute(&source, &[], options).err().unwrap();
    let mut emitted = 0;
    let result = prepared.execute_progressive(&source, &[], options, &mut |_| {
        emitted += 1;
        Ok::<_, ()>(())
    });
    assert!(matches!(result, Err(StreamError::Query(error)) if error == expected));
    assert_eq!(emitted, 0);
}

#[test]
fn sorted_top_never_hides_late_eager_errors_malformed_rows_or_eof_failure() {
    for sql in [
        "SELECT TOP 1 N,1/N FROM T ORDER BY N DESC",
        "SELECT TOP 1 N FROM T WHERE True OR 1/N=0 ORDER BY N DESC",
        "SELECT TOP 1 N FROM T ORDER BY 1/N,N",
    ] {
        let source = Rows::new([2, 1, 0]);
        let syntax = parse_query(sql, Default::default()).unwrap();
        let prepared = prepare(syntax.query(), &source).unwrap();
        let mut emitted = 0;
        let result = prepared.execute_progressive(&source, &[], Default::default(), &mut |_| {
            emitted += 1;
            Ok::<_, ()>(())
        });
        assert!(
            matches!(
                result,
                Err(StreamError::Query(QueryError::Evaluation(
                    EvalError::DivisionByZero
                )))
            ),
            "{sql}"
        );
        assert_eq!(emitted, 0);
        assert_eq!(source.visits.get(), 3);
    }
    for malformed in [false, true] {
        let mut source = Rows::new([2, 1, 3]);
        source.eof_error = !malformed;
        if malformed {
            source.rows[2][0] = Value::Text("wrong carrier".into());
        }
        let syntax = parse_query("SELECT TOP 1 N FROM T ORDER BY N", Default::default()).unwrap();
        let prepared = prepare(syntax.query(), &source).unwrap();
        let mut emitted = 0;
        let result = prepared.execute_progressive(&source, &[], Default::default(), &mut |_| {
            emitted += 1;
            Ok::<_, ()>(())
        });
        assert!(result.is_err());
        assert_eq!(emitted, 0);
    }
}

#[test]
fn sorted_top_keeps_cumulative_pools_and_every_cancellation_checkpoint() {
    let source = Rows::new([4, 3, 2, 2, 1]);
    let syntax = parse_query("SELECT TOP 3 N FROM T ORDER BY N", Default::default()).unwrap();
    let prepared = prepare(syntax.query(), &source).unwrap();
    for pool in 0..3 {
        let mut low = 0;
        let mut high = match pool {
            0 => QueryLimits::DEFAULT_BYTES,
            1 => QueryLimits::DEFAULT_ROWS,
            _ => QueryLimits::DEFAULT_STEPS,
        };
        while low < high {
            let middle = low + (high - low) / 2;
            let mut limits = QueryLimits::default();
            match pool {
                0 => limits.bytes = middle,
                1 => limits.rows = middle,
                _ => limits.steps = middle,
            }
            let result = prepared.execute_progressive(
                &source,
                &[],
                ExecutionOptions {
                    limits,
                    ..Default::default()
                },
                &mut |_| Ok::<_, ()>(()),
            );
            match result {
                Ok(summary) => {
                    assert_eq!(summary.emitted_rows, 3);
                    high = middle;
                }
                Err(StreamError::Query(
                    QueryError::Limit(error) | QueryError::Evaluation(EvalError::Limit(error)),
                )) => {
                    let expected = match pool {
                        0 => tablecraft_core::limits::Resource::Bytes,
                        1 => tablecraft_core::limits::Resource::Rows,
                        _ => tablecraft_core::limits::Resource::Steps,
                    };
                    assert_eq!(error.resource, expected);
                    low = middle + 1;
                }
                other => panic!("unexpected public boundary result: {other:?}"),
            }
        }
        assert!(low > 0);
        for (limit, succeeds) in [(low, true), (low - 1, false)] {
            let mut limits = QueryLimits::default();
            match pool {
                0 => limits.bytes = limit,
                1 => limits.rows = limit,
                _ => limits.steps = limit,
            }
            let result = prepared.execute_progressive(
                &source,
                &[],
                ExecutionOptions {
                    limits,
                    ..Default::default()
                },
                &mut |_| Ok::<_, ()>(()),
            );
            if succeeds {
                assert_eq!(result.unwrap().emitted_rows, 3);
            } else {
                assert!(matches!(
                    result,
                    Err(StreamError::Query(
                        QueryError::Limit(_) | QueryError::Evaluation(EvalError::Limit(_))
                    ))
                ));
            }
        }
    }
    let observed = Cell::new(0);
    let observe = || {
        observed.set(observed.get() + 1);
        false
    };
    prepared
        .execute_progressive(
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
        let result = prepared.execute_progressive(
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
}

#[test]
fn sorted_top_preserves_sink_error_after_complete_scan_and_rejects_old_shapes_before_scan() {
    let source = Rows::new([3, 2, 1]);
    let syntax = parse_query("SELECT TOP 1 N FROM T ORDER BY N", Default::default()).unwrap();
    let prepared = prepare(syntax.query(), &source).unwrap();
    let result = prepared.execute_progressive(&source, &[], Default::default(), &mut |_| {
        assert_eq!(source.visits.get(), 3);
        Err("closed sink")
    });
    assert!(matches!(result, Err(StreamError::Sink("closed sink"))));
    for sql in [
        "SELECT N FROM T ORDER BY N",
        "SELECT TOP 50 PERCENT N FROM T ORDER BY N",
        "SELECT TOP 1 Count(*) FROM T ORDER BY Count(*)",
        "SELECT TOP 1 N FROM (SELECT TOP 1 N FROM T ORDER BY N) Q ORDER BY N",
        "SELECT TOP 1 1 ORDER BY 1",
    ] {
        let source = Rows::new([1]);
        let syntax = parse_query(sql, Default::default()).unwrap();
        let prepared = prepare(syntax.query(), &source).unwrap();
        let result = prepared
            .execute_progressive(&source, &[], Default::default(), &mut |_| Ok::<_, ()>(()));
        assert!(
            matches!(result, Err(StreamError::Query(QueryError::Unsupported))),
            "{sql}"
        );
        assert_eq!(source.scans.get(), 0, "{sql}");
    }
    for sql in [
        "SELECT TOP 0 N FROM T ORDER BY N",
        "SELECT TOP 0 PERCENT N FROM T ORDER BY N",
    ] {
        let source = Rows::new([1]);
        let syntax = parse_query(sql, Default::default()).unwrap();
        assert_eq!(
            prepare(syntax.query(), &source).err(),
            Some(QueryError::Unsupported)
        );
        assert_eq!(source.scans.get(), 0);
    }
}

#[test]
fn sorted_top_keeps_a_large_boundary_tie_and_dynamic_comparison_failures() {
    let source = Rows::new(std::iter::repeat_n(7, 64));
    let syntax = parse_query("SELECT TOP 1 N FROM T ORDER BY N", Default::default()).unwrap();
    let prepared = prepare(syntax.query(), &source).unwrap();
    let mut count = 0;
    let summary = prepared
        .execute_progressive(&source, &[], Default::default(), &mut |row| {
            assert_eq!(source.visits.get(), 64);
            assert_eq!(row, [Value::Int32(7)]);
            count += 1;
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(count, 64);
    assert_eq!(summary.emitted_rows, 64);

    let source = Rows::new([1, 2, 3]);
    let syntax = parse_query(
        "SELECT TOP 1 N FROM T ORDER BY IIf(N=3,CVErr(1),N)",
        Default::default(),
    )
    .unwrap();
    let prepared = prepare(syntax.query(), &source).unwrap();
    let expected = prepared
        .execute(&source, &[], Default::default())
        .err()
        .unwrap();
    let mut emitted = 0;
    let result = prepared.execute_progressive(&source, &[], Default::default(), &mut |_| {
        emitted += 1;
        Ok::<_, ()>(())
    });
    assert!(matches!(result, Err(StreamError::Query(error)) if error == expected));
    assert_eq!(emitted, 0);
    assert_eq!(source.visits.get(), 3);
}

#[test]
fn a_source_swallowing_a_sorted_projection_error_cannot_publish_a_success() {
    let mut source = Rows::new([2, 0, 1]);
    source.swallow = true;
    let syntax = parse_query("SELECT TOP 1 N,1/N FROM T ORDER BY N", Default::default()).unwrap();
    let prepared = prepare(syntax.query(), &source).unwrap();
    let mut emitted = 0;
    let result = prepared.execute_progressive(&source, &[], Default::default(), &mut |_| {
        emitted += 1;
        Ok::<_, ()>(())
    });
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
    assert_eq!(source.visits.get(), 3);
    assert_eq!(emitted, 0);
}
