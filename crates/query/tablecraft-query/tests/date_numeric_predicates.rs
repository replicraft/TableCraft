use tablecraft_core::values::{LegacyDateTime, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    QueryError, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

fn source() -> MemorySource {
    let rows = [Some(1.0), Some(-0.25), Some(0.25), Some(-1.25), None]
        .into_iter()
        .enumerate()
        .map(|(index, serial)| {
            vec![
                Value::Int32(index as i32 + 1),
                serial.map_or(Value::Null, |serial| {
                    Value::DateTime(LegacyDateTime::from_ole_days(serial).unwrap())
                }),
            ]
        })
        .collect();
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![
                Column::new("ID".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Moment".into(), ScalarType::DateTime, true).unwrap(),
            ],
            rows,
        )
        .unwrap(),
    ])
    .unwrap()
}

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = source();
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
        .execute(&source, &[], Default::default())?
        .rows()
        .to_vec())
}

#[test]
fn default_date_numeric_predicates_compose_through_query_consumers() {
    for predicate in [
        "Moment=.25",
        ".25=Moment",
        "Moment IN (.25)",
        "Moment BETWEEN .25 AND .25",
        "Moment IN (SELECT .25)",
        "Moment=ANY(SELECT .25)",
        "Moment=(SELECT .25)",
    ] {
        let sql = format!("SELECT ID FROM T WHERE {predicate} ORDER BY ID");
        assert_eq!(
            run(&sql).unwrap(),
            [vec![Value::Int32(2)], vec![Value::Int32(3)]]
        );
    }
    assert_eq!(
        run("SELECT A.ID FROM T A INNER JOIN (SELECT .25 AS N) B ON A.Moment=B.N ORDER BY A.ID")
            .unwrap(),
        [vec![Value::Int32(2)], vec![Value::Int32(3)]]
    );
    assert_eq!(
        run("SELECT DCount('*','T','Moment=.25')").unwrap(),
        [vec![Value::Int64(2)]]
    );
    assert_eq!(
        run("SELECT Count(*) FROM T GROUP BY Moment HAVING Moment=.25").unwrap(),
        [vec![Value::Int64(2)]]
    );
}

#[test]
fn decimal_selection_range_checks_and_late_failures_remain_distinct() {
    for (predicate, id) in [("Moment=CDec(.25)", 3), ("Moment=CDec(-.25)", 2)] {
        assert_eq!(
            run(&format!("SELECT ID FROM T WHERE {predicate}")).unwrap(),
            [vec![Value::Int32(id)]]
        );
    }
    assert_eq!(
        run("SELECT ID FROM T WHERE Moment<2958466"),
        Err(EvalError::Overflow.into())
    );
    assert_eq!(
        run("SELECT ID FROM T WHERE ID=0 AND Moment<2958466"),
        Err(EvalError::Overflow.into())
    );
    assert_eq!(
        run("SELECT ID FROM T WHERE Moment IN (.25,1/0)"),
        Err(EvalError::DivisionByZero.into())
    );
    assert_eq!(
        run("SELECT Count(*) FROM T WHERE Moment<CDec(2958466)").unwrap(),
        [vec![Value::Int64(4)]]
    );
}
