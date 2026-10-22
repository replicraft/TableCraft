use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::functions::{
    conversions::text::NumberFormat, text::casing::CaseLocale,
};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn prepared_trimming_and_casing_rebind_formats_and_keep_nested_context() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS N DOUBLE; SELECT LTrim(N),RTrim(N),Trim(N),(SELECT UCase(N)),LCase(True)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let bindings = [("N", Value::Double(Double::from_float(12.5)))];
    let locale: CaseLocale = "en-US".parse().unwrap();
    for (format, text) in [(NumberFormat::EnUs, "12.5"), (NumberFormat::DeDe, "12,5")] {
        let mut expected = vec![Value::Text(text.into()); 4];
        expected.push(Value::Text("true".into()));
        assert_eq!(
            plan.execute(
                &source,
                &bindings,
                ExecutionOptions {
                    number_format: Some(format),
                    case_locale: Some(&locale),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            &[expected]
        );
    }
    assert!(
        plan.execute(&source, &bindings, Default::default())
            .is_err()
    );
}

#[test]
fn null_and_eager_failures_do_not_infer_missing_providers() {
    let source = MemorySource::default();
    for function in ["LTrim", "RTrim", "Trim", "LCase", "UCase"] {
        let parsed = parse_query(&format!("SELECT {function}(NULL)"), Default::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default())
                .unwrap()
                .rows(),
            &[vec![Value::Null]]
        );
        let parsed = parse_query(&format!("SELECT {function}(1/0)"), Default::default()).unwrap();
        assert!(matches!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default()),
            Err(QueryError::Evaluation(_))
        ));
    }
}
