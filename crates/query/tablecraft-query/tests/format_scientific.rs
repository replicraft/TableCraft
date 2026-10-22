use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{ExecutionOptions, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn scientific_formats_compose_and_reused_plans_keep_execution_profiles() {
    let source = MemorySource::default();
    for (sql, expected) in [
        (
            "SELECT Len(Format(A.N,'0.00E+00')),Count(*) FROM (SELECT 12 AS N UNION ALL SELECT 23) AS A GROUP BY Len(Format(A.N,'0.00E+00'))",
            vec![Value::Int32(8), Value::Int64(2)],
        ),
        (
            "SELECT (SELECT Format(A.N,'0,.00')) FROM (SELECT 123456 AS N) AS A",
            vec![Value::Text("123.46".into())],
        ),
        (
            "SELECT Format(Sum(A.N),'0.00E+00') FROM (SELECT 12 AS N UNION ALL SELECT 23) AS A",
            vec![Value::Text("3.50E+01".into())],
        ),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    number_format: Some(NumberFormat::EnUs),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.rows(), [expected]);
    }
    let parsed = parse_query(
        "PARAMETERS P LONG,M TEXT; SELECT Format(P,M)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (profile, mask, expected) in [
        (Some(NumberFormat::EnUs), "0.00E+00", Some("1.23E+04")),
        (Some(NumberFormat::DeDe), "0.00E+00", Some("1,23E+04")),
        (Some(NumberFormat::EnUs), "0,.00", Some("12.35")),
        (None, "0,.00", None),
    ] {
        let result = plan.execute(
            &source,
            &[("P", Value::Int32(12345)), ("M", Value::Text(mask.into()))],
            ExecutionOptions {
                number_format: profile,
                ..Default::default()
            },
        );
        if let Some(expected) = expected {
            assert_eq!(result.unwrap().rows(), [vec![Value::Text(expected.into())]]);
        } else {
            assert_eq!(result.err(), Some(EvalError::NumberFormatRequired.into()));
        }
    }
}
