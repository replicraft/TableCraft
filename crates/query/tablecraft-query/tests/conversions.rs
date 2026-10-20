use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
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
                Column::new("Id".into(), ScalarType::Int64, false).unwrap(),
                Column::new("Unused".into(), ScalarType::Text, false).unwrap(),
                Column::new("Score".into(), ScalarType::Int32, true).unwrap(),
            ],
            vec![
                vec![
                    Value::Int64(1),
                    Value::Text("unused".into()),
                    Value::Int32(10),
                ],
                vec![Value::Int64(2), Value::Text("unused".into()), Value::Null],
            ],
        )
        .unwrap(),
    ])
    .unwrap()
}
fn run(sql: &str, source: &MemorySource) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(parsed.query(), source)?
        .execute(source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}
#[test]
fn checked_conversions_participate_in_retrieval_grouping_and_correlations() {
    let source = source();
    assert_eq!(
        run(
            "SELECT CByte(Id),CInt(Id),CLng(Id),CLngLng(Id),CBool(Score) FROM Items WHERE Id=1",
            &source
        )
        .unwrap(),
        [vec![
            Value::Byte(1),
            Value::Int16(1),
            Value::Int32(1),
            Value::Int64(1),
            Value::Boolean(true)
        ]]
    );
    assert_eq!(
        run(
            "SELECT CInt(Nz(Score,0)) FROM Items ORDER BY CLng(Id) DESC",
            &source
        )
        .unwrap(),
        [vec![Value::Int16(0)], vec![Value::Int16(10)]]
    );
    assert_eq!(
        run("SELECT CInt(Sum(Score)),Sum(CLng(Id)) FROM Items", &source).unwrap(),
        [vec![Value::Int16(10), Value::Int64(3)]]
    );
    assert_eq!(run("SELECT A.Id FROM Items AS A WHERE (SELECT CBool(B.Score) FROM Items AS B WHERE B.Id=A.Id AND B.Score IS NOT NULL)",&source).unwrap(),[vec![Value::Int64(1)]]);
    assert_eq!(
        run("SELECT CInt(Id),Count(*) FROM Items", &source),
        Err(QueryError::Grouping)
    );
    assert_eq!(
        run("SELECT CInt(Score) FROM Items", &source),
        Err(QueryError::Evaluation(EvalError::InvalidNullConversion))
    );
    assert_eq!(
        run("SELECT IIf(True,1,CInt(Null)) FROM Items", &source),
        Err(QueryError::Evaluation(EvalError::InvalidNullConversion))
    );
    for sql in [
        "SELECT CInt(*) FROM Items",
        "SELECT CLng(Id,1) FROM Items",
        "SELECT VBA.CByte(Id) FROM Items",
        "SELECT Sum(CLng(Sum(Score))) FROM Items",
    ] {
        assert_eq!(run(sql, &source), Err(QueryError::Unsupported));
    }
}

#[test]
fn explicit_float_conversions_keep_widths_and_do_not_relax_implicit_precision_rules() {
    use tablecraft_core::values::{Double, Single};
    let source = source();
    assert_eq!(
        run("SELECT CSng(Id),CDbl(Id) FROM Items WHERE Id=1", &source).unwrap(),
        [vec![
            Value::Single(Single::from_float(1.0)),
            Value::Double(Double::from_float(1.0))
        ]]
    );
    assert_eq!(
        run("SELECT CDbl(Sum(CSng(Id))) FROM Items", &source).unwrap(),
        [vec![Value::Double(Double::from_float(3.0))]]
    );
    assert_eq!(
        run(
            "SELECT CSng(Nz(Score,0)) FROM Items ORDER BY CDbl(Id) DESC",
            &source
        )
        .unwrap(),
        [
            vec![Value::Single(Single::from_float(0.0))],
            vec![Value::Single(Single::from_float(10.0))]
        ]
    );
    assert_eq!(run("SELECT A.Id FROM Items AS A WHERE (SELECT CDbl(B.Id) FROM Items AS B WHERE B.Id=A.Id)=2",&source).unwrap(),[vec![Value::Int64(2)]]);
    assert_eq!(
        run("SELECT CDbl(9007199254740993)+0.5", &source).unwrap(),
        [vec![Value::Double(Double::from_float(9007199254740992.0))]]
    );
    assert_eq!(
        run("SELECT 9007199254740993+0.5", &source),
        Err(QueryError::Evaluation(EvalError::PrecisionLoss))
    );
    assert_eq!(
        run("SELECT CSng(1e100) FROM Items", &source),
        Err(QueryError::Evaluation(EvalError::Overflow))
    );
    assert_eq!(
        run("SELECT CDbl(Null) FROM Items", &source),
        Err(QueryError::Evaluation(EvalError::InvalidNullConversion))
    );
    assert_eq!(
        run("SELECT CSng(Id),Count(*) FROM Items", &source),
        Err(QueryError::Grouping)
    );
}

#[test]
fn currency_conversions_preserve_scale_and_query_traversal_without_implicit_coercion() {
    use tablecraft_core::values::Currency;
    let source = source();
    assert_eq!(
        run(
            "SELECT CCur(Id),CCur(Nz(Score,0)) FROM Items ORDER BY Id",
            &source
        )
        .unwrap(),
        [
            vec![
                Value::Currency(Currency::from_scaled(10000)),
                Value::Currency(Currency::from_scaled(100000))
            ],
            vec![
                Value::Currency(Currency::from_scaled(20000)),
                Value::Currency(Currency::from_scaled(0))
            ]
        ]
    );
    assert_eq!(
        run("SELECT CCur(Sum(Id)) FROM Items", &source).unwrap(),
        [vec![Value::Currency(Currency::from_scaled(30000))]]
    );
    assert_eq!(run("SELECT A.Id FROM Items AS A WHERE (SELECT CCur(B.Id) FROM Items AS B WHERE B.Id=A.Id)=CCur(2)",&source).unwrap(),[vec![Value::Int64(2)]]);
    assert_eq!(
        run("SELECT CCur(Score) FROM Items", &source),
        Err(QueryError::Evaluation(EvalError::InvalidNullConversion))
    );
    assert_eq!(
        run("SELECT CCur(Id),Count(*) FROM Items", &source),
        Err(QueryError::Grouping)
    );
}
