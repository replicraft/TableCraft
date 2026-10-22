use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::functions::conversions::text::NumberFormat;
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn prepared_val_calls_rebind_source_formats_without_localizing_prefix_parsing() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS N DOUBLE; SELECT Val(N),(SELECT Val(N)),Val('12.5'),Val(True)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let bindings = [("N", Value::Double(Double::from_float(12.5)))];
    for (format, expected) in [(NumberFormat::EnUs, 12.5), (NumberFormat::DeDe, 12.0)] {
        let value = Value::Double(Double::from_float(expected));
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
            &[vec![
                value.clone(),
                value,
                Value::Double(Double::from_float(12.5)),
                Value::Double(Double::from_float(0.0))
            ]]
        );
    }
    assert!(
        plan.execute(&source, &bindings, Default::default())
            .is_err()
    );
}

#[test]
fn val_preserves_null_and_eager_errors_and_boolean_string_conversion() {
    let source = MemorySource::default();
    for operand in ["NULL", "1/0"] {
        let parsed = parse_query(&format!("SELECT Val({operand})"), Default::default()).unwrap();
        assert!(matches!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default()),
            Err(QueryError::Evaluation(_))
        ));
    }
    let parsed = parse_query("SELECT Val(True)", Default::default()).unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        &[vec![Value::Double(Double::from_float(0.0))]]
    );
}
