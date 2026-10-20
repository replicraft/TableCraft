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
                Column::new("Score".into(), ScalarType::Int32, true).unwrap(),
            ],
            vec![
                vec![Value::Int32(1), Value::Int32(10)],
                vec![Value::Int32(2), Value::Null],
                vec![Value::Int32(3), Value::Int32(30)],
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
fn builtins_work_in_outputs_filters_aggregates_sorting_and_correlated_branches() {
    let source = source();
    assert_eq!(run("SELECT Id, IsNull(Score), IIf(IsNull(Score),0,Score) AS Chosen FROM Items ORDER BY Chosen DESC",&source).unwrap(),[vec![Value::Int32(3),Value::Boolean(false),Value::Int32(30)],vec![Value::Int32(1),Value::Boolean(false),Value::Int32(10)],vec![Value::Int32(2),Value::Boolean(true),Value::Int64(0)]]);
    assert_eq!(
        run(
            "SELECT Sum(IIf(IsNull(Score),0,Score)) AS Total FROM Items",
            &source
        )
        .unwrap(),
        [vec![Value::Int64(40)]]
    );
    assert_eq!(
        run("SELECT Id FROM Items WHERE IsNull(Score)", &source).unwrap(),
        [vec![Value::Int32(2)]]
    );
    assert_eq!(run("SELECT A.Id, IIf(A.Id>1,(SELECT Count(*) FROM Items AS B WHERE B.Id<=A.Id),0) FROM Items AS A ORDER BY A.Id",&source).unwrap(),[vec![Value::Int32(1),Value::Int64(0)],vec![Value::Int32(2),Value::Int64(2)],vec![Value::Int32(3),Value::Int64(3)]]);
}
#[test]
fn grouping_arity_and_unknown_calls_are_checked_and_unselected_errors_still_fail() {
    let source = source();
    for sql in [
        "SELECT IIf(True,1,1/0) FROM Items",
        "SELECT Id FROM Items WHERE Id>1 AND IIf(True,True,1/0)",
    ] {
        assert_eq!(
            run(sql, &source),
            Err(QueryError::Evaluation(EvalError::DivisionByZero))
        );
    }
    assert_eq!(
        run("SELECT IIf(Count(*)>0,Id,0) FROM Items", &source),
        Err(QueryError::Grouping)
    );
    for sql in [
        "SELECT IIf(True,1) FROM Items",
        "SELECT IsNull(*) FROM Items",
        "SELECT VBA.IIf(True,1,0) FROM Items",
        "SELECT Sum(IIf(True,Sum(Score),0)) FROM Items",
    ] {
        assert_eq!(run(sql, &source), Err(QueryError::Unsupported));
    }
}

#[test]
fn nz_preserves_omission_null_grouping_and_correlation() {
    let source = source();
    assert_eq!(
        run(
            "SELECT Nz(Score), Nz(Score,Null), Nz(Score,0) FROM Items WHERE Id=2",
            &source
        )
        .unwrap(),
        [vec![
            Value::Text(String::new()),
            Value::Null,
            Value::Int64(0)
        ]]
    );
    assert_eq!(
        run(
            "SELECT Sum(Nz(Score,0)), Nz(Sum(Score),0) FROM Items",
            &source
        )
        .unwrap(),
        [vec![Value::Int64(40), Value::Int64(40)]]
    );
    assert_eq!(run("SELECT A.Id FROM Items AS A WHERE Nz((SELECT B.Score FROM Items AS B WHERE B.Id=A.Id),0)=0", &source).unwrap(), [vec![Value::Int32(2)]]);
    assert_eq!(
        run("SELECT Nz(Null,Sum(Score)) FROM Items", &source).unwrap(),
        [vec![Value::Int64(40)]]
    );
    assert_eq!(
        run(
            "SELECT Id FROM Items WHERE Nz(Score,0)=0 ORDER BY Nz(Score,0)",
            &source
        )
        .unwrap(),
        [vec![Value::Int32(2)]]
    );
    assert_eq!(
        run("SELECT Nz(Count(*),Id) FROM Items", &source),
        Err(QueryError::Grouping)
    );
    assert_eq!(
        run("SELECT Nz(Id,1/0) FROM Items", &source),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    for sql in [
        "SELECT Nz(*) FROM Items",
        "SELECT Nz(Null,) FROM Items",
        "SELECT Nz(1,2,3) FROM Items",
        "SELECT VBA.Nz(Null) FROM Items",
        "SELECT Sum(Nz(Sum(Score),0)) FROM Items",
    ] {
        assert_eq!(run(sql, &source), Err(QueryError::Unsupported));
    }
}
