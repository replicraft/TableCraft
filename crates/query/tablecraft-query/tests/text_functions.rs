use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};
fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "Items".into(),
            vec![
                Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Unused".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Label".into(), ScalarType::Text, true).unwrap(),
                Column::new("Width".into(), ScalarType::Int32, false).unwrap(),
            ],
            vec![
                vec![
                    Value::Int32(1),
                    Value::Int32(90),
                    Value::Text(" é🦀中 ".into()),
                    Value::Int32(1),
                ],
                vec![
                    Value::Int32(2),
                    Value::Int32(90),
                    Value::Text(String::new()),
                    Value::Int32(2),
                ],
                vec![
                    Value::Int32(3),
                    Value::Int32(90),
                    Value::Null,
                    Value::Int32(3),
                ],
            ],
        )
        .unwrap(),
    ])
    .unwrap()
}
fn run(sql: &str, source: &dyn Source) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(parsed.query(), source)?
        .execute(source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}
#[test]
fn text_calls_retain_fields_sort_keys_aggregate_arguments_and_correlations() {
    let source = source();
    assert_eq!(run("SELECT Len(Trim(Label)), Left(Trim(Label),Width), Right(Trim(Label),Width), Mid(Trim(Label),2,2) FROM Items WHERE Id=1",&source).unwrap(), [vec![Value::Int32(4),Value::Text("é".into()),Value::Text("中".into()),Value::Text("🦀".into())]]);
    assert_eq!(
        run(
            "SELECT Id FROM Items WHERE Len(Label)>0 ORDER BY Len(Trim(Label)) DESC",
            &source
        )
        .unwrap(),
        [vec![Value::Int32(1)]]
    );
    assert_eq!(
        run(
            "SELECT Sum(Len(Label)), Len(Max(Label)) FROM Items WHERE Id=2",
            &source
        )
        .unwrap(),
        [vec![Value::Int64(0), Value::Int32(0)]]
    );
    assert_eq!(
        run(
            "SELECT Len(Label), Left(Label,Width), Mid(Label,1) FROM Items WHERE Id=3",
            &source
        )
        .unwrap(),
        [vec![Value::Null, Value::Null, Value::Null]]
    );
    assert_eq!(run("SELECT A.Id FROM Items AS A WHERE (SELECT Len(B.Label) FROM Items AS B WHERE B.Id=A.Id)=6",&source).unwrap(), [vec![Value::Int32(1)]]);
    assert_eq!(
        run(
            "SELECT Id, Len(Max(Label)) FROM Items GROUP BY Id HAVING Len(Max(Label))=0",
            &source
        )
        .unwrap(),
        [vec![Value::Int32(2), Value::Int32(0)]]
    );
}
#[test]
fn text_calls_cannot_hide_invalid_grouping_operands_or_lossy_slices() {
    let source = source();
    assert_eq!(
        run("SELECT Mid(Max(Label),Width) FROM Items", &source),
        Err(QueryError::Grouping)
    );
    for sql in [
        "SELECT Left(Null,1/0) FROM Items",
        "SELECT Id FROM Items WHERE Id>1 AND Left(Null,1/0) IS NULL",
    ] {
        assert_eq!(
            run(sql, &source),
            Err(QueryError::Evaluation(EvalError::DivisionByZero))
        );
    }
    assert_eq!(
        run("SELECT Left('🦀',1) FROM Items", &source),
        Err(QueryError::Evaluation(EvalError::UnrepresentableText))
    );
    for sql in [
        "SELECT Len(*) FROM Items",
        "SELECT Mid(Label,1,) FROM Items",
        "SELECT VBA.Left(Label,1) FROM Items",
        "SELECT Sum(Len(Max(Label))) FROM Items",
    ] {
        assert_eq!(run(sql, &source), Err(QueryError::Unsupported));
    }
}
