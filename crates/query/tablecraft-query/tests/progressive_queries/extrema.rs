//! Retained extrema use the existing kernels and publish only after checked EOF.
use super::*;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};

fn stream(sql: &str, source: &Rows, declared: bool) -> Result<Vec<Vec<Value>>, StreamError<()>> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), source).unwrap();
    let mut output = Vec::new();
    let initial_scans = source.scans.get();
    let initial_visits = source.visits.get();
    let summary = plan.execute_progressive(
        source,
        &[],
        ExecutionOptions {
            declared_integral: declared,
            number_format: Some(NumberFormat::Invariant),
            ..Default::default()
        },
        &mut |row| {
            assert_eq!(
                source.visits.get(),
                initial_visits + source.rows.len() * (source.scans.get() - initial_scans)
            );
            output.push(row.to_vec());
            Ok(())
        },
    )?;
    assert_eq!(summary.emitted_rows as usize, output.len());
    Ok(output)
}

#[test]
fn selected_cells_retain_numeric_carriers_and_unknown_aggregate_declarations() {
    for declared in [false, true] {
        let source = Rows::new([3, 1, 2]);
        let sql = "SELECT Min(N) AS Low,Max(N) AS High,Low+High AS Total,
                          Count(*),Sum(N),Avg(N),Round(Min(N),Count(NULL)) FROM T";
        let syntax = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(syntax.query(), &source).unwrap();
        let options = ExecutionOptions {
            declared_integral: declared,
            ..Default::default()
        };
        let buffered = plan.execute(&source, &[], options).unwrap();
        assert_eq!(stream(sql, &source, declared).unwrap(), buffered.rows());
        assert_eq!(
            buffered.rows()[0][..3],
            [Value::Int32(1), Value::Int32(3), Value::Int32(4)]
        );
        assert_eq!(source.scans.get(), 2);
    }

    let source = Rows::new([1, 2]);
    let sql = "SELECT Min(N)+'2' FROM T";
    assert!(matches!(
        stream(sql, &source, true),
        Err(StreamError::Query(QueryError::ArithmeticDeclaration))
    ));
}

#[test]
fn raw_string_and_error_results_are_retained_until_the_owning_consumer() {
    for declared in [false, true] {
        for (argument, expected) in [("ChrW(55296)", 2), ("ChrB(65)", 1)] {
            let source = Rows::new([1]);
            let sql = format!("SELECT LenB(Min({argument})),LenB(Max({argument})) FROM T");
            assert_eq!(
                stream(&sql, &source, declared).unwrap(),
                [vec![Value::Int32(expected), Value::Int32(expected)]]
            );
            let source = Rows::new([1]);
            assert!(stream(&format!("SELECT Min({argument}) FROM T"), &source, declared).is_err());
        }
        let source = Rows::new([7, 0, 65535]);
        assert_eq!(
            stream(
                "SELECT CLng(Min(CVErr(N))),CLng(Max(CVErr(N))) FROM T",
                &source,
                declared
            )
            .unwrap(),
            [vec![Value::Int32(0), Value::Int32(65535)]]
        );
        let source = Rows::new([7]);
        assert!(stream("SELECT Min(CVErr(N)) FROM T", &source, declared).is_err());
        let source = Rows::new([65536]);
        assert!(stream("SELECT LenB(Min(CVErr(N))) FROM T", &source, declared).is_err());
    }
}

#[test]
fn empty_filtered_and_union_arms_keep_the_existing_one_row_and_promotion_contracts() {
    for values in [vec![], vec![1, 2, 3]] {
        let source = Rows::new(values);
        assert_eq!(
            stream(
                "SELECT Min(N),Max(N),Count(*) FROM T WHERE False",
                &source,
                false
            )
            .unwrap(),
            [vec![Value::Null, Value::Null, Value::Int64(0)]]
        );
    }
    let sql = "SELECT Min(N) AS V FROM T UNION ALL SELECT Max(N) FROM T";
    let source = Rows::new([3, 1, 2]);
    assert_eq!(
        stream(sql, &source, false).unwrap(),
        [vec![Value::Int32(1)], vec![Value::Int32(3)]]
    );
    assert_eq!(source.scans.get(), 2);
}

#[test]
fn later_errors_and_checked_eof_never_publish_an_unfinished_extremum_or_next_arm() {
    let sql = "SELECT Min(1/(3-N)) FROM T UNION ALL SELECT Max(N) FROM T";
    let source = Rows::new([1, 2, 3]);
    assert!(matches!(
        stream(sql, &source, false),
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
    assert_eq!(source.scans.get(), 1);
    assert_eq!(source.visits.get(), 3);

    let mut source = Rows::new([1, 2, 3]);
    source.eof_error = true;
    assert!(matches!(
        stream(
            "SELECT Min(N) FROM T UNION ALL SELECT Max(N) FROM T",
            &source,
            false
        ),
        Err(StreamError::Query(QueryError::Source))
    ));
    assert_eq!(source.scans.get(), 1);
    assert_eq!(source.visits.get(), 3);
}
