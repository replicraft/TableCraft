use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{ExecutionOptions, QueryError, Source, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::new(vec![]).unwrap();
    execute(sql, &source)
}

fn execute(sql: &str, source: &dyn Source) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(parsed.query(), source)?
        .execute(source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}

#[test]
fn integral_operators_keep_result_carriers_before_predicate_selection() {
    assert_eq!(
        run("SELECT CByte(128) Or CByte(1),CInt(1) And CInt(2),Not CLng(0),CByte(1) Imp CByte(2),CInt(1) Eqv CInt(2),True Xor False").unwrap(),
        [vec![Value::Byte(129), Value::Int16(0), Value::Int32(-1), Value::Byte(254), Value::Int16(-4), Value::Boolean(true)]]
    );
    for (predicate, selected) in [
        ("1 And 2", false),
        ("Not 1", true),
        ("Not -1", false),
        ("1 Or 2", true),
        ("0.25", true),
        ("#1899-12-30#", false),
        ("#1899-12-30 06:00:00#", true),
        ("NULL", false),
    ] {
        let rows = run(&format!("SELECT 99 WHERE {predicate}")).unwrap();
        assert_eq!(rows.len(), usize::from(selected), "{predicate}");
    }
}

#[test]
fn predicates_work_in_joins_groups_correlations_and_crosstabs() {
    assert_eq!(run("SELECT A.ID FROM (SELECT 1 AS ID UNION ALL SELECT 2 UNION ALL SELECT 3) AS A INNER JOIN (SELECT 2 AS Mask) AS B ON A.ID And B.Mask ORDER BY A.ID").unwrap(), [vec![Value::Int64(2)], vec![Value::Int64(3)]]);
    assert_eq!(run("SELECT ID And 2 AS K,Count(*) FROM (SELECT 1 AS ID UNION ALL SELECT 2 UNION ALL SELECT 3) AS A GROUP BY ID And 2 HAVING Count(*) And 2 ORDER BY K").unwrap(), [vec![Value::Int64(2), Value::Int64(2)]]);
    assert_eq!(run("SELECT ID FROM (SELECT 1 AS ID UNION ALL SELECT 2) AS A WHERE EXISTS (SELECT 1 WHERE A.ID And 2)").unwrap(), [vec![Value::Int64(2)]]);
    assert_eq!(run("TRANSFORM Count(*) SELECT K FROM (SELECT 1 AS K,2 AS P UNION ALL SELECT 2,3) AS A WHERE K And 2 GROUP BY K HAVING Count(*) PIVOT P IN (2,3)").unwrap(), [vec![Value::Int64(2), Value::Null, Value::Int64(1)]]);
}

#[test]
fn eager_failures_and_unsupported_coercions_do_not_turn_into_false() {
    for sql in [
        "SELECT 0 And (1/0)",
        "SELECT 1 WHERE 0 And (1/0)",
        "SELECT 1 WHERE True Or (1/0)",
    ] {
        assert_eq!(
            run(sql),
            Err(QueryError::Evaluation(EvalError::DivisionByZero)),
            "{sql}"
        );
    }
    for sql in [
        "SELECT Not 1.5",
        "SELECT 1 WHERE CVErr(0)",
        "SELECT 1 WHERE '1'",
    ] {
        assert!(run(sql).is_err(), "{sql}");
    }
}
