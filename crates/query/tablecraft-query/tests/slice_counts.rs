use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};

fn run(sql: &str, format: Option<NumberFormat>) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: format,
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn count_coercion_reaches_scalar_nested_and_correlated_query_paths() {
    assert_eq!(
        run(
            "SELECT Left('abcd',1.5),Right('abcd',CDec(2.5)),Mid('abcd',CDate(1.5),CCur(2.5))",
            None
        )
        .unwrap(),
        [vec![
            Value::Text("ab".into()),
            Value::Text("cd".into()),
            Value::Text("bc".into())
        ]]
    );
    assert_eq!(run("SELECT Left('abcd',N), (SELECT Mid('abcd',N,1.5)) FROM (SELECT 1.5 AS N UNION ALL SELECT 3.5) AS A ORDER BY N", None).unwrap(), [vec![Value::Text("ab".into()),Value::Text("bc".into())],vec![Value::Text("abcd".into()),Value::Text("d".into())]]);
    assert_eq!(
        run(
            "SELECT Left('abc','1,5'),Eval(\"Right('abc','2,5')\")",
            Some(NumberFormat::DeDe)
        )
        .unwrap(),
        [vec![Value::Text("ab".into()), Value::Text("bc".into())]]
    );
}

#[test]
fn prepared_text_counts_rebind_values_and_execution_policies() {
    let source = MemorySource::default();
    let query = parse_query(
        "PARAMETERS N TEXT; SELECT Left('abcdef',N),Mid('abcdef',N,'2.5')",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (number, left, mid) in [("1.5", "ab", "bc"), ("3.5", "abcd", "de")] {
        let bindings = [("N", Value::Text(number.into()))];
        let result = plan
            .execute(
                &source,
                &bindings,
                ExecutionOptions {
                    number_format: Some(NumberFormat::EnUs),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            result.rows(),
            [vec![Value::Text(left.into()), Value::Text(mid.into())]]
        );
        assert_eq!(
            plan.execute(&source, &bindings, ExecutionOptions::default())
                .err(),
            Some(EvalError::NumberFormatRequired.into())
        );
    }
}

#[test]
fn eager_source_null_and_unicode_failures_preserve_buffered_error_boundaries() {
    for sql in ["SELECT Left(NULL,1/0)", "SELECT Mid('abc','bad',1/0)"] {
        assert_eq!(
            run(sql, Some(NumberFormat::EnUs)),
            Err(QueryError::Evaluation(EvalError::DivisionByZero))
        );
    }
    assert_eq!(
        run("SELECT Left(NULL,'bad'),Right(NULL,1e40)", None).unwrap(),
        [vec![Value::Null, Value::Null]]
    );
    assert_eq!(
        run("SELECT Left('é🦀中',2.5)", None),
        Err(QueryError::Evaluation(EvalError::UnrepresentableText))
    );
    for sql in [
        "SELECT Left('abc',-1.5)",
        "SELECT Left('abc',2147483647.5)",
        "SELECT Mid('abc',0.5)",
        "SELECT Left('abc','1.5')",
    ] {
        assert!(run(sql, None).is_err(), "{sql}");
    }
}
