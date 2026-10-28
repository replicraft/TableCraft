//! Exact retained byte algebra; native Access odd-byte behavior is unqualified.
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn execute(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
        .execute(&source, &[], Default::default())?
        .rows()
        .to_vec())
}

#[test]
fn byte_fragments_remain_lossless_across_retained_derived_and_union_rows() {
    for operator in ["&", "+"] {
        let sql = format!(
            "SELECT LenB(S),S{operator}ChrB(0) FROM (SELECT ChrB(65){operator}'' AS S UNION ALL SELECT ChrB(66){operator}'') AS Q"
        );
        assert_eq!(
            execute(&sql).unwrap(),
            [
                vec![Value::Int32(1), Value::Text("A".into())],
                vec![Value::Int32(1), Value::Text("B".into())],
            ],
            "{sql}"
        );
        let sql = format!("SELECT LenB(ChrB(65){operator}''),ChrB(65){operator}ChrB(0)");
        assert_eq!(
            execute(&sql).unwrap(),
            [vec![Value::Int32(1), Value::Text("A".into())]]
        );
    }
    assert_eq!(
        execute("SELECT LenB(S),Count(S) FROM (SELECT ChrB(65)&'' AS S UNION ALL SELECT ChrB(66)&'') AS Q GROUP BY LenB(S)").unwrap(),
        [vec![Value::Int32(1), Value::Int64(2)]]
    );
}

#[test]
fn complete_parameterized_fragments_rebind_and_incomplete_publication_stays_atomic() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS B LONG; SELECT ChrB(B)&ChrB(0)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (byte, expected) in [(65, "A"), (66, "B")] {
        assert_eq!(
            plan.execute(&source, &[("B", Value::Int32(byte))], Default::default())
                .unwrap()
                .rows(),
            [vec![Value::Text(expected.into())]]
        );
    }
    for operator in ["&", "+"] {
        for sql in [
            format!("SELECT 1,ChrB(65){operator}''"),
            format!("SELECT Len(ChrB(65){operator}'')"),
            format!("SELECT AscW(ChrB(65){operator}'')"),
            format!("SELECT ChrB(0){operator}ChrB(216)"),
        ] {
            assert_eq!(
                execute(&sql),
                Err(QueryError::Evaluation(EvalError::UnrepresentableText)),
                "{sql}"
            );
        }
        assert_eq!(
            execute(&format!("SELECT ChrB(65){operator}(1/0)")),
            Err(QueryError::Evaluation(EvalError::DivisionByZero))
        );
    }
    assert_eq!(
        execute("SELECT LenB(ChrB(65)&NULL),ChrB(65)+NULL").unwrap(),
        [vec![Value::Int32(1), Value::Null]]
    );
}
