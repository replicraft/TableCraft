use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "Words".into(),
            vec![Column::new("Label".into(), ScalarType::Text, true).unwrap()],
            vec![
                vec![Value::Text("a".into())],
                vec![Value::Text("bc".into())],
            ],
        )
        .unwrap(),
    ])
    .unwrap()
}

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = source();
    let parsed = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source)?;
    let result = plan.execute(&source, &[], ExecutionOptions::default())?;
    Ok(result.rows().to_vec())
}

#[test]
fn retained_reversal_survives_derived_grouped_correlated_and_domain_consumers() {
    assert_eq!(
        run("SELECT LenB(S),AscW(S) FROM (SELECT StrReverse('a'&ChrW(55296)) AS S) AS Q").unwrap(),
        [vec![Value::Int32(4), Value::Int16(-10240)]],
    );
    assert_eq!(
        run("SELECT LenB(StrReverse(Label&ChrW(55296))) AS Width,Count(*) FROM Words GROUP BY LenB(StrReverse(Label&ChrW(55296))) ORDER BY Width").unwrap(),
        [vec![Value::Int32(4), Value::Int64(1)], vec![Value::Int32(6), Value::Int64(1)]],
    );
    assert_eq!(
        run("SELECT (SELECT AscW(StrReverse(W.Label&ChrW(55296)))) FROM Words AS W").unwrap(),
        [vec![Value::Int16(-10240)], vec![Value::Int16(-10240)]],
    );
    assert_eq!(
        run("SELECT AscW(DLookup('StrReverse(Label&ChrW(55296))','Words'))").unwrap(),
        [vec![Value::Int16(-10240)]],
    );
}

#[test]
fn prepared_reversal_rebinds_units_and_double_reversal_repairs_publication() {
    let source = source();
    let query = parse_query(
        "PARAMETERS P LONG; SELECT AscW(StrReverse('a'&ChrW(P)))",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();

    for (input, expected) in [(65, 65), (55296, -10240), (57343, -8193)] {
        let result = plan
            .execute(&source, &[("P", Value::Int32(input))], Default::default())
            .unwrap();
        assert_eq!(result.rows(), [vec![Value::Int16(expected)]]);
    }

    assert_eq!(
        run("SELECT StrReverse(StrReverse('a🦀中')),StrReverse(ChrW(56832)&ChrW(55357))").unwrap(),
        [vec![Value::Text("a🦀中".into()), Value::Text("😀".into())]],
    );
}

#[test]
fn final_invalid_units_and_eager_late_errors_refuse_complete_results() {
    for sql in [
        "SELECT StrReverse('🦀')",
        "SELECT StrReverse(ChrW(55296))",
        "SELECT LenB(StrReverse(ChrB(1)))",
    ] {
        assert!(matches!(
            run(sql),
            Err(QueryError::Evaluation(EvalError::UnrepresentableText))
        ));
    }
    assert!(matches!(
        run("SELECT IIf(True,StrReverse(ChrW(55296)),1/0)"),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    ));
}
