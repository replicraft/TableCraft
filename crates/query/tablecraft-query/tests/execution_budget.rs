use tablecraft_core::values::Value;
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

#[test]
fn larger_ordered_reads_need_an_explicit_shared_work_allowance() {
    assert_eq!(QueryLimits::default().steps, 1_000_000);
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("N".into(), ScalarType::Int32, false).unwrap()],
            (1..=20_000)
                .map(|value| vec![Value::Int32(value)])
                .collect(),
        )
        .unwrap(),
    ])
    .unwrap();
    let parsed = parse_query("SELECT TOP 3 N FROM T ORDER BY N DESC", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert!(matches!(
        plan.execute(&source, &[], Default::default()),
        Err(QueryError::Limit(_))
    ));
    let options = ExecutionOptions {
        limits: QueryLimits {
            steps: 10_000_000,
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        plan.execute(&source, &[], options).unwrap().rows(),
        &[
            vec![Value::Int32(20000)],
            vec![Value::Int32(19999)],
            vec![Value::Int32(19998)]
        ]
    );
    let cancelled = || true;
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancelled),
                ..options
            }
        )
        .err(),
        Some(QueryError::Cancelled)
    );
}
