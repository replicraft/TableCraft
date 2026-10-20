use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};
fn date(text: &str) -> Value {
    Value::DateTime(tablecraft_expressions::datetime::parse_literal(text).unwrap())
}
fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "Parts".into(),
            ["Unused", "Y", "M"]
                .map(|n| Column::new(n.into(), ScalarType::Int32, true).unwrap())
                .to_vec(),
            vec![
                vec![Value::Null, Value::Int32(2024), Value::Int32(3)],
                vec![Value::Null, Value::Int32(2024), Value::Int32(3)],
                vec![Value::Null, Value::Int32(2023), Value::Int32(3)],
            ],
        )
        .unwrap(),
    ])
    .unwrap()
}
fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = source();
    let query = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(query.query(), &source)?
        .execute(&source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}
#[test]
fn constructors_group_prune_correlate_and_compose_with_date_components() {
    assert_eq!(run("SELECT DateSerial(Y,M,0) AS D,Count(*) FROM Parts GROUP BY DateSerial(Y,M,0) ORDER BY D").unwrap(),[vec![date("2023-02-28"),Value::Int64(1)],vec![date("2024-02-29"),Value::Int64(2)]]);
    assert_eq!(run("SELECT (SELECT Day(DateSerial(P.Y,P.M,0))) AS D,Hour(TimeSerial(M,75,0)) AS H FROM Parts AS P ORDER BY D").unwrap(),[vec![Value::Int16(28),Value::Int16(4)],vec![Value::Int16(29),Value::Int16(4)],vec![Value::Int16(29),Value::Int16(4)]]);
    assert!(run("SELECT DateSerial(Y,M,1) FROM Parts GROUP BY TimeSerial(Y,M,1)").is_err());
}
#[test]
fn reusable_constructor_parameters_and_eager_errors_remain_explicit() {
    let source = source();
    let query = parse_query(
        "PARAMETERS Y LONG; SELECT Year(DateSerial(Y,3,0))",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (year, expected) in [(29, 2029), (30, 1930), (2024, 2024)] {
        assert_eq!(
            plan.execute(
                &source,
                &[("Y", Value::Int32(year))],
                ExecutionOptions::default()
            )
            .unwrap()
            .rows(),
            [vec![Value::Int16(expected)]]
        );
    }
    assert_eq!(
        run("SELECT DateSerial(NULL,1,1/0)").err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    assert_eq!(
        run("SELECT TimeSerial(NULL,0,0)").err(),
        Some(QueryError::Evaluation(EvalError::InvalidNullConversion))
    );
    for sql in [
        "SELECT DateSerial(2024,,1)",
        "SELECT TimeSerial(1,2)",
        "SELECT DateSerial(2024,1,1,1)",
        "SELECT TimeSerial(32768,0,0)",
    ] {
        assert!(run(sql).is_err(), "{sql}");
    }
}
