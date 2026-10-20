use tablecraft_core::{
    limits::Resource,
    values::{Currency, Value},
};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{ExecutionOptions, QueryError, QueryLimits, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};
#[test]
fn conversion_text_policy_is_execution_local_across_parameters_groups_and_subqueries() {
    let source = MemorySource::default();
    let query = parse_query(
        "PARAMETERS P TEXT; SELECT CCur(P),Sum(CLng(P)) FROM (SELECT 1 AS N) AS T GROUP BY CCur(P)",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (format, expected) in [(NumberFormat::EnUs, 1234), (NumberFormat::DeDe, 1)] {
        let result = plan
            .execute(
                &source,
                &[("P", Value::Text("1,234".into()))],
                ExecutionOptions {
                    number_format: Some(format),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            result.rows(),
            [vec![
                Value::Currency(Currency::from_scaled(if expected == 1 {
                    12340
                } else {
                    12340000
                })),
                Value::Int64(expected)
            ]]
        );
    }
    let nested = parse_query(
        "PARAMETERS P TEXT; SELECT (SELECT CInt(P))",
        ParseLimits::default(),
    )
    .unwrap();
    let nested = prepare(nested.query(), &source).unwrap();
    for (format, expected) in [(NumberFormat::EnUs, 1234), (NumberFormat::DeDe, 1)] {
        assert_eq!(
            nested
                .execute(
                    &source,
                    &[("P", Value::Text("1,234".into()))],
                    ExecutionOptions {
                        number_format: Some(format),
                        ..Default::default()
                    }
                )
                .unwrap()
                .rows(),
            [vec![Value::Int16(expected)]]
        );
    }
    assert_eq!(
        plan.execute(
            &source,
            &[("P", Value::Text("1,234".into()))],
            ExecutionOptions::default()
        )
        .err(),
        Some(QueryError::Evaluation(EvalError::NumberFormatRequired))
    );
}
#[test]
fn full_text_is_validated_eagerly_with_shared_limits() {
    let source = MemorySource::default();
    for sql in [
        "SELECT CLng('12tail')",
        "SELECT IIf(False,CInt('bad'),0)",
        "SELECT CCur('1,23.00')",
        "SELECT CLngLng('9223372036854775808')",
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        assert!(
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
                .is_err(),
            "{sql}"
        );
    }
    let query = parse_query("SELECT CLng('123456789')", ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    assert!(
        matches!(plan.execute(&source,&[],ExecutionOptions{number_format:Some(NumberFormat::Invariant),limits:QueryLimits{steps:10,..Default::default()},..Default::default()}),Err(QueryError::Evaluation(EvalError::Limit(error))) if error.resource==Resource::Steps)
    );
}
