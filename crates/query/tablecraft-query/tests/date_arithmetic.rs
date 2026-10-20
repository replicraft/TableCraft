use tablecraft_core::values::{Double, LegacyDateTime, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};
fn date(serial: f64) -> Value {
    Value::DateTime(LegacyDateTime::from_ole_days(serial).unwrap())
}
fn double(value: f64) -> Value {
    Value::Double(Double::from_float(value))
}
fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "Dates".into(),
            vec![
                Column::new("Unused".into(), ScalarType::Text, true).unwrap(),
                Column::new("Moment".into(), ScalarType::DateTime, true).unwrap(),
            ],
            vec![
                vec![Value::Null, date(2.25)],
                vec![Value::Null, date(-1.25)],
                vec![Value::Null, Value::Null],
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
fn date_operators_bind_prune_correlate_and_preserve_result_types() {
    assert_eq!(
        run("SELECT +Moment,Moment+0.5,0.5+Moment,Moment-Moment,0.5-Moment,-Moment FROM Dates")
            .unwrap(),
        [
            vec![
                date(2.25),
                date(2.75),
                date(2.75),
                double(0.0),
                date(-1.75),
                date(-2.25)
            ],
            vec![
                date(-1.25),
                date(-0.75),
                date(-0.75),
                double(0.0),
                date(1.75),
                date(1.25)
            ],
            vec![Value::Null; 6],
        ]
    );
    assert_eq!(
        run(
            "SELECT (SELECT T.Moment+1) AS M FROM Dates AS T WHERE T.Moment IS NOT NULL ORDER BY M"
        )
        .unwrap(),
        [vec![date(-0.25)], vec![date(3.25)]]
    );
    assert_eq!(
        run("SELECT Moment+1 AS M,Count(*) FROM Dates GROUP BY Moment+1 ORDER BY M").unwrap(),
        [
            vec![Value::Null, Value::Int64(1)],
            vec![date(-0.25), Value::Int64(1)],
            vec![date(3.25), Value::Int64(1)]
        ]
    );
}
#[test]
fn date_parameters_rebind_without_changing_aggregate_and_error_boundaries() {
    let source = source();
    let query = parse_query("PARAMETERS P DateTime; SELECT P+1", ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (input, expected) in [
        (date(2.25), date(3.25)),
        (date(-1.25), date(-0.25)),
        (Value::Null, Value::Null),
    ] {
        assert_eq!(
            plan.execute(&source, &[("P", input)], ExecutionOptions::default())
                .unwrap()
                .rows(),
            [vec![expected]]
        );
    }
    for sql in [
        "SELECT Sum(Moment) FROM Dates",
        "SELECT Avg(Moment) FROM Dates",
        "SELECT Moment*2 FROM Dates",
        "SELECT Moment+'1' FROM Dates",
    ] {
        assert!(run(sql).is_err(), "{sql}");
    }
    assert_eq!(
        run("SELECT NULL+(#2025-01-01#+1/0)").err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    assert_eq!(
        run("SELECT #9999-12-31#+1").unwrap(),
        [vec![double(2958466.0)]]
    );
}
