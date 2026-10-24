use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

fn source(text: String) -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("S".into(), ScalarType::Text, false).unwrap()],
            vec![vec![Value::Text(text)]],
        )
        .unwrap(),
    ])
    .unwrap()
}

fn options(limit: usize) -> ExecutionOptions<'static> {
    ExecutionOptions {
        max_text_bytes: Some(limit),
        limits: QueryLimits {
            steps: 10_000_000,
            ..Default::default()
        },
        ..Default::default()
    }
}

#[test]
fn execution_opt_in_preserves_default_and_reuses_prepared_plans() {
    let text = "a".repeat(70_000);
    let source = source(text.clone());
    let parsed = parse_query("SELECT S FROM T", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert!(matches!(
        plan.execute(&source, &[], Default::default()),
        Err(QueryError::Evaluation(EvalError::TextLimit))
    ));
    assert_eq!(
        plan.execute(&source, &[], options(70_000)).unwrap().rows(),
        &[vec![Value::Text(text)]]
    );
    assert!(plan.execute(&source, &[], options(69_999)).is_err());
    assert!(plan.execute(&source, &[], Default::default()).is_err());
}

#[test]
fn nested_and_domain_calls_share_the_selected_limit() {
    let source = source("a".repeat(70_000));
    for sql in [
        "SELECT Len(S),Len((SELECT S)),Len(DLookup('S','T')) FROM T",
        "SELECT Len(Q.S),Len(Left(Q.S,3)) FROM (SELECT S FROM T) AS Q",
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], options(70_000))
            .unwrap();
        let expected = if sql.contains("DLookup") {
            vec![
                Value::Int32(70_000),
                Value::Int32(70_000),
                Value::Int32(70_000),
            ]
        } else {
            vec![Value::Int32(70_000), Value::Int32(3)]
        };
        assert_eq!(result.rows(), &[expected]);
    }
}

#[test]
fn cap_counts_utf8_bytes_and_clamps_to_sixteen_mib() {
    for (text, accepted) in [
        ("é".repeat(8_388_608), true),
        ("a".repeat(16_777_217), false),
    ] {
        let source = source(text);
        let parsed = parse_query("SELECT Len(S) FROM T", Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        let mut options = options(usize::MAX);
        options.limits.bytes = 268_435_456;
        options.limits.steps = 100_000_000;
        assert_eq!(plan.execute(&source, &[], options).is_ok(), accepted);
    }
}

#[test]
fn parameters_and_generated_intermediates_obey_execution_limit() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS P TEXT; SELECT Len(P),Len((SELECT P))",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let bindings = [("P", Value::Text("x".repeat(70_000)))];
    assert!(
        plan.execute(&source, &bindings, Default::default())
            .is_err()
    );
    assert_eq!(
        plan.execute(&source, &bindings, options(70_000))
            .unwrap()
            .rows(),
        &[vec![Value::Int32(70_000), Value::Int32(70_000)]]
    );
    let parsed = parse_query("SELECT Len(String(70000,'a') & 'b')", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert!(plan.execute(&source, &[], options(70_000)).is_err());
    assert_eq!(
        plan.execute(&source, &[], options(70_001)).unwrap().rows(),
        &[vec![Value::Int32(70_001)]]
    );
}

#[test]
fn explicit_text_size_does_not_waive_allocations_work_or_cancellation() {
    let source = source("a".repeat(70_000));
    let parsed = parse_query("SELECT Len(S) FROM T", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for limits in [
        QueryLimits {
            bytes: 100,
            ..Default::default()
        },
        QueryLimits {
            steps: 5,
            ..Default::default()
        },
    ] {
        assert!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    limits,
                    ..options(70_000)
                }
            )
            .is_err()
        );
    }
    let calls = Cell::new(0);
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() > 20
    };
    assert!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancelled),
                ..options(70_000)
            }
        )
        .is_err()
    );
}
