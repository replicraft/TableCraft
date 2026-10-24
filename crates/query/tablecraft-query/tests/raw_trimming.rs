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
                vec![Value::Text(" a ".into())],
                vec![Value::Text("b".into())],
                vec![Value::Null],
            ],
        )
        .unwrap(),
    ])
    .unwrap()
}

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = source();
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
        .execute(&source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}

#[test]
fn trimming_cells_survive_derived_groups_correlations_and_domain_retrieval() {
    assert_eq!(
        run("SELECT LenB(S),AscW(S) FROM (SELECT Trim(' '&ChrW(55296)&' ') AS S) AS Q").unwrap(),
        [vec![Value::Int32(2), Value::Int16(-10240)]],
    );
    assert_eq!(
        run("SELECT LenB(Trim(' '&ChrW(55296)&Label&' ')) AS Width,Count(*) FROM Words GROUP BY LenB(Trim(' '&ChrW(55296)&Label&' ')) ORDER BY Width").unwrap(),
        [vec![Value::Int32(2), Value::Int64(1)], vec![Value::Int32(4), Value::Int64(1)], vec![Value::Int32(6), Value::Int64(1)]],
    );
    assert_eq!(
        run("SELECT (SELECT LenB(Trim(' '&ChrW(55296)&W.Label&' '))) FROM Words AS W").unwrap(),
        [
            vec![Value::Int32(6)],
            vec![Value::Int32(4)],
            vec![Value::Int32(2)]
        ],
    );
    assert_eq!(
        run("SELECT LenB(DLookup('Trim(ChrW(55296)&Label)','Words'))").unwrap(),
        [vec![Value::Int32(6)]],
    );
}

#[test]
fn prepared_trimming_rebinds_code_units_and_nested_results_repair_final_pairs() {
    let source = source();
    let parsed = parse_query(
        "PARAMETERS P LONG; SELECT AscW(Trim(' '&ChrW(P)&' '))",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (input, expected) in [(65, 65), (55296, -10240), (57343, -8193)] {
        let result = plan
            .execute(
                &source,
                &[("P", Value::Int32(input))],
                ExecutionOptions::default(),
            )
            .unwrap();
        assert_eq!(result.rows(), [vec![Value::Int16(expected)]]);
    }
    assert_eq!(
        run("SELECT Trim(' '&ChrW(55357)&' ')&ChrW(56832),LenB(Trim(Replace(' '&ChrW(55296)&' ','z','')))").unwrap(),
        [vec![Value::Text("😀".into()), Value::Int32(2)]],
    );
}

#[test]
fn final_publication_odd_ranges_and_late_operational_errors_stay_explicit() {
    for sql in ["SELECT Trim(ChrW(55296))", "SELECT LenB(Trim(ChrB(32)))"] {
        assert!(matches!(
            run(sql),
            Err(QueryError::Evaluation(EvalError::UnrepresentableText))
        ));
    }
    assert!(matches!(
        run("SELECT LenB(Trim(ChrW(55296))),1/0"),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    ));
}
