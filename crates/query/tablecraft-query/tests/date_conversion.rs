use tablecraft_core::values::{LegacyDateTime, Value};
use tablecraft_expressions::{
    EvalError,
    functions::dates::parsing::{DateTextFormat, ReferenceYear},
};
use tablecraft_query::{
    ExecutionOptions, QueryError, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};
fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "Items".into(),
            vec![
                Column::new("Unused".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Label".into(), ScalarType::Text, true).unwrap(),
            ],
            vec![
                vec![
                    Value::Int32(99),
                    Value::Int32(1),
                    Value::Text("2026-10-20 16:35:17".into()),
                ],
                vec![
                    Value::Int32(99),
                    Value::Int32(2),
                    Value::Text("2026-10-20".into()),
                ],
                vec![Value::Int32(99), Value::Int32(3), Value::Null],
            ],
        )
        .unwrap(),
    ])
    .unwrap()
}
fn run(sql: &str, source: &MemorySource) -> Result<Vec<Vec<Value>>, QueryError> {
    let query = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(query.query(), source)?
        .execute(
            source,
            &[],
            ExecutionOptions {
                date_format: Some(DateTextFormat::Iso),
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}
#[test]
fn date_calls_preserve_fields_groups_aggregates_and_correlations() {
    let source = source();
    assert_eq!(
        run(
            "SELECT Id FROM Items WHERE Hour(TimeValue(Label))=16 ORDER BY Id",
            &source
        )
        .unwrap(),
        [vec![Value::Int32(1)]]
    );
    let groups = run(
        "SELECT DateValue(Label),Count(*) FROM Items GROUP BY DateValue(Label)",
        &source,
    )
    .unwrap();
    assert!(groups.contains(&vec![Value::Null, Value::Int64(1)]));
    assert!(groups.contains(&vec![
        Value::DateTime(tablecraft_expressions::datetime::parse_literal("2026-10-20").unwrap()),
        Value::Int64(2)
    ]));
    assert_eq!(run("SELECT A.Id FROM Items AS A WHERE (SELECT DateValue(A.Label))=#2026-10-20# ORDER BY A.Id",&source).unwrap(),[vec![Value::Int32(1)],vec![Value::Int32(2)]]);
    assert_eq!(
        run("SELECT DateValue(Max(Id)) FROM Items", &source).unwrap(),
        [vec![Value::DateTime(
            LegacyDateTime::from_ole_days(3.0).unwrap()
        )]]
    );
    assert_eq!(
        run("SELECT DateValue(Label) FROM Items GROUP BY Id", &source).err(),
        Some(QueryError::Grouping)
    );
}
#[test]
fn date_reference_year_is_per_execution_and_invalid_component_errors_are_eager() {
    let source = source();
    let query = parse_query(
        "PARAMETERS P TEXT; SELECT Year(DateValue(P)),(SELECT Year(CDate(P)))",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for year in [2024, 2028, 2024] {
        assert_eq!(
            plan.execute(
                &source,
                &[("P", Value::Text("Feb 29".into()))],
                ExecutionOptions {
                    date_format: Some(DateTextFormat::EnUs),
                    reference_year: Some(ReferenceYear::new(year).unwrap()),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            [vec![Value::Int16(year as i16), Value::Int16(year as i16)]]
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
    for sql in [
        "SELECT IIf(False,CDate(NULL),0)",
        "SELECT DateValue('2026-01-01 89:98')",
        "SELECT TimeValue('2026-02-30 12:00')",
        "SELECT CDate(1,1/0)",
        "SELECT VBA.TimeValue('12:00')",
    ] {
        assert!(run(sql, &source).is_err(), "{sql}");
    }
}
