use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::functions::conversions::text::NumberFormat;
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn prepared_scalar_slices_rebind_formats_and_preserve_nested_counts() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS N DOUBLE; SELECT Left(N,2),Right(N,2),(SELECT Mid(N,3)),Mid(N,'2,5','2,5')",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let bindings = [("N", Value::Double(Double::from_float(12.5)))];
    let result = plan
        .execute(
            &source,
            &bindings,
            ExecutionOptions {
                number_format: Some(NumberFormat::DeDe),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        result.rows(),
        &[vec![
            Value::Text("12".into()),
            Value::Text(",5".into()),
            Value::Text(",5".into()),
            Value::Text("2,".into())
        ]]
    );
    let parsed = parse_query("PARAMETERS N DOUBLE; SELECT Right(N,2)", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (format, expected) in [(NumberFormat::EnUs, ".5"), (NumberFormat::DeDe, ",5")] {
        assert_eq!(
            plan.execute(
                &source,
                &bindings,
                ExecutionOptions {
                    number_format: Some(format),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            &[vec![Value::Text(expected.into())]]
        );
    }
    assert!(
        plan.execute(&source, &bindings, Default::default())
            .is_err()
    );
}

#[test]
fn null_sources_preserve_eager_argument_failures_and_no_count_conversion() {
    let source = MemorySource::default();
    for function in ["Left", "Right", "Mid"] {
        let parsed =
            parse_query(&format!("SELECT {function}(NULL,1/0)"), Default::default()).unwrap();
        assert!(matches!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default()),
            Err(QueryError::Evaluation(_))
        ));
        let parsed = parse_query(
            &format!("SELECT {function}(NULL,'bad')"),
            Default::default(),
        )
        .unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default())
                .unwrap()
                .rows(),
            &[vec![Value::Null]]
        );
    }
}
