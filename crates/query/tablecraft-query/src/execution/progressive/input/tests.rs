use super::*;
use crate::{
    discovery::DiscoveryState,
    execution::{
        ExecutionOptions,
        execute::{ExecutionContext, ExecutionFrame},
    },
    plan::{PreparedBody, ReadNode},
    prepare,
    source::{MemorySource, MemoryTable, SourceKey},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

fn with_runtime(sql: &str, test: impl FnOnce(&PreparedBody, &mut Runtime<'_, '_>)) {
    let table = MemoryTable::new(
        "T".into(),
        vec![Column::new("N".into(), ScalarType::Int32, false).unwrap()],
        vec![vec![Value::Int32(7)]],
    )
    .unwrap();
    let source = MemorySource::new(vec![table]).unwrap();
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
    test(&prepared.body, &mut runtime);
}

#[test]
fn owner_scopes_restore_the_caller_after_nested_query_and_sink_failures() {
    with_runtime("SELECT N FROM T", |_, runtime| {
        let first = SourceNamespace::External(SourceKey::new(1));
        let second = SourceNamespace::External(SourceKey::new(2));
        let failed = runtime.with_progressive_owner(first, |runtime| {
            assert!(runtime.owner == first);
            let inner = runtime.with_progressive_owner(second, |runtime| {
                assert!(runtime.owner == second);
                Err::<(), _>(QueryError::Cancelled)
            });
            assert_eq!(inner, Err(QueryError::Cancelled));
            assert!(runtime.owner == first);
            Err::<(), _>(StreamError::Sink("original sink"))
        });
        assert!(matches!(failed, Err(StreamError::Sink("original sink"))));
        assert!(runtime.owner == SourceNamespace::Root);
    });
}

#[test]
fn direct_inputs_allocate_no_frame_storage_and_a_layer_prepays_its_actual_width() {
    for (sql, count) in [
        ("SELECT Count(*) FROM T", 0),
        ("SELECT N FROM (SELECT N FROM T) Q", 1),
    ] {
        with_runtime(sql, |body, runtime| {
            let PreparedBody::Read(read) = body else {
                panic!("read fixture")
            };
            let ReadNode::Select(select) = &read.node else {
                panic!("select fixture")
            };
            let before = runtime.budget.remaining(Resource::Bytes);
            let input = runtime.progressive_input(&select.source).unwrap();
            assert_eq!(input.frames.len(), count);
            assert_eq!(input.frames.capacity(), count);
            assert_eq!(
                before - runtime.budget.remaining(Resource::Bytes),
                (count * size_of::<Frame<'_>>()) as u64
            );
        });
    }
}
