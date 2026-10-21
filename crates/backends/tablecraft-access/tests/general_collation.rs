use std::{cell::Cell, cmp::Ordering};
use tablecraft_access::GeneralCollation;
use tablecraft_ace::{InspectError, NativeSortOrder};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::{
    EvalError,
    comparison::{Collation, StringCompareMode, StringSearch},
};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};

const INPUT: &str = "(SELECT 0 AS ID, NULL AS V UNION ALL SELECT 1,'' UNION ALL SELECT 2,'a' UNION ALL SELECT 3,'A ' UNION ALL SELECT 4,'b' UNION ALL SELECT 5,'é' UNION ALL SELECT 6,'é' UNION ALL SELECT 7,'e' UNION ALL SELECT 8,'ss' UNION ALL SELECT 9,'ß')";

fn policy(version: u8) -> GeneralCollation<'static> {
    GeneralCollation::new(
        NativeSortOrder::from_native_bytes([9, 4, 0, version]),
        ResourceBudget::new(16 * 1024 * 1024, 0, 1_000_000),
        &|| false,
    )
    .unwrap()
}
fn rows(sql: &str, policy: &GeneralCollation<'_>) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(policy),
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}
fn integers(rows: &[&[i64]]) -> Vec<Vec<Value>> {
    rows.iter()
        .map(|row| row.iter().map(|n| Value::Int64(*n)).collect())
        .collect()
}

#[test]
fn comparisons_remain_consistent_across_query_operations() {
    for version in [0, 1] {
        for (sql, expected) in [
            (
                format!("SELECT q.ID FROM {INPUT} AS q WHERE q.V=(SELECT 'A') ORDER BY q.ID"),
                integers(&[&[2], &[3]]),
            ),
            (
                format!("SELECT q.ID FROM {INPUT} AS q WHERE q.V='a' ORDER BY q.ID"),
                integers(&[&[2], &[3]]),
            ),
            (
                format!("SELECT q.ID FROM {INPUT} AS q ORDER BY q.V,q.ID"),
                integers(&[&[0], &[1], &[2], &[3], &[4], &[7], &[5], &[6], &[8], &[9]]),
            ),
            (
                format!(
                    "SELECT Min(q.ID) AS I,Count(*) AS N FROM {INPUT} AS q GROUP BY q.V ORDER BY Min(q.ID)"
                ),
                integers(&[
                    &[0, 1],
                    &[1, 1],
                    &[2, 2],
                    &[4, 1],
                    &[5, 2],
                    &[7, 1],
                    &[8, 2],
                ]),
            ),
            (
                format!(
                    "SELECT Min(q.ID) AS I,Count(*) AS N FROM {INPUT} AS q GROUP BY q.V HAVING q.V='a'"
                ),
                integers(&[&[2, 2]]),
            ),
            (
                format!("SELECT Count(*) AS N FROM (SELECT DISTINCT q.V FROM {INPUT} AS q) AS d"),
                integers(&[&[7]]),
            ),
            (
                format!(
                    "SELECT a.ID,b.ID AS B FROM {INPUT} AS a INNER JOIN {INPUT} AS b ON a.V=b.V WHERE a.ID<b.ID ORDER BY a.ID,b.ID"
                ),
                integers(&[&[2, 3], &[5, 6], &[8, 9]]),
            ),
            (
                "SELECT Count(*) AS N FROM (SELECT 'a' AS V UNION SELECT 'A ') AS q".into(),
                integers(&[&[1]]),
            ),
            (
                format!("SELECT q.ID FROM {INPUT} AS q WHERE q.V IN ('A','É') ORDER BY q.ID"),
                integers(&[&[2], &[3], &[5], &[6]]),
            ),
            (
                format!(
                    "SELECT q.ID FROM {INPUT} AS q WHERE q.V BETWEEN 'A' AND 'B ' ORDER BY q.ID"
                ),
                integers(&[&[2], &[3], &[4]]),
            ),
            (
                format!("SELECT TOP 1 q.ID FROM {INPUT} AS q WHERE q.V<>'' ORDER BY q.V"),
                integers(&[&[2], &[3]]),
            ),
        ] {
            assert_eq!(
                rows(&sql, &policy(version)).unwrap(),
                expected,
                "General {version}: {sql}"
            );
        }
        assert_eq!(
            rows(
                &format!("SELECT Min(q.V) AS Low,Max(q.V) AS High FROM {INPUT} AS q"),
                &policy(version)
            )
            .unwrap(),
            vec![vec![Value::Text("".into()), Value::Text("ss".into())]]
        );
    }
}

