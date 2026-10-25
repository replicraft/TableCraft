//! Retention probes distinguish dropped wide rows from merely pruned sources.
use super::*;
use crate::{
    OutputType, QueryError,
    discovery::DiscoveryState,
    execution::{
        ExecutionOptions, Runtime,
        cell::CellValue,
        execute::{ExecutionContext, ExecutionFrame},
    },
    plan::ProfiledType,
    prepare,
    source::{MemorySource, MemoryTable},
};
use std::cmp::Ordering;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Double, ExpressionValue, LegacyDateTime, Value},
};
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

struct Ordinal;

impl Collation for Ordinal {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

#[test]
fn count_and_key_only_capture_retains_no_source_row_or_assignment_capacity() {
    for (projection, having, retain) in [
        ("K,Count(*)", "", false),
        ("K", "", false),
        ("K,Count(*)", "HAVING Count(*)>1", false),
        ("K,Sum(N)", "", true),
        ("K", "HAVING Sum(N)>0", true),
        ("K,Sum(N)", "HAVING Count(*)>0", true),
    ] {
        let source = MemorySource::new(vec![
            MemoryTable::new(
                "T".into(),
                vec![
                    Column::new("K".into(), ScalarType::Text, false).unwrap(),
                    Column::new("N".into(), ScalarType::Int32, false).unwrap(),
                    Column::new("Payload".into(), ScalarType::Text, false).unwrap(),
                ],
                ["a", "b", "a"]
                    .into_iter()
                    .map(|key| {
                        vec![
                            Value::Text(key.into()),
                            Value::Int32(1),
                            Value::Text("x".repeat(4096)),
                        ]
                    })
                    .collect(),
            )
            .unwrap(),
        ])
        .unwrap();
        // Every selected inner field executes and reaches the terminal layer;
        // native/source field pruning cannot explain the no-row assertion.
        let sql =
            format!("SELECT {projection} FROM (SELECT K,N,Payload FROM T) Q GROUP BY K {having}");
        let syntax = parse_query(&sql, Default::default()).unwrap();
        let prepared = prepare(syntax.query(), &source).unwrap();
        let discovery = DiscoveryState::new(false);
        let mut budget = ResourceBudget::new(1_000_000, 100_000, 1_000_000);
        let mut runtime: Runtime<'_, '_> = prepared
            .initialize_runtime(
                ExecutionFrame {
                    source: &source,
                    parameters: &[],
                    options: ExecutionOptions {
                        collation: Some(&Ordinal),
                        ..Default::default()
                    },
                    discovery: &discovery,
                    context: ExecutionContext {
                        strict_parameters: true,
                        repeatable_dynamic: false,
                    },
                },
                &mut budget,
            )
            .unwrap();
        let mut plan = runtime.progressive_plan(&prepared.body, &[]).unwrap();
        let mut arm = plan.arms.remove(0);
        let mut summary = StreamSummary {
            scanned_rows: 0,
            emitted_rows: 0,
        };
        let mut captured = runtime
            .capture_text_groups::<()>(&mut arm, &[], &mut summary)
            .unwrap();
        assert_eq!(summary.scanned_rows, 3);
        assert_eq!(captured.candidates.len(), 3);
        for candidate in &captured.candidates {
            assert_eq!(candidate.row.is_some(), retain, "{sql}");
            if let Some(row) = &candidate.row {
                assert_eq!(row.len(), 3);
                let ExpressionValue::Value(Value::Text(payload)) = &row[2].value else {
                    panic!("retained original wide row");
                };
                assert_eq!(payload.len(), 4096);
            }
        }
        let partition = runtime.partition_text_groups(&mut captured).unwrap();
        assert_eq!(partition.groups.len(), 2);
        if retain {
            assert_eq!(partition.rows.len(), 3);
        } else {
            assert_eq!(partition.rows.len(), 0);
            assert_eq!(partition.rows.capacity(), 0);
        }
    }
}

#[test]
fn actual_key_validation_preserves_error_data_but_refuses_wrong_successful_carriers() {
    let source = MemorySource::new(vec![]).unwrap();
    let syntax = parse_query("SELECT 1", Default::default()).unwrap();
    let prepared = prepare(syntax.query(), &source).unwrap();
    let discovery = DiscoveryState::new(false);
    let mut budget = ResourceBudget::new(1_000_000, 100_000, 1_000_000);
    let mut runtime = prepared
        .initialize_runtime(
            ExecutionFrame {
                source: &source,
                parameters: &[],
                options: ExecutionOptions::default(),
                discovery: &discovery,
                context: ExecutionContext {
                    strict_parameters: true,
                    repeatable_dynamic: false,
                },
            },
            &mut budget,
        )
        .unwrap();
    let types = [ProfiledType::uniform(OutputType::scalar(
        ScalarType::Text,
        true,
    ))];
    for value in [
        Value::Double(Double::from_float(1.0)),
        Value::DateTime(LegacyDateTime::from_ole_days(1.0).unwrap()),
        Value::Int32(1),
    ] {
        let row = [CellValue::scalar(value)];
        assert!(matches!(
            runtime.validate_text_group_keys(&row, &types),
            Err(QueryError::Source)
        ));
    }
    let error = [CellValue::from(ExpressionValue::Error(7))];
    runtime.validate_text_group_keys(&error, &types).unwrap();
    let invalid = [CellValue::from(ExpressionValue::Error(u32::MAX))];
    assert!(matches!(
        runtime.validate_text_group_keys(&invalid, &types),
        Err(QueryError::Evaluation(EvalError::UnsupportedExpression))
    ));
    let null = [CellValue::scalar(Value::Null)];
    runtime.validate_text_group_keys(&null, &types).unwrap();
}
