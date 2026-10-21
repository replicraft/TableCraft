use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError,
    functions::{
        conversions::text::NumberFormat,
        dates::parsing::{DateTextFormat, ReferenceYear},
    },
};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};
#[test]
fn named_format_fields_groups_correlations_and_modes_remain_distinct() {
    let source = MemorySource::default();
    let sql = "SELECT FormatDateTime(A.Stamp,A.Mode),Count(*) FROM (SELECT #2026-10-20 16:28:14# AS Stamp,4 AS Mode,999 AS Unused) AS A GROUP BY FormatDateTime(A.Stamp,A.Mode)";
    let query = parse_query(sql, ParseLimits::default()).unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap()
            .rows(),
        [vec![Value::Text("16:28".into()), Value::Int64(1)]]
    );
    let query = parse_query("SELECT (SELECT FormatDateTime(A.Stamp,4)) FROM (SELECT #2026-10-20 16:28:14# AS Stamp,999 AS Unused) AS A", ParseLimits::default()).unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap()
            .rows(),
        [vec![Value::Text("16:28".into())]]
    );
    let query = parse_query("SELECT FormatDateTime(A.Stamp,3) FROM (SELECT #2026-10-20# AS Stamp) AS A GROUP BY FormatDateTime(A.Stamp,4)", ParseLimits::default()).unwrap();
    assert_eq!(
        prepare(query.query(), &source).err(),
        Some(QueryError::Grouping)
    );
}
#[test]
fn prepared_text_conversion_and_output_policies_do_not_leak_between_executions() {
    let source = MemorySource::default();
    let query = parse_query(
        "PARAMETERS P TEXT; SELECT FormatDateTime(P,2)",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (profile, expected) in [
        (DateTextFormat::EnUs, "2/29/2024"),
        (DateTextFormat::DeDe, "29.02.2024"),
        (DateTextFormat::Iso, "2024-02-29"),
    ] {
        assert_eq!(
            plan.execute(
                &source,
                &[("P", Value::Text("Feb 29".into()))],
                ExecutionOptions {
                    date_format: Some(profile),
                    reference_year: Some(ReferenceYear::new(2024).unwrap()),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            [vec![Value::Text(expected.into())]]
        );
    }
    assert_eq!(
        plan.execute(
            &source,
            &[("P", Value::Text("Feb 29".into()))],
            ExecutionOptions {
                date_format: Some(DateTextFormat::EnUs),
                ..Default::default()
            }
        )
        .err(),
        Some(QueryError::Evaluation(EvalError::ReferenceYearRequired))
    );
    let query = parse_query(
        "PARAMETERS P TEXT; SELECT FormatDateTime(P,4)",
        ParseLimits::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[("P", Value::Text("-1.25".into()))],
                ExecutionOptions {
                    date_format: Some(DateTextFormat::Iso),
                    number_format: Some(NumberFormat::Invariant),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
        [vec![Value::Text("06:00".into())]]
    );
}
