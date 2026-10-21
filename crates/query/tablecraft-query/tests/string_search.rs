#[path = "string_search/locale.rs"]
mod locale;
use tablecraft_core::values::Value;
use tablecraft_query::{
    ExecutionOptions, QueryError, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};
fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "Words".into(),
            vec![Column::new("Label".into(), ScalarType::Text, true).unwrap()],
            vec![Some("banana"), Some("bandana"), None, Some("😀ban😀")]
                .into_iter()
                .map(|label| vec![label.map_or(Value::Null, |v| Value::Text(v.into()))])
                .collect(),
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
fn search_positions_bind_group_correlate_and_reuse_typed_parameters() {
    let rows = run("SELECT InStr(1,Label,'an',0),InStrRev(Label,'an') FROM Words").unwrap();
    assert_eq!(
        rows,
        [
            vec![Value::Int32(2), Value::Int32(4)],
            vec![Value::Int32(2), Value::Int32(5)],
            vec![Value::Null, Value::Null],
            vec![Value::Int32(4), Value::Int32(4)]
        ]
    );
    let rows=run("SELECT InStr(1,Label,'an',0) AS [Position],Count(*) FROM Words GROUP BY InStr(1,Label,'an',0) ORDER BY [Position]").unwrap();
    assert_eq!(
        rows,
        [
            vec![Value::Null, Value::Int64(1)],
            vec![Value::Int32(2), Value::Int64(2)],
            vec![Value::Int32(4), Value::Int64(1)]
        ]
    );
    assert_eq!(
        run(
            "SELECT (SELECT InStrRev(W.Label,'an')) FROM Words AS W WHERE InStr(1,W.Label,'an',0)>2"
        )
        .unwrap(),
        [vec![Value::Int32(4)]]
    );
    let source = source();
    let query = parse_query(
        "PARAMETERS P TEXT; SELECT InStrRev(P,'a')",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (input, expected) in [
        (Value::Text("banana".into()), Value::Int32(6)),
        (Value::Text("b".into()), Value::Int32(0)),
        (Value::Null, Value::Null),
    ] {
        assert_eq!(
            plan.execute(&source, &[("P", input)], ExecutionOptions::default())
                .unwrap()
                .rows(),
            [vec![expected]]
        );
    }
}
#[test]
fn optional_slots_defaults_and_eager_failures_remain_distinct() {
    assert_eq!(run("SELECT InStrRev('aba','a',),InStrRev('aba','a',,0),InStrRev('aba','a',,),InStrRev('aba','a',1,)").unwrap(),[vec![Value::Int32(3),Value::Int32(3),Value::Int32(3),Value::Int32(1)]]);
    assert_eq!(
        run("SELECT InStr(1,NULL,'x',1/0)").err(),
        Some(QueryError::Evaluation(
            tablecraft_expressions::EvalError::DivisionByZero
        ))
    );
    for sql in [
        "SELECT InStr('a','a')",
        "SELECT InStr(,'a','a',0)",
        "SELECT InStrRev('a',)",
        "SELECT InStrRev(NULL,'a',NULL)",
        "SELECT InStrRev(NULL,'a',-1,NULL)",
        "SELECT IIf(False,InStrRev('a','a',0),1)",
        "SELECT VBA.InStrRev('a','a')",
    ] {
        assert!(run(sql).is_err(), "{sql}");
    }
}
