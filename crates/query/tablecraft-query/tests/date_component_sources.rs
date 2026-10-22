use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError,
    functions::{conversions::text::NumberFormat, dates::parsing::DateTextFormat},
};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};

#[test]
fn date_component_sources_rebind_and_share_policies_with_nested_eval() {
    let source = MemorySource::default();
    let query = parse_query(
        concat!(
            "PARAMETERS D TEXT; SELECT Year(D),Month(D),Day(D),Hour(D),Minute(D),Second(D),",
            "(SELECT Hour(D)),Eval('Hour(45658.5)')"
        ),
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (profile, text, expected) in [
        (
            NumberFormat::EnUs,
            "45658.5",
            [2025, 1, 1, 12, 0, 0, 12, 12],
        ),
        (NumberFormat::DeDe, "-1,25", [1899, 12, 29, 6, 0, 0, 6, 12]),
    ] {
        let bindings = [("D", Value::Text(text.into()))];
        let options = ExecutionOptions {
            date_format: Some(DateTextFormat::Iso),
            number_format: Some(profile),
            ..Default::default()
        };
        assert_eq!(
            plan.execute(&source, &bindings, options).unwrap().rows(),
            [expected.into_iter().map(Value::Int16).collect::<Vec<_>>()]
        );
        assert_eq!(
            plan.execute(&source, &bindings, ExecutionOptions::default())
                .err(),
            Some(QueryError::Evaluation(EvalError::DateFormatRequired))
        );
    }
}

#[test]
fn source_conversion_works_in_filters_groups_and_correlations() {
    let source = MemorySource::default();
    for (sql, expected) in [
        (
            "SELECT Year(D),Count(*) FROM (SELECT '2024-02-29' AS D,999 AS Unused) AS A WHERE Day(D)=29 GROUP BY Year(D)",
            vec![Value::Int16(2024), Value::Int64(1)],
        ),
        (
            "SELECT (SELECT Month(A.D)) FROM (SELECT '2024-02-29' AS D,999 AS Unused) AS A",
            vec![Value::Int16(2)],
        ),
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        let options = ExecutionOptions {
            date_format: Some(DateTextFormat::Iso),
            ..Default::default()
        };
        assert_eq!(
            prepare(query.query(), &source)
                .unwrap()
                .execute(&source, &[], options)
                .unwrap()
                .rows(),
            [expected]
        );
    }
}
