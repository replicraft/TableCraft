use std::cell::Cell;
use tablecraft_core::values::{Decimal, ExtendedDateTime, Guid, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
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
fn cvar_retains_scopes_grouping_aggregate_and_domain_dependencies() {
    assert_eq!(
        run("SELECT (SELECT CVar(A.N)) FROM (SELECT 1 AS N UNION ALL SELECT 2 AS N) AS A ORDER BY A.N").unwrap(),
        [vec![Value::Int64(1)], vec![Value::Int64(2)]]
    );
    assert_eq!(
        run("SELECT CVar(A.N),Count(*) FROM (SELECT 1 AS N UNION ALL SELECT 1 AS N) AS A GROUP BY CVar(A.N)").unwrap(),
        [vec![Value::Int64(1), Value::Int64(2)]]
    );
    assert_eq!(
        run("SELECT CVar(Sum(A.N)) FROM (SELECT 1 AS N UNION ALL SELECT 2 AS N) AS A").unwrap(),
        [vec![Value::Int64(3)]]
    );
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("N".into(), ScalarType::Int32, false).unwrap()],
            [1, 2, 4].map(|n| vec![Value::Int32(n)]).to_vec(),
        )
        .unwrap(),
    ])
    .unwrap();
    let parsed = parse_query("SELECT DSum('CVar(N)','T')", Default::default()).unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        [vec![Value::Int64(7)]]
    );

    for sql in ["SELECT CVar(1/0)", "SELECT IIf(False,CVar(1/0),0)"] {
        assert_eq!(run(sql).err(), Some(EvalError::DivisionByZero.into()));
    }
    for sql in [
        "SELECT CVar(1,1/0)",
        "SELECT CVar(*)",
        "SELECT CVar(Array(1,2))",
    ] {
        assert_eq!(run(sql).err(), Some(QueryError::Unsupported));
    }
}

#[test]
fn cvar_parameter_values_are_not_cached_or_retyped() {
    let source = MemorySource::default();
    let parsed = parse_query("PARAMETERS P DECIMAL; SELECT CVar(P)", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for coefficient in [12300, -1] {
        let value = Value::Decimal(Decimal::new(coefficient, 4).unwrap());
        let result = plan
            .execute(&source, &[("P", value.clone())], Default::default())
            .unwrap();
        assert_eq!(result.rows(), [vec![value]]);
    }
}

#[test]
fn cvar_does_not_fold_clocks_or_skip_execution_cancellation() {
    let source = MemorySource::default();
    let parsed = parse_query("SELECT CVar(Now())", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(
        plan.execute(&source, &[], Default::default()).err(),
        Some(EvalError::ClockRequired.into())
    );

    let parsed = parse_query("SELECT CVar(1)", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let checkpoints = Cell::new(0);
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&|| {
                checkpoints.set(checkpoints.get() + 1);
                false
            }),
            ..Default::default()
        },
    )
    .unwrap();

    for stop in 1..=checkpoints.get() {
        let calls = Cell::new(0);
        let result = plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&|| {
                    calls.set(calls.get() + 1);
                    calls.get() == stop
                }),
                ..Default::default()
            },
        );
        assert!(matches!(
            result,
            Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }
}

#[test]
fn cvar_preserves_binary_and_native_extension_carriers_without_coercion() {
    for value in [
        Value::Binary(vec![0, 255, 127]),
        Value::Guid(Guid::from_bytes([0xab; 16])),
        Value::DateTimeExtended(
            ExtendedDateTime::new(2026, 10, 21, 12, 34, 56, 1_234_567).unwrap(),
        ),
    ] {
        let source = MemorySource::new(vec![
            MemoryTable::new(
                "T".into(),
                vec![Column::new("V".into(), ScalarType::of(&value).unwrap(), false).unwrap()],
                vec![vec![value.clone()]],
            )
            .unwrap(),
        ])
        .unwrap();
        let parsed = parse_query("SELECT CVar(V) FROM T", Default::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap();
        assert_eq!(result.rows(), [vec![value]]);
    }
}