#[test]
fn comparison_budget_is_cumulative_across_callbacks_and_executions() {
    let policy = GeneralCollation::new(
        NativeSortOrder::from_native_bytes([9, 4, 0, 1]),
        ResourceBudget::new(1000, 0, 1000),
        &|| false,
    )
    .unwrap();
    assert_eq!(policy.compare("a", "A").unwrap(), Ordering::Equal);
    let mut calls = 1;
    loop {
        calls += 1;
        match policy.compare("a", "A") {
            Ok(_) => assert!(calls < 10),
            Err(EvalError::Limit(_)) => break,
            other => panic!("unexpected comparison result: {other:?}"),
        }
    }
    assert!(matches!(
        rows("SELECT 'a'='A' AS R", &policy),
        Err(QueryError::Evaluation(EvalError::Limit(_)))
    ));
}

#[test]
fn module_text_search_and_text_compare_are_separate_contracts() {
    let policy = policy(1);
    assert_eq!(policy.option_compare(), None);
    assert_eq!(
        policy.compare_text("a", "A"),
        Err(EvalError::CollationRequired)
    );
    assert_eq!(
        policy.find(
            "a",
            "A",
            StringSearch::ForwardFrom(0),
            StringCompareMode::Database
        ),
        Err(EvalError::UnsupportedExpression)
    );
}

#[test]
fn unknown_descriptors_and_cancellation_fail_explicitly() {
    assert!(matches!(
        GeneralCollation::new(
            NativeSortOrder::from_native_bytes([9, 4, 1, 1]),
            ResourceBudget::new(1000, 0, 1000),
            &|| false
        ),
        Err(InspectError::Unsupported)
    ));
    assert!(matches!(
        GeneralCollation::new(
            NativeSortOrder::from_native_bytes([9, 4, 0, 1]),
            ResourceBudget::new(1000, 0, 1000),
            &|| true
        ),
        Err(InspectError::Cancelled)
    ));
    let cancelled = Cell::new(false);
    let callback = || cancelled.get();
    let policy = GeneralCollation::new(
        NativeSortOrder::from_native_bytes([9, 4, 0, 1]),
        ResourceBudget::new(1000, 0, 1000),
        &callback,
    )
    .unwrap();
    cancelled.set(true);
    assert_eq!(policy.compare("a", "A"), Err(EvalError::Cancelled));
}

#[test]
fn reentrant_cancellation_provider_returns_a_typed_error_without_panicking() {
    let active: Cell<Option<&GeneralCollation<'_>>> = Cell::new(None);
    let attempts = Cell::new(0);
    let callback = || {
        if let Some(policy) = active.get() {
            attempts.set(attempts.get() + 1);
            assert_eq!(policy.compare("a", "A"), Err(EvalError::CollationFailed));
        }
        false
    };
    let policy = GeneralCollation::new(
        NativeSortOrder::from_native_bytes([9, 4, 0, 1]),
        ResourceBudget::new(1000, 0, 1000),
        &callback,
    )
    .unwrap();
    active.set(Some(&policy));
    assert_eq!(policy.compare("a", "A").unwrap(), Ordering::Equal);
    assert!(attempts.get() > 0);
}
