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
            vec![Column::new("Label".into(), ScalarType::Text, false).unwrap()],
            vec![
                vec![Value::Text("banana".into())],
                vec![Value::Text("bandana".into())],
                vec![Value::Text("😀banana".into())],
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
fn replacement_binds_groups_correlates_and_reuses_parameters() {
    assert_eq!(
        run("SELECT Replace(Label,'an','X') FROM Words").unwrap(),
        [
            vec![Value::Text("bXXa".into())],
            vec![Value::Text("bXdXa".into())],
            vec![Value::Text("😀bXXa".into())]
        ]
    );
    assert_eq!(run("SELECT Len(Replace(Label,'an','X')) AS Width,Count(*) FROM Words GROUP BY Len(Replace(Label,'an','X')) ORDER BY Width").unwrap(),[vec![Value::Int32(4),Value::Int64(1)],vec![Value::Int32(5),Value::Int64(1)],vec![Value::Int32(6),Value::Int64(1)]]);
    assert_eq!(run("SELECT (SELECT Replace(W.Label,'an','X',3,1)) FROM Words AS W WHERE Len(Replace(W.Label,'an','X'))=4").unwrap(),[vec![Value::Text("nXa".into())]]);
    let source = source();
    let query = parse_query(
        "PARAMETERS P TEXT; SELECT Replace(P,'a','x')",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (input, expected) in [("a", "x"), ("ba", "bx")] {
        assert_eq!(
            plan.execute(
                &source,
                &[("P", Value::Text(input.into()))],
                ExecutionOptions::default()
            )
            .unwrap()
            .rows(),
            [vec![Value::Text(expected.into())]]
        );
    }
    assert!(
        plan.execute(&source, &[("P", Value::Null)], ExecutionOptions::default())
            .is_err()
    );
}
#[test]
fn defaults_slots_and_eager_errors_are_not_hidden_by_empty_or_null_inputs() {
    assert_eq!(run("SELECT Replace('banana','an','X',,,),Replace('banana','an','X',,1,),Replace('banana','an','X',3,0)").unwrap(),[vec![Value::Text("bXXa".into()),Value::Text("bXana".into()),Value::Text("nana".into())]]);
    assert_eq!(
        run("SELECT Replace(NULL,'a','x',1/0)").err(),
        Some(QueryError::Evaluation(
            tablecraft_expressions::EvalError::DivisionByZero
        ))
    );
    for sql in [
        "SELECT Replace(NULL,'a','b')",
        "SELECT Replace('a',,'b')",
        "SELECT Replace('','a','b',0)",
        "SELECT Replace('','a','b',1,NULL)",
        "SELECT Replace('a','a','b',1,-1,NULL)",
        "SELECT IIf(False,Replace('a','a','b',0),1)",
        "SELECT VBA.Replace('a','a','b')",
    ] {
        assert!(run(sql).is_err(), "{sql}");
    }
}
