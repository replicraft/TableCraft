use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn prepared_byte_strings_rebind_source_format_in_nested_expressions() {
    let source = MemorySource::default();
    let query = parse_query(
        "PARAMETERS N DOUBLE; SELECT LeftB(N,4),RightB(N,4),MidB(N,5,2),AscB(N),InStrB(1,N,5,0),(SELECT StrConv(N,0))",
        Default::default(),
    ).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    let parameters = [("N", Value::Double(Double::from_float(12.5)))];
    for (format, suffix, separator, full) in [
        (NumberFormat::EnUs, ".5", ".", "12.5"),
        (NumberFormat::DeDe, ",5", ",", "12,5"),
        (NumberFormat::EnUs, ".5", ".", "12.5"),
    ] {
        assert_eq!(
            plan.execute(
                &source,
                &parameters,
                ExecutionOptions {
                    number_format: Some(format),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            &[vec![
                Value::Text("12".into()),
                Value::Text(suffix.into()),
                Value::Text(separator.into()),
                Value::Int16(49),
                Value::Int32(7),
                Value::Text(full.into()),
            ]]
        );
    }
    assert_eq!(
        plan.execute(&source, &parameters, Default::default()).err(),
        Some(QueryError::Evaluation(EvalError::NumberFormatRequired))
    );
}

#[test]
fn nested_byte_sources_preserve_eager_errors_and_publication_boundaries() {
    let source = MemorySource::default();
    for (sql, expected) in [
        ("SELECT LeftB(123,1)", EvalError::UnrepresentableText),
        ("SELECT InStrB(1,NULL,1/0,0)", EvalError::DivisionByZero),
        ("SELECT StrConv(123,1,1/0)", EvalError::DivisionByZero),
    ] {
        let query = parse_query(sql, Default::default()).unwrap();
        assert_eq!(
            prepare(query.query(), &source)
                .unwrap()
                .execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        number_format: Some(NumberFormat::EnUs),
                        ..Default::default()
                    }
                )
                .err(),
            Some(QueryError::Evaluation(expected)),
            "{sql}"
        );
    }
    let query = parse_query(
        "SELECT AscB(LeftB(123,1)),StrConv(StrConv(1234,128,1033),64,1033)",
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    number_format: Some(NumberFormat::EnUs),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
        &[vec![Value::Int16(49), Value::Text("1234".into())]]
    );
}
