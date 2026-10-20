use tablecraft_core::values::Value;
use tablecraft_expressions::functions::{
    conversions::text::NumberFormat, text::cstr::DateTextFormat,
};
use tablecraft_query::{ExecutionOptions, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};
#[test]
fn cstr_profiles_remain_execution_local_across_parameters_grouping_and_subqueries() {
    let source = MemorySource::default();
    let query = parse_query(
        "PARAMETERS P DOUBLE; SELECT CStr(P),Count(*) FROM (SELECT 1 AS N) AS T GROUP BY CStr(P)",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (format, expected) in [
        (NumberFormat::Invariant, "1.25"),
        (NumberFormat::EnUs, "1.25"),
        (NumberFormat::DeDe, "1,25"),
    ] {
        assert_eq!(
            plan.execute(
                &source,
                &[(
                    "P",
                    Value::Double(tablecraft_core::values::Double::from_float(1.25))
                )],
                ExecutionOptions {
                    number_format: Some(format),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            [vec![Value::Text(expected.into()), Value::Int64(1)]]
        );
    }
    let query = parse_query(
        "PARAMETERS P DATE; SELECT CStr(P),(SELECT CStr(P))",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    let date =
        Value::DateTime(tablecraft_core::values::LegacyDateTime::from_ole_days(1.0).unwrap());
    for (format, expected) in [
        (DateTextFormat::Iso, "1899-12-31"),
        (DateTextFormat::EnUs, "12/31/1899"),
        (DateTextFormat::DeDe, "31.12.1899"),
    ] {
        assert_eq!(
            plan.execute(
                &source,
                &[("P", date.clone())],
                ExecutionOptions {
                    date_format: Some(format),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            [vec![
                Value::Text(expected.into()),
                Value::Text(expected.into())
            ]]
        );
    }
    assert!(
        plan.execute(&source, &[("P", date)], ExecutionOptions::default())
            .is_err()
    );
}
