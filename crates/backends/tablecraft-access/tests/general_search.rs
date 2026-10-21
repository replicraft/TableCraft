use std::cell::Cell;
use tablecraft_access::GeneralCollation;
use tablecraft_ace::NativeSortOrder;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::{
    EvalError,
    comparison::{Collation, StringCompareMode, StringSearch},
};
use tablecraft_query::{ExecutionOptions, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};

fn policy(version: u8) -> GeneralCollation<'static> {
    GeneralCollation::new(
        NativeSortOrder::from_native_bytes([9, 4, 0, version]),
        ResourceBudget::new(16 * 1024 * 1024, 0, 1_000_000),
        &|| false,
    )
    .unwrap()
}
#[test]
fn scoped_query_search_and_replacement_consume_actual_expansion_spans() {
    for version in [0, 1] {
        let source = MemorySource::default();
        let parsed = parse_query("SELECT InStr(1,'Straße STRASSE','ss',2) AS [First],InStrRev('Straße STRASSE','ss',-1,2) AS [Last],Replace('SS ß','ß','x',1,-1,2) AS Replaced", ParseLimits::default()).unwrap();
        let policy = policy(version);
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(&policy),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            result.rows(),
            &[vec![
                Value::Int32(5),
                Value::Int32(12),
                Value::Text("x x".into())
            ]]
        );
        let parsed = parse_query("SELECT q.ID FROM (SELECT 1 AS ID,'Straße' AS V UNION ALL SELECT 2,'road') AS q WHERE EXISTS(SELECT 1 WHERE InStr(1,q.V,'ss',2)>0)", ParseLimits::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(&policy),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.rows(), &[vec![Value::Int64(1)]]);
    }
}
#[test]
fn search_shares_comparison_budget_and_callback_cancellation() {
    let policy = GeneralCollation::new(
        NativeSortOrder::from_native_bytes([9, 4, 0, 1]),
        ResourceBudget::new(1000, 0, 1000),
        &|| false,
    )
    .unwrap();
    let mut calls = 0;
    loop {
        calls += 1;
        match policy.find(
            "ss",
            "ß",
            StringSearch::ForwardFrom(0),
            StringCompareMode::Database,
        ) {
            Ok(Some(found)) => {
                assert_eq!(found, 0..2);
                assert!(calls < 10);
            }
            Err(EvalError::Limit(_)) => break,
            other => panic!("unexpected result {other:?}"),
        }
    }
    assert!(matches!(policy.compare("a", "A"), Err(EvalError::Limit(_))));
    let cancelled = Cell::new(false);
    let callback = || cancelled.get();
    let policy = GeneralCollation::new(
        NativeSortOrder::from_native_bytes([9, 4, 0, 1]),
        ResourceBudget::new(1000, 0, 1000),
        &callback,
    )
    .unwrap();
    cancelled.set(true);
    assert_eq!(
        policy.find(
            "ss",
            "ß",
            StringSearch::ForwardFrom(0),
            StringCompareMode::Database
        ),
        Err(EvalError::Cancelled)
    );
}
