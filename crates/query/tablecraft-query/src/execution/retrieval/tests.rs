//! Shared nesting state restores even when an inner read refuses or fails.
use super::*;
use crate::{
    discovery::DiscoveryState,
    execution::{
        ExecutionOptions,
        execute::{ExecutionContext, ExecutionFrame},
    },
    external_sources::SourceNamespace,
    plan::{CrosstabPlan, DomainFunction, Expr, PreparedBody},
    prepare,
    profile::MAX_RUNTIME_READ_DEPTH,
    source::{MemorySource, MemoryTable, SourceKey},
};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{ExpressionValue, Value},
};
use tablecraft_expressions::EvalError;
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{parse_query, select::Expression};

fn check_reads(apply: impl FnOnce(&ReadPlan, &ReadPlan, &CrosstabPlan, &mut Runtime<'_, '_>)) {
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("N".into(), ScalarType::Int32, false).unwrap()],
            vec![vec![Value::Int32(7)]],
        )
        .unwrap(),
    ])
    .unwrap();
    let good_syntax = parse_query("SELECT N FROM T", Default::default()).unwrap();
    let good = prepare(good_syntax.query(), &source).unwrap();
    let bad_syntax = parse_query("SELECT 1/(N-N) FROM T", Default::default()).unwrap();
    let bad = prepare(bad_syntax.query(), &source).unwrap();
    let cross_syntax = parse_query(
        "TRANSFORM Count(*) SELECT N FROM T GROUP BY N PIVOT 1 IN (1)",
        Default::default(),
    )
    .unwrap();
    let cross = prepare(cross_syntax.query(), &source).unwrap();
    let discovery = DiscoveryState::new(false);
    let mut budget = ResourceBudget::new(1_000_000, 100_000, 1_000_000);
    let mut runtime = good
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
    let (PreparedBody::Read(good), PreparedBody::Read(bad)) = (&good.body, &bad.body) else {
        panic!("read fixtures")
    };
    let PreparedBody::Crosstab(cross) = &cross.body else {
        panic!("crosstab fixture")
    };
    apply(good, bad, cross, &mut runtime);
}

#[test]
fn read_limit_refusal_preserves_shared_depth_and_lexical_owner() {
    check_reads(|good, _, _, runtime| {
        let caller = SourceNamespace::External(SourceKey::new(99));
        runtime.owner = caller;
        runtime
            .discovery
            .active_read_depth
            .set(MAX_RUNTIME_READ_DEPTH);
        assert_eq!(
            runtime.read(good, &[]).err(),
            Some(QueryError::StructureLimit)
        );
        assert_eq!(
            runtime.discovery.active_read_depth.get(),
            MAX_RUNTIME_READ_DEPTH
        );
        assert!(runtime.owner == caller);

        runtime.discovery.active_read_depth.set(0);
        assert!(runtime.read(good, &[]).is_ok());
        assert_eq!(runtime.discovery.active_read_depth.get(), 0);
        assert!(runtime.owner == caller);
    });
}

#[test]
fn inner_evaluation_failure_releases_shared_depth_before_a_later_read() {
    check_reads(|good, bad, _, runtime| {
        let caller = SourceNamespace::External(SourceKey::new(99));
        runtime.owner = caller;
        assert_eq!(
            runtime.read(bad, &[]).err(),
            Some(QueryError::Evaluation(EvalError::DivisionByZero))
        );
        assert_eq!(runtime.discovery.active_read_depth.get(), 0);
        assert!(runtime.owner == caller);
        assert!(runtime.read(good, &[]).is_ok());
        assert_eq!(runtime.discovery.active_read_depth.get(), 0);
        assert!(runtime.owner == caller);
    });
}

#[test]
fn domain_failure_releases_operation_scope_before_another_call() {
    check_reads(|_, _, _, runtime| {
        let arguments = |expression: &str| {
            [
                Expr::Literal(Expression::Text(expression.into())),
                Expr::Literal(Expression::Text("T".into())),
            ]
        };
        let environment = Environment {
            aggregate_results: None,
            collections: None,
            row: &[],
            outer: &[],
            output: &[],
            group: None,
            group_keys: None,
        };
        assert_eq!(
            runtime
                .domain_expression(DomainFunction::Count, &arguments("1/0"), environment)
                .err(),
            Some(QueryError::Evaluation(EvalError::DivisionByZero)),
        );
        assert_eq!(runtime.discovery.active_domain_depth.get(), 0);
        assert_eq!(runtime.discovery.active_read_depth.get(), 0);
        let result = runtime
            .domain_expression(DomainFunction::Count, &arguments("*"), environment)
            .unwrap();
        assert!(matches!(
            result.value,
            ExpressionValue::Value(Value::Int64(1))
        ));
        assert_eq!(runtime.discovery.active_domain_depth.get(), 0);
        assert_eq!(runtime.discovery.active_read_depth.get(), 0);
    });
}

#[test]
fn direct_crosstab_counts_once_and_internal_read_scope_is_not_recounted() {
    check_reads(|_, _, cross, runtime| {
        let caller = SourceNamespace::External(SourceKey::new(99));
        runtime.owner = caller;
        runtime
            .discovery
            .active_read_depth
            .set(MAX_RUNTIME_READ_DEPTH);
        assert_eq!(
            runtime.crosstab(cross).err(),
            Some(QueryError::StructureLimit)
        );
        assert_eq!(
            runtime.discovery.active_read_depth.get(),
            MAX_RUNTIME_READ_DEPTH
        );
        assert!(runtime.owner == caller);

        // Runtime::read has already counted this scope for a named crosstab.
        assert!(runtime.crosstab_in_read(cross).is_ok());
        assert_eq!(
            runtime.discovery.active_read_depth.get(),
            MAX_RUNTIME_READ_DEPTH
        );
        assert!(runtime.owner == caller);

        runtime.discovery.active_read_depth.set(0);
        assert!(runtime.crosstab(cross).is_ok());
        assert_eq!(runtime.discovery.active_read_depth.get(), 0);
        assert!(runtime.owner == caller);
    });
}
