use super::*;
use tablecraft_expressions::EvalError;

#[test]
fn global_count_evaluates_repeated_and_aliased_outputs_only_after_eof() {
    let source = Rows::new([1, 2, 3]);
    let syntax = parse_query(
        "SELECT Count(*) AS Population,Population+CLngLng(1) AS Adjusted,
         Count(*) AS Repeated FROM T WHERE N>1",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    let mut rows = Vec::new();
    let summary = plan
        .execute_progressive(&source, &[], Default::default(), &mut |row: &[Value]| {
            assert_eq!(source.visits.get(), 3);
            rows.push(row.to_vec());
            Ok::<(), ()>(())
        })
        .unwrap();
    assert_eq!(summary.scanned_rows, 3);
    assert_eq!(summary.emitted_rows, 1);
    assert_eq!(
        rows,
        [vec![Value::Int64(2), Value::Int64(3), Value::Int64(2)]]
    );
    assert_eq!(source.scans.get(), 1);
}

#[test]
fn count_and_row_union_arms_use_their_own_counts_and_common_carriers() {
    let source = Rows::new([1, 2, 3]);
    let syntax = parse_query(
        "SELECT Count(*) AS V FROM T UNION ALL SELECT N FROM T
         UNION ALL SELECT Count(*) FROM T WHERE N>1",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let mut rows = Vec::new();
    let summary = plan
        .execute_progressive(&source, &[], Default::default(), &mut |row: &[Value]| {
            rows.push(row.to_vec());
            Ok::<(), ()>(())
        })
        .unwrap();
    assert_eq!(rows, [3, 1, 2, 3, 2].map(|count| vec![Value::Int64(count)]));
    assert_eq!(summary.scanned_rows, 9);
    assert_eq!(summary.emitted_rows, 5);
    assert_eq!(source.scans.get(), 3);
}

#[test]
fn filtered_empty_count_projection_errors_are_not_skipped_and_bad_eof_wins() {
    let syntax = parse_query("SELECT 1/Count(*) FROM T WHERE False", Default::default()).unwrap();
    for bad_eof in [false, true] {
        let mut source = Rows::new([1, 2, 3]);
        source.eof_error = bad_eof;
        let plan = prepare(syntax.query(), &source).unwrap();
        let mut calls = 0;
        let result = plan.execute_progressive(&source, &[], Default::default(), &mut |_| {
            calls += 1;
            Ok::<(), ()>(())
        });
        let expected = if bad_eof {
            QueryError::Source
        } else {
            QueryError::Evaluation(EvalError::DivisionByZero)
        };
        assert!(matches!(result, Err(StreamError::Query(error)) if error == expected));
        assert_eq!(source.visits.get(), 3);
        assert_eq!(calls, 0);
    }
}

#[test]
fn unsupported_aggregate_arms_fail_before_even_an_earlier_valid_count_scan() {
    for sql in [
        "SELECT First(N) FROM T",
        "SELECT Count(*) FROM T GROUP BY CDbl(N)",
        "SELECT Count(*) FROM T HAVING Last(N)>0",
        "SELECT Count(*) FROM T UNION ALL SELECT Last(N) FROM T",
        "SELECT N FROM T UNION ALL SELECT First(N) FROM T WHERE False",
    ] {
        let source = Rows::new([1, 2, 3]);
        let syntax = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(syntax.query(), &source).unwrap();
        let mut calls = 0;
        let result = plan.execute_progressive(&source, &[], Default::default(), &mut |_| {
            calls += 1;
            Ok::<(), ()>(())
        });
        assert!(
            matches!(result, Err(StreamError::Query(QueryError::Unsupported))),
            "{sql}"
        );
        assert_eq!(source.scans.get(), 0);
        assert_eq!(calls, 0);
    }
}

#[test]
fn count_literal_text_remains_a_value_instead_of_dynamic_code_or_a_field_name() {
    let source = Rows::new([1, 2, 3]);
    let syntax = parse_query(
        "SELECT Count('MissingField'),Count('1/0'),Count(''),Count(NULL) FROM T",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let mut rows = Vec::new();
    plan.execute_progressive(&source, &[], Default::default(), &mut |row: &[Value]| {
        rows.push(row.to_vec());
        Ok::<(), ()>(())
    })
    .unwrap();
    assert_eq!(
        rows,
        [vec![
            Value::Int64(3),
            Value::Int64(3),
            Value::Int64(3),
            Value::Int64(0)
        ]]
    );
}

#[test]
fn streaming_count_arguments_follow_row_then_written_occurrence_error_order() {
    let source = Rows::new([1, 2, 3]);
    let syntax = parse_query(
        "SELECT Count(1/(3-N)) AS LateZero,Count(CByte(256/N)) AS EarlyOverflow FROM T",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    assert_eq!(
        plan.execute(&source, &[], Default::default()).err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero)),
    );
    let mut calls = 0;
    let streamed = plan.execute_progressive(&source, &[], Default::default(), &mut |_| {
        calls += 1;
        Ok::<(), ()>(())
    });
    assert!(matches!(
        streamed,
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::Overflow
        )))
    ));
    assert_eq!(source.visits.get(), 1);
    assert_eq!(calls, 0);
}
