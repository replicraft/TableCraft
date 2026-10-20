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
            vec![
                Column::new("N".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Label".into(), ScalarType::Text, true).unwrap(),
            ],
            vec![
                vec![Value::Int32(2), Value::Text("abc".into())],
                vec![Value::Int32(0), Value::Text("é中".into())],
                vec![Value::Int32(3), Value::Null],
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
fn construction_binds_groups_correlates_and_reuses_typed_parameters() {
    assert_eq!(
        run("SELECT String(N,Label),StrReverse(Nz(Label,'')),Len(Space(N)) FROM Words").unwrap(),
        [
            vec![
                Value::Text("aa".into()),
                Value::Text("cba".into()),
                Value::Int32(2)
            ],
            vec![
                Value::Text("".into()),
                Value::Text("中é".into()),
                Value::Int32(0)
            ],
            vec![Value::Null, Value::Text("".into()), Value::Int32(3)]
        ]
    );
    assert_eq!(run("SELECT Len(String(N,Nz(Label,'x'))) AS Width,Count(*) FROM Words GROUP BY Len(String(N,Nz(Label,'x'))) ORDER BY Width").unwrap(),[vec![Value::Int32(0),Value::Int64(1)],vec![Value::Int32(2),Value::Int64(1)],vec![Value::Int32(3),Value::Int64(1)]]);
    assert_eq!(
        run("SELECT (SELECT StrReverse(String(W.N,W.Label))) FROM Words AS W WHERE W.N=2").unwrap(),
        [vec![Value::Text("aa".into())]]
    );
    let source = source();
    let query = parse_query(
        "PARAMETERS N LONG,P TEXT; SELECT StrReverse(String(N,P))",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (n, p, expected) in [(2, "ab", "aa"), (3, "é", "ééé")] {
        assert_eq!(
            plan.execute(
                &source,
                &[("N", Value::Int32(n)), ("P", Value::Text(p.into()))],
                ExecutionOptions::default()
            )
            .unwrap()
            .rows(),
            [vec![Value::Text(expected.into())]]
        );
    }
}
#[test]
fn null_eagerness_arity_and_encoding_limits_are_explicit() {
    assert_eq!(
        run("SELECT Space(NULL),String(NULL,'x'),String(2,NULL)").unwrap(),
        [vec![Value::Null, Value::Null, Value::Null]]
    );
    assert_eq!(
        run("SELECT String(NULL,1/0)").err(),
        Some(QueryError::Evaluation(
            tablecraft_expressions::EvalError::DivisionByZero
        ))
    );
    for sql in [
        "SELECT Space(-1)",
        "SELECT String(1,'')",
        "SELECT String(1,128)",
        "SELECT String(1,'😀')",
        "SELECT StrReverse('😀')",
        "SELECT StrReverse(NULL)",
        "SELECT String(1,'x',1/0)",
        "SELECT Space(1,1/0)",
        "SELECT StrReverse('a',1/0)",
        "SELECT VBA.String(1,'x')",
    ] {
        assert!(run(sql).is_err(), "{sql}");
    }
}
