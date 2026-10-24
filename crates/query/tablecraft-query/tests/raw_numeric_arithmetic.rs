use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn raw_numeric_null_arithmetic_rebinds_without_weakening_constructor_validation() {
    let source = MemorySource::default();
    let expressions: Vec<_> = ["-", "*", "/", "\\", "Mod", "^"]
        .into_iter()
        .flat_map(|operator| {
            [
                format!("ChrW(P) {operator} NULL"),
                format!("NULL {operator} ChrW(P)"),
            ]
        })
        .collect();
    let sql = format!("PARAMETERS P LONG; SELECT {}", expressions.join(","));
    let parsed = parse_query(&sql, Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for code in [55296, 65, 57343] {
        assert_eq!(
            plan.execute(&source, &[("P", Value::Int32(code))], Default::default())
                .unwrap()
                .rows(),
            &[vec![Value::Null; expressions.len()]]
        );
    }
    assert_eq!(
        plan.execute(&source, &[("P", Value::Int32(65536))], Default::default())
            .err(),
        Some(QueryError::Evaluation(EvalError::Domain))
    );
}

#[test]
fn grouped_and_correlated_numeric_nulls_can_observe_raw_sources() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "SELECT Count(*),(SELECT ChrW(Q.N)/NULL) FROM (SELECT 55296 AS N UNION ALL SELECT 55297) AS Q GROUP BY ChrW(Q.N)/NULL",
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        &[vec![Value::Int64(2), Value::Null]]
    );
}
