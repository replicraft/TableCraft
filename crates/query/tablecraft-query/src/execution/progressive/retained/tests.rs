//! Retained receivers preserve semantic state before the public scalar boundary.
use super::*;
use crate::{
    discovery::DiscoveryState,
    execution::{
        ExecutionOptions, Runtime,
        execute::{ExecutionContext, ExecutionFrame},
    },
    plan::PreparedBody,
    prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{ExpressionValue, Value},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

fn with_runtime(sql: &str, apply: impl FnOnce(&PreparedBody, &mut Runtime<'_, '_>)) {
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("N".into(), ScalarType::Int32, false).unwrap()],
            vec![],
        )
        .unwrap(),
    ])
    .unwrap();
    let syntax = parse_query(sql, Default::default()).unwrap();
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
    apply(&prepared.body, &mut runtime);
}

#[test]
fn retained_global_finish_preserves_unknown_extrema_declaration_and_raw_string() {
    with_runtime("SELECT Min(CInt(N)),ChrW(55296) FROM T", |body, runtime| {
        let mut plan = runtime.progressive_plan(body, &[]).unwrap();
        let arm = plan.arms.remove(0);
        let output = StageOutput {
            read: arm.read,
            select: arm.select,
            aggregates: &arm.aggregates,
        };
        let mut state = runtime.global_input(output).unwrap();
        for number in [2, 1] {
            runtime
                .push_global_input(
                    output,
                    &mut state,
                    vec![CellValue::scalar(Value::Int32(number))],
                )
                .unwrap();
        }
        let mut received = 0;
        runtime
            .finish_global_cells::<()>(output, state, &mut |_, cells| {
                received += 1;
                assert!(cells[0].declaration.is_none());
                assert!(matches!(
                    cells[0].value,
                    ExpressionValue::Value(Value::Int16(1))
                ));
                assert!(matches!(cells[1].value, ExpressionValue::StringBytes(_)));
                Ok(())
            })
            .unwrap();
        assert_eq!(received, 1);
    });
}

#[test]
fn retained_group_receiver_failure_stops_later_final_expression_errors() {
    with_runtime(
        "SELECT N,1/(Count(*)-1) FROM T GROUP BY N",
        |body, runtime| {
            let mut plan = runtime.progressive_plan(body, &[]).unwrap();
            let mut arm = plan.arms.remove(0);
            let output = StageOutput {
                read: arm.read,
                select: arm.select,
                aggregates: &arm.aggregates,
            };
            let mut state = runtime
                .group_input(output.select, arm.group_lookup.take().unwrap())
                .unwrap();
            for number in [1, 1, 2] {
                runtime
                    .push_group_input(
                        output,
                        &mut state,
                        vec![CellValue::scalar(Value::Int32(number))],
                    )
                    .unwrap();
            }
            let mut received = 0;
            let result = runtime.finish_group_input(output, state, &mut |_, _| {
                received += 1;
                Err(StreamError::Sink("original receiver"))
            });
            assert!(matches!(
                result,
                Err(StreamError::Sink("original receiver"))
            ));
            assert_eq!(received, 1);
        },
    );
}
