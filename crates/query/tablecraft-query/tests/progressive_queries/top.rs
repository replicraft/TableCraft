//! TOP caps emitted rows without skipping input validation or eager computation.
use super::*;
use tablecraft_expressions::EvalError;
use tablecraft_query::QueryLimits;

#[test]
fn unordered_top_counts_match_buffered_prefixes_and_consume_every_input_row() {
    for values in [vec![], vec![1], vec![1, 2, 2, 3, 4]] {
        for count in [1, 2, 10] {
            for filter in ["True", "N>=2", "False"] {
                let sql = format!("SELECT TOP {count} N,N+1 AS V FROM T WHERE {filter}");
                let source = Rows::new(values.clone());
                let parsed = parse_query(&sql, Default::default()).unwrap();
                let plan = prepare(parsed.query(), &source).unwrap();
                for declared in [false, true] {
                    let options = ExecutionOptions {
                        declared_integral: declared,
                        ..Default::default()
                    };
                    let expected = plan.execute(&source, &[], options).unwrap();
                    source.visits.set(0);
                    let mut output = Vec::new();
                    let summary = plan
                        .execute_progressive(&source, &[], options, &mut |row| {
                            output.push(row.to_vec());
                            Ok::<_, ()>(())
                        })
                        .unwrap();
                    assert_eq!(output, expected.rows(), "{sql}");
                    assert_eq!(summary.scanned_rows, values.len() as u64);
                    assert_eq!(summary.emitted_rows, expected.rows().len() as u64);
                    assert_eq!(source.visits.get(), values.len());
                }
            }
        }
    }
}

#[test]
fn top_does_not_hide_eager_filter_or_projection_failures_after_its_output_prefix() {
    for sql in [
        "SELECT TOP 1 N,1/N FROM T",
        "SELECT TOP 1 IIf(True,N,1/N) FROM T",
        "SELECT TOP 1 N FROM T WHERE True OR 1/N=0",
    ] {
        let source = Rows::new([1, 0, 3]);
        let parsed = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        let calls = Cell::new(0);
        let result = plan.execute_progressive(&source, &[], Default::default(), &mut |_| {
            calls.set(calls.get() + 1);
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
        assert_eq!(calls.get(), 1);
        assert_eq!(source.visits.get(), 2);
    }
}

#[test]
fn discarded_raw_strings_are_evaluated_but_not_strictly_published() {
    for raw in ["ChrW(55296)", "ChrB(65)"] {
        let sql = format!("SELECT TOP 1 IIf(N=1,'kept',{raw}) AS S FROM T");
        let source = Rows::new([1, 2, 3]);
        let parsed = parse_query(&sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        let expected = plan.execute(&source, &[], Default::default()).unwrap();
        let mut output = Vec::new();
        let summary = plan
            .execute_progressive(&source, &[], Default::default(), &mut |row| {
                output.push(row.to_vec());
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(output, expected.rows());
        assert_eq!(output, [vec![Value::Text("kept".into())]]);
        assert_eq!(summary.scanned_rows, 3);
        assert_eq!(summary.emitted_rows, 1);
    }
}

#[test]
fn malformed_later_rows_and_failed_eof_still_fail_after_top_is_satisfied() {
    let mut source = Rows::new([1, 2, 3]);
    let parsed = parse_query("SELECT TOP 1 N FROM T", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    source.rows[1][0] = Value::Text("invalid".into());
    let calls = Cell::new(0);
    let result = plan.execute_progressive(&source, &[], Default::default(), &mut |_| {
        calls.set(calls.get() + 1);
        Ok::<_, ()>(())
    });
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Schema(_)))
    ));
    assert_eq!(calls.get(), 1);

    let mut source = Rows::new([1, 2, 3]);
    source.eof_error = true;
    let plan = prepare(parsed.query(), &source).unwrap();
    let calls = Cell::new(0);
    let result = plan.execute_progressive(&source, &[], Default::default(), &mut |_| {
        calls.set(calls.get() + 1);
        Ok::<_, ()>(())
    });
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Source))
    ));
    assert_eq!(calls.get(), 1);
    assert_eq!(source.visits.get(), 3);
}

#[test]
fn omitted_rows_keep_cumulative_query_limits_and_cancellation_checkpoints() {
    let source = Rows::new([1, 2, 3]);
    let parsed = parse_query("SELECT TOP 1 N+1 AS V FROM T", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let checkpoints = Cell::new(0);
    let emissions = Cell::new(0);
    let first_emission = Cell::new(0);
    plan.execute_progressive(
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&|| {
                checkpoints.set(checkpoints.get() + 1);
                false
            }),
            ..Default::default()
        },
        &mut |_| {
            emissions.set(emissions.get() + 1);
            first_emission.set(checkpoints.get());
            Ok::<_, ()>(())
        },
    )
    .unwrap();
    assert_eq!(emissions.get(), 1);
    assert!(checkpoints.get() > first_emission.get());

    for stop in first_emission.get() + 1..=checkpoints.get() {
        let current = Cell::new(0);
        let calls = Cell::new(0);
        let result = plan.execute_progressive(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&|| {
                    current.set(current.get() + 1);
                    current.get() == stop
                }),
                ..Default::default()
            },
            &mut |_| {
                calls.set(calls.get() + 1);
                Ok::<_, ()>(())
            },
        );
        assert!(
            matches!(
                result,
                Err(StreamError::Query(
                    QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled)
                ))
            ),
            "{stop}"
        );
        assert_eq!(calls.get(), 1);
    }

    let calls = Cell::new(0);
    let result = plan.execute_progressive(
        &source,
        &[],
        ExecutionOptions {
            limits: QueryLimits {
                rows: 4,
                ..Default::default()
            },
            ..Default::default()
        },
        &mut |_| {
            calls.set(calls.get() + 1);
            Ok::<_, ()>(())
        },
    );
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Limit(_)))
    ));
    assert_eq!(calls.get(), 1);
}

#[test]
fn zero_top_counts_keep_the_existing_metadata_only_binder_refusal() {
    for sql in ["SELECT TOP 0 N FROM T", "SELECT TOP 0 PERCENT N FROM T"] {
        let source = Rows::new([1]);
        let parsed = parse_query(sql, Default::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source).err(),
            Some(QueryError::Unsupported)
        );
        assert_eq!(source.scans.get(), 0);
    }
}
