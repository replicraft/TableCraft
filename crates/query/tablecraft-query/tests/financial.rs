use std::cell::Cell;
use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};
fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            ["Unused", "N"]
                .map(|n| Column::new(n.into(), ScalarType::Int32, false).unwrap())
                .to_vec(),
            [1, 2, 4]
                .into_iter()
                .map(|n| vec![Value::Int32(999), Value::Int32(n)])
                .collect(),
        )
        .unwrap(),
    ])
    .unwrap()
}
fn double(value: f64) -> Value {
    Value::Double(Double::from_float(value))
}
fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let data = source();
    let query = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(query.query(), &data)?
        .execute(&data, &[], Default::default())?
        .rows()
        .to_vec())
}
#[test]
fn financial_calls_retain_dependencies_group_identity_and_correlated_slots() {
    assert_eq!(
        run("SELECT Pmt(0,N,120) FROM T ORDER BY N").unwrap(),
        [vec![double(-120.)], vec![double(-60.)], vec![double(-30.)]]
    );
    assert_eq!(
        run("SELECT N,FV(0,N,-10) FROM T WHERE PV(0,N,-10)>10 GROUP BY N ORDER BY N").unwrap(),
        [
            vec![Value::Int32(2), double(20.)],
            vec![Value::Int32(4), double(40.)]
        ]
    );
    assert_eq!(
        run("SELECT FV(0,N,-10) FROM T GROUP BY FV(0,N,-10) ORDER BY 1").unwrap(),
        [vec![double(10.)], vec![double(20.)], vec![double(40.)]]
    );
    assert_eq!(
        run("SELECT (SELECT PV(0,T.N,-10)) FROM T ORDER BY N").unwrap(),
        [vec![double(10.)], vec![double(20.)], vec![double(40.)]]
    );
    assert_eq!(
        run("SELECT DSum('FV(0,N,-10)','T')").unwrap(),
        [vec![double(70.)]]
    );
    assert_eq!(
        run("SELECT Pmt(0,10,100,,1) UNION ALL SELECT PV(0,10,-100,,)").unwrap(),
        [vec![double(-10.)], vec![double(1000.)]]
    );
}
#[test]
fn parameters_reuse_plans_and_late_errors_cannot_be_hidden_by_top_or_nulls() {
    let data = source();
    let parsed = parse_query("PARAMETERS P LONG; SELECT FV(0,P,-10)", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &data).unwrap();
    for n in [1, 10] {
        assert_eq!(
            plan.execute(&data, &[("P", Value::Int32(n))], Default::default())
                .unwrap()
                .rows(),
            [vec![double(f64::from(n) * 10.)]]
        );
    }
    for sql in [
        "SELECT TOP 1 Pmt(0,N-2,120) FROM T",
        "SELECT Pmt(Null,10,100,1/0)",
        "SELECT IIf(False,Pmt(0,0,100),1)",
    ] {
        assert_eq!(
            run(sql).err(),
            Some(EvalError::DivisionByZero.into()),
            "{sql}"
        );
    }
    for sql in [
        "SELECT Pmt(0,N) FROM T",
        "SELECT FV(0,,N) FROM T",
        "SELECT PV(0,10,N,0,2) FROM T",
    ] {
        assert!(run(sql).is_err(), "{sql}");
    }
}
#[test]
fn financial_calls_remain_under_cumulative_query_limits_and_all_observed_checkpoints() {
    let data = source();
    let parsed = parse_query(
        "SELECT Pmt(.01,N,120,,1),FV(.01,N,-10) FROM T",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &data).unwrap();
    let calls = Cell::new(0);
    plan.execute(
        &data,
        &[],
        ExecutionOptions {
            cancelled: Some(&|| {
                calls.set(calls.get() + 1);
                false
            }),
            ..Default::default()
        },
    )
    .unwrap();
    for stop in 1..=calls.get() {
        let count = Cell::new(0);
        let error = plan
            .execute(
                &data,
                &[],
                ExecutionOptions {
                    cancelled: Some(&|| {
                        count.set(count.get() + 1);
                        count.get() == stop
                    }),
                    ..Default::default()
                },
            )
            .err();
        assert!(
            matches!(
                error,
                Some(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "{stop}: {error:?}"
        );
    }
    assert!(
        plan.execute(
            &data,
            &[],
            ExecutionOptions {
                limits: QueryLimits {
                    steps: 10,
                    ..Default::default()
                },
                ..Default::default()
            }
        )
        .is_err()
    );
}
