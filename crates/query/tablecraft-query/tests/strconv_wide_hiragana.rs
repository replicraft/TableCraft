//! Bound-query composition of the explicit Japanese wide/Hiragana profile.
use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source)?;
    let result = plan.execute(
        &source,
        &[],
        ExecutionOptions {
            number_format: Some(NumberFormat::Invariant),
            ..Default::default()
        },
    )?;
    Ok(result.rows().to_vec())
}

#[test]
fn wide_hiragana_preserves_derived_null_correlated_union_and_raw_consumers() {
    let derived = "SELECT StrConv(Q.T,36,1041),(SELECT Len(StrConv(Q.T,36,1041))) FROM (SELECT 'ｶﾞ' AS T UNION ALL SELECT NULL) Q";
    let expected = [
        vec![Value::Text("が".into()), Value::Int32(1)],
        vec![Value::Null, Value::Null],
    ];
    assert_eq!(run(derived).unwrap(), expected);

    let union = "SELECT StrConv('ｶﾞ',36,1041) AS V UNION ALL SELECT CLng(2)";
    assert_eq!(
        run(union).unwrap(),
        [
            vec![Value::Text("が".into())],
            vec![Value::Text("2".into())]
        ]
    );

    let raw = "SELECT LenB(StrConv(CVar(LeftB('ｶﾞ',4)),36,1041)),AscW(StrConv(CVar(LeftB('ｶﾞ',4)),36,1041))";
    assert_eq!(
        run(raw).unwrap(),
        [vec![Value::Int32(2), Value::Int16(0x304c)]]
    );
    assert_eq!(
        run("SELECT StrConv(ChrB(65),36,1041)"),
        Err(QueryError::Evaluation(EvalError::UnrepresentableText))
    );
}
