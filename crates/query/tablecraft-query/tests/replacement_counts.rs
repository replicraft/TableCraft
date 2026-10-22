use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};

fn run(sql: &str, format: Option<NumberFormat>) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let query = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(query.query(), &source)?
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
fn date_and_contextual_text_counts_reach_suffixes_and_closed_dynamic_expressions() {
    assert_eq!(run("SELECT Replace('banana','an','X',CDate(2.5),CDate(-1.25)),Replace('banana','an','X',CDate(2.5),CDate(-0.5))", None).unwrap(), [vec![Value::Text("XXa".into()),Value::Text("anana".into())]]);
    assert_eq!(run("SELECT Replace('banana','an','X','2,5','2,5'),Eval(\"Replace('aaaa','a','aa','1,5','2,5')\")", Some(NumberFormat::DeDe)).unwrap(), [vec![Value::Text("XXa".into()),Value::Text("aaaaa".into())]]);
    assert_eq!(
        run(
            "SELECT Replace('banana','an','X',,'0'),Replace('banana','an','X','2')",
            Some(NumberFormat::EnUs)
        )
        .unwrap(),
        [vec![
            Value::Text("banana".into()),
            Value::Text("XXa".into())
        ]]
    );
}

#[test]
fn prepared_parameters_rebind_both_positions_and_number_policy() {
    let source = MemorySource::default();
    let query = parse_query(
        "PARAMETERS StartAt TEXT,LimitTo TEXT; SELECT Replace('banana','an','X',StartAt,LimitTo)",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (start, count, expected) in [("2.5", "2.5", "XXa"), ("3.5", "1.5", "Xa")] {
        let bindings = [
            ("StartAt", Value::Text(start.into())),
            ("LimitTo", Value::Text(count.into())),
        ];
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
        assert_eq!(result.rows(), [vec![Value::Text(expected.into())]]);
        assert_eq!(
            plan.execute(&source, &bindings, ExecutionOptions::default())
                .err(),
            Some(EvalError::NumberFormatRequired.into())
        );
    }
}

#[test]
fn optional_conversion_precedence_and_eager_errors_never_publish_success() {
    for sql in [
        "SELECT Replace(NULL,'a','b','bad',1/0)",
        "SELECT Replace('','a',1/0,'0',0)",
    ] {
        assert_eq!(
            run(sql, Some(NumberFormat::EnUs)),
            Err(QueryError::Evaluation(EvalError::DivisionByZero))
        );
    }
    for sql in [
        "SELECT Replace('abc','a','b','1')",
        "SELECT Replace('abc','a','b',1,'1')",
    ] {
        assert_eq!(
            run(sql, None),
            Err(QueryError::Evaluation(EvalError::NumberFormatRequired))
        );
    }
    assert_eq!(
        run(
            "SELECT Replace(NULL,'a','b','bad',0)",
            Some(NumberFormat::EnUs)
        ),
        Err(QueryError::Evaluation(EvalError::TypeMismatch))
    );
    assert_eq!(
        run(
            "SELECT Replace(NULL,'a','b','1',0)",
            Some(NumberFormat::EnUs)
        ),
        Err(QueryError::Evaluation(EvalError::InvalidNullConversion))
    );
    assert_eq!(
        run(
            "SELECT Replace('é🦀a','a','x','2,5',0)",
            Some(NumberFormat::DeDe)
        )
        .unwrap(),
        [vec![Value::Text("🦀a".into())]]
    );
    assert!(
        run(
            "SELECT Replace('é🦀a','a','x','3',0)",
            Some(NumberFormat::EnUs)
        )
        .is_err()
    );
}
