use super::support::{Data, derived, extended, prepared, syntax, wrapped};
use std::cell::Cell;
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, PreparationOptions, QueryError, execute_with_schema_discovery, prepare,
    prepare_with_options,
};

#[test]
fn derived_and_named_sources_share_the_selected_structural_limit() {
    let text = derived();
    let source = Data::new(&[("N", &text)]);
    for sql in [text, "SELECT K FROM N".into()] {
        assert_eq!(
            prepare(&syntax(&sql), &source).err(),
            Some(QueryError::StructureLimit)
        );
        let plan = prepared(&sql, &source);
        assert_eq!(source.scans.get(), 0);
        let result = plan.execute(&source, &[], Default::default()).unwrap();
        assert_eq!(
            result.rows(),
            &[
                vec![Value::Int32(2)],
                vec![Value::Int32(1)],
                vec![Value::Int32(3)]
            ]
        );
        source.scans.set(0);
        let staged = execute_with_schema_discovery(
            &syntax(&sql),
            &source,
            &[],
            ExecutionOptions {
                binding_depth: extended(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(staged.rows().len(), 3);
        source.scans.set(0);
    }
}

#[test]
fn larger_binding_depth_still_detects_named_query_cycles_before_scans() {
    let source = Data::new(&[("A", "SELECT K FROM B"), ("B", "SELECT K FROM A")]);
    let result = prepare_with_options(
        &syntax("SELECT K FROM A"),
        &source,
        PreparationOptions {
            binding_depth: extended(),
            ..Default::default()
        },
    );
    assert_eq!(result.err(), Some(QueryError::DependencyCycle));
    assert_eq!(source.scans.get(), 0);
}

#[test]
fn runtime_domain_parsing_uses_the_captured_plan_policy() {
    let source = Data::new(&[]);
    let sql = format!("SELECT DSum('{}','T') AS Total", wrapped("K"));
    let default = prepare(&syntax(&sql), &source).unwrap();
    assert_eq!(
        default
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    binding_depth: extended(),
                    ..Default::default()
                }
            )
            .err(),
        Some(QueryError::StructureLimit)
    );
    assert_eq!(source.scans.get(), 0);

    let result = prepared(&sql, &source)
        .execute(&source, &[], Default::default())
        .unwrap();
    assert_eq!(result.rows(), &[vec![Value::Int64(6)]]);
}

#[test]
fn execution_stage_crosstab_discovery_inherits_the_selected_depth() {
    let definition = format!(
        "TRANSFORM Count(*) SELECT 1 AS G FROM T GROUP BY 1 PIVOT {}",
        wrapped("K")
    );
    let source = Data::new(&[("Piv", &definition)]);
    let root = syntax("SELECT [1] FROM Piv");
    assert!(prepare(&root, &source).is_err());
    assert_eq!(source.scans.get(), 0);

    let result = execute_with_schema_discovery(
        &root,
        &source,
        &[],
        ExecutionOptions {
            binding_depth: extended(),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        result.rows(),
        &[vec![ExpressionValue::Value(Value::Int64(1))]]
    );
    assert_eq!(source.scans.get(), 1);
}

#[test]
fn staged_binding_observes_every_cancellation_checkpoint_before_source_scans() {
    let source = Data::new(&[]);
    let query = syntax(&format!("SELECT {} FROM T", wrapped("K")));
    let visits = Cell::new(0);
    let before_scan = Cell::new(0);
    let count = || {
        visits.set(visits.get() + 1);
        if source.scans.get() == 0 {
            before_scan.set(visits.get());
        }
        false
    };
    execute_with_schema_discovery(
        &query,
        &source,
        &[],
        ExecutionOptions {
            binding_depth: extended(),
            cancelled: Some(&count),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(before_scan.get() > 50);

    for boundary in 1..=before_scan.get() {
        source.scans.set(0);
        visits.set(0);
        let cancelled = || {
            visits.set(visits.get() + 1);
            visits.get() == boundary
        };
        let result = execute_with_schema_discovery(
            &query,
            &source,
            &[],
            ExecutionOptions {
                binding_depth: extended(),
                cancelled: Some(&cancelled),
                ..Default::default()
            },
        );
        assert!(
            matches!(
                result,
                Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "checkpoint {boundary}"
        );
        assert_eq!(source.scans.get(), 0, "checkpoint {boundary}");
    }
}
