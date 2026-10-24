use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
        .execute(&source, &[], Default::default())?
        .rows()
        .to_vec())
}

#[test]
fn unicode_observations_compose_over_grouped_and_correlated_numeric_results() {
    assert_eq!(
        run("SELECT Q.N,(SELECT AscW(ChrW(Q.N))),Len(ChrW(Q.N)) FROM (SELECT 55296 AS N UNION ALL SELECT 57343) AS Q").unwrap(),
        [
            vec![Value::Int64(55296), Value::Int16(-10240), Value::Int32(1)],
            vec![Value::Int64(57343), Value::Int16(-8193), Value::Int32(1)],
        ]
    );
    assert_eq!(
        run("SELECT Len(LeftB(Q.T,2)),Sum(AscW(LeftB(Q.T,2))) FROM (SELECT '😀' AS T UNION ALL SELECT '😁') AS Q GROUP BY Len(LeftB(Q.T,2))").unwrap(),
        [vec![Value::Int32(1), Value::Int64(-20358)]]
    );
    assert_eq!(
        run("TRANSFORM Count(*) SELECT Len(ChrW(Q.N)) AS Units FROM (SELECT 55296 AS N UNION ALL SELECT 55297) AS Q GROUP BY Len(ChrW(Q.N)) PIVOT AscW(ChrW(Q.N)) IN (-10240,-10239)").unwrap(),
        [vec![Value::Int32(1), Value::Int64(1), Value::Int64(1)]]
    );
}

#[test]
fn utf16_code_parameters_rebind_without_retaining_raw_values_in_the_plan() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS P LONG; SELECT Len(ChrW(P)),AscW(CVar(ChrW(P))),LenB(ChrW(P))",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (code, expected) in [(55296, -10240), (57343, -8193), (-32768, -32768), (65, 65)] {
        assert_eq!(
            plan.execute(
                &source,
                &[("P", Value::Int32(code))],
                ExecutionOptions::default()
            )
            .unwrap()
            .rows(),
            &[vec![
                Value::Int32(1),
                Value::Int16(expected),
                Value::Int32(2)
            ]]
        );
    }
    assert_eq!(
        plan.execute(&source, &[("P", Value::Int32(65536))], Default::default())
            .err(),
        Some(QueryError::Evaluation(EvalError::Domain))
    );
    for sql in [
        "SELECT ChrW(55296)",
        "SELECT N FROM (SELECT ChrW(55296) AS N) AS Q",
        "SELECT Len(ChrB(65))",
        "SELECT AscW(LeftB('A',1))",
    ] {
        assert_eq!(
            run(sql),
            Err(QueryError::Evaluation(EvalError::UnrepresentableText)),
            "{sql}"
        );
    }
}
