use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn explicit_numeric_text_formats_reuse_parameter_plans_and_compose_through_sources() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS P TEXT; SELECT Hex(P),Oct(P),Str(P)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (format, input) in [(NumberFormat::EnUs, "2.5"), (NumberFormat::DeDe, "2,5")] {
        let result = plan
            .execute(
                &source,
                &[("P", Value::Text(input.into()))],
                ExecutionOptions {
                    number_format: Some(format),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            result.rows(),
            &[vec![
                Value::Text("2".into()),
                Value::Text("2".into()),
                Value::Text(" 2.5".into())
            ]]
        );
    }
    let parsed = parse_query(
        "SELECT Hex(Q.N),Str(Q.N) FROM (SELECT '2,5' AS N) AS Q",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert!(matches!(
        plan.execute(&source, &[], Default::default()),
        Err(QueryError::Evaluation(EvalError::NumberFormatRequired))
    ));
    let result = plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: Some(NumberFormat::DeDe),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        result.rows(),
        &[vec![Value::Text("2".into()), Value::Text(" 2.5".into())]]
    );
}
