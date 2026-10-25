//! Final grouped expressions consume retained key cells, never representative rows.
use super::*;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};

fn run(sql: &str, source: &Rows, declared: bool) -> Result<Vec<Vec<Value>>, StreamError<()>> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    let prepared = prepare(syntax.query(), source).unwrap();
    let mut rows = Vec::new();
    prepared.execute_progressive(
        source,
        &[],
        ExecutionOptions {
            declared_integral: declared,
            number_format: Some(NumberFormat::Invariant),
            ..Default::default()
        },
        &mut |row| {
            assert_eq!(source.visits.get(), source.rows.len());
            rows.push(row.to_vec());
            Ok(())
        },
    )?;
    Ok(rows)
}

#[test]
fn conditional_keys_reach_scalar_and_retained_builtin_argument_paths() {
    let sql = "SELECT IIf(N>0,N,CLng(0)) AS K,
                      Abs(IIf(N>0,N,CLng(0))),Len(CStr(IIf(N>0,N,CLng(0)))),
                      K,Count(*) FROM T GROUP BY IIf(N>0,N,CLng(0))";
    for declared in [false, true] {
        let source = Rows::new([2, -1, 2, 0]);
        assert_eq!(
            run(sql, &source, declared).unwrap(),
            [
                vec![
                    Value::Int32(2),
                    Value::Int32(2),
                    Value::Int32(1),
                    Value::Int32(2),
                    Value::Int64(2)
                ],
                vec![
                    Value::Int32(0),
                    Value::Int32(0),
                    Value::Int32(1),
                    Value::Int32(0),
                    Value::Int64(2)
                ],
            ]
        );
    }
}

#[test]
fn active_declared_addition_key_is_reused_inside_numeric_and_string_arguments() {
    let sql = "SELECT CInt(N)+CInt(1) AS K,Abs(CInt(N)+CInt(1)),
                      CStr(CInt(N)+CInt(1)),Count(*) FROM T GROUP BY CInt(N)+CInt(1)";
    let source = Rows::new([2, 0, 2, -1]);
    assert_eq!(
        run(sql, &source, true).unwrap(),
        [
            vec![
                Value::Int16(3),
                Value::Int16(3),
                Value::Text("3".into()),
                Value::Int64(2)
            ],
            vec![
                Value::Int16(1),
                Value::Int16(1),
                Value::Text("1".into()),
                Value::Int64(1)
            ],
            vec![
                Value::Int16(0),
                Value::Int16(0),
                Value::Text("0".into()),
                Value::Int64(1)
            ],
        ]
    );
    let source = Rows::new([2, 0]);
    assert!(matches!(
        run(sql, &source, false),
        Err(StreamError::Query(QueryError::Unsupported))
    ));
    assert_eq!(source.scans.get(), 0);
}

#[test]
fn top_omitted_groups_still_evaluate_final_errors_after_checked_eof() {
    let source = Rows::new([1, 1, 2]);
    let syntax = parse_query(
        "SELECT TOP 1 N,1/(Count(*)-1) FROM T GROUP BY N",
        Default::default(),
    )
    .unwrap();
    let prepared = prepare(syntax.query(), &source).unwrap();
    let mut rows = Vec::new();
    let result = prepared.execute_progressive(&source, &[], Default::default(), &mut |row| {
        assert_eq!(source.visits.get(), 3);
        rows.push(row.to_vec());
        Ok::<(), ()>(())
    });
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0], Value::Int32(1));
}

#[test]
fn empty_groups_skip_final_expressions_and_null_keys_retain_one_population() {
    let source = Rows::new([3, 1, 3]);
    assert!(
        run("SELECT 1/0 FROM T WHERE False GROUP BY N", &source, false)
            .unwrap()
            .is_empty()
    );
    let source = Rows::new([3, 1, 3]);
    assert_eq!(
        run("SELECT N FROM T GROUP BY N", &source, false).unwrap(),
        [vec![Value::Int32(3)], vec![Value::Int32(1)]]
    );
    let source = Rows::new([3, 1, 3]);
    assert_eq!(
        run(
            "SELECT IIf(True,NULL,N) AS K,Count(*) FROM T GROUP BY IIf(True,NULL,N)",
            &source,
            false
        )
        .unwrap(),
        [vec![Value::Null, Value::Int64(3)]]
    );
}
