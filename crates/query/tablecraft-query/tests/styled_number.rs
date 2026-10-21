use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};
#[test]
fn formatting_profiles_remain_execution_local_in_prepared_grouped_queries() {
    let source = MemorySource::default();
    let query = parse_query("PARAMETERS N LONG; SELECT FormatNumber(N),Count(*) FROM (SELECT 1 AS Id) AS A GROUP BY FormatNumber(N)", ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (format, expected) in [
        (NumberFormat::Invariant, "1234.00"),
        (NumberFormat::EnUs, "1,234.00"),
        (NumberFormat::DeDe, "1.234,00"),
    ] {
        assert_eq!(
            plan.execute(
                &source,
                &[("N", Value::Int32(1234))],
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
    assert_eq!(
        plan.execute(
            &source,
            &[("N", Value::Int32(1234))],
            ExecutionOptions::default()
        )
        .err(),
        Some(QueryError::Evaluation(EvalError::NumberFormatRequired))
    );
}
#[test]
fn dynamic_options_fields_correlations_and_group_identity_are_retained() {
    let source = MemorySource::default();
    let options = || ExecutionOptions {
        number_format: Some(NumberFormat::Invariant),
        ..Default::default()
    };
    for (sql, expected) in [
        (
            "SELECT FormatNumber(A.N,A.P),Count(*) FROM (SELECT 12.345 AS N,2 AS P,999 AS Unused) AS A GROUP BY FormatNumber(A.N,A.P)",
            vec![Value::Text("12.35".into()), Value::Int64(1)],
        ),
        (
            "SELECT (SELECT FormatPercent(A.N,A.P)) FROM (SELECT 2 AS N,0 AS P,999 AS Unused) AS A",
            vec![Value::Text("200%".into())],
        ),
        (
            "SELECT FormatNumber(Max(A.N),0) FROM (SELECT 1234 AS N) AS A",
            vec![Value::Text("1234".into())],
        ),
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        assert_eq!(
            prepare(query.query(), &source)
                .unwrap()
                .execute(&source, &[], options())
                .unwrap()
                .rows(),
            [expected]
        );
    }
    let query = parse_query(
        "SELECT FormatNumber(A.N,3) FROM (SELECT 1 AS N) AS A GROUP BY FormatNumber(A.N,2)",
        ParseLimits::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(query.query(), &source).err(),
        Some(QueryError::Grouping)
    );
}
