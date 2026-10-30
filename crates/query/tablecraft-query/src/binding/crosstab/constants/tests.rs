use super::*;
use crate::Source;
use std::cell::Cell;
use tablecraft_core::limits::Resource;
use tablecraft_schema::Column;
use tablecraft_sql::select::UnaryOperator;

struct NoSource;
impl Source for NoSource {
    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        panic!("constant metadata must not request source columns")
    }
    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("constant metadata must not scan sources")
    }
}

fn binder<'a>(
    budget: Option<&'a mut ResourceBudget>,
    cancelled: Option<&'a dyn Fn() -> bool>,
) -> Binder<'a> {
    Binder {
        inherited_read_depth: 0,
        pending_named_depth: 0,
        next_relation_occurrence: 0,
        binding_depth: Default::default(),
        aliases: Default::default(),
        budget,
        cancelled,
        source: &NoSource,
        external_sources: Vec::new(),
        external_providers: Vec::new(),
        materialized: &[],
        discover: false,
        reference_year: None,
        declared_arithmetic: false,
        pending_schema: None,
        parameters: Vec::new(),
        parameter_scope: Vec::new(),
        retrieval: true,
        mutation_scalar: false,
        collection_criteria: false,
        active_queries: Vec::new(),
        owner: crate::external_sources::SourceNamespace::Root,
        nodes: 0,
        policy_visits: 0,
        bytes: 0,
    }
}

fn text(value: &str) -> Expr {
    Expr::Literal(Expression::Text(value.into()))
}

#[test]
fn text_reservations_and_nodes_consume_the_parent_binding_budget() {
    let input = Expr::Binary(
        BinaryOperator::Concatenate,
        Box::new(text("ab")),
        Box::new(text("cd")),
    );
    for (bytes, steps, resource) in [(1, 1000, Resource::Bytes), (10000, 1, Resource::Steps)] {
        let mut budget = ResourceBudget::new(bytes, 0, steps);
        assert!(
            matches!(binder(Some(&mut budget), None).constant_heading(&input, 1), Err(QueryError::Limit(error)) if error.resource == resource)
        );
    }
    let mut budget = ResourceBudget::new(10000, 0, 1000);
    assert_eq!(
        binder(Some(&mut budget), None)
            .constant_heading(&input, 1)
            .unwrap(),
        Value::Text("abcd".into())
    );
    let small_remaining = budget.remaining(Resource::Bytes);
    assert!(budget.remaining(Resource::Steps) < 1000);
    let larger = Expr::Binary(
        BinaryOperator::Concatenate,
        Box::new(text("abcdef")),
        Box::new(text("ghijkl")),
    );
    let mut larger_budget = ResourceBudget::new(10000, 0, 1000);
    assert_eq!(
        binder(Some(&mut larger_budget), None)
            .constant_heading(&larger, 1)
            .unwrap(),
        Value::Text("abcdefghijkl".into())
    );
    // Identical node shapes cancel structural charges: 24 payload bytes versus 8.
    // This catches omission or duplication of the concatenation reservation.
    assert_eq!(
        small_remaining - larger_budget.remaining(Resource::Bytes),
        16
    );
}

#[test]
fn structural_allocation_depth_and_nonconstant_nodes_fail_without_source_access() {
    let large = Expr::Literal(Expression::Text("a".repeat(1_048_577)));
    assert_eq!(
        binder(None, None).constant_heading(&large, 1),
        Err(QueryError::StructureLimit)
    );
    let mut deep = Expr::Literal(Expression::Integer(1));
    for _ in 0..20 {
        deep = Expr::Unary(UnaryOperator::Plus, Box::new(deep));
    }
    assert_eq!(
        binder(None, None).constant_heading(&deep, 1),
        Err(QueryError::StructureLimit)
    );
    assert_eq!(
        binder(None, None).constant_heading(&Expr::Parameter(0), 1),
        Err(QueryError::Unsupported)
    );
    assert_eq!(
        binder(None, None).constant_heading(&Expr::CurrentUser, 1),
        Err(QueryError::Unsupported)
    );
}

#[test]
fn every_observed_constant_binding_checkpoint_can_cancel() {
    let input = Expr::Binary(
        BinaryOperator::Concatenate,
        Box::new(text("ab")),
        Box::new(text("cd")),
    );
    let calls = Cell::new(0);
    let run = |stop| {
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        binder(None, Some(&cancelled)).constant_heading(&input, 1)
    };
    assert!(run(usize::MAX).is_ok());
    let count = calls.get();
    for stop in 1..=count {
        calls.set(0);
        assert_eq!(run(stop), Err(QueryError::Cancelled), "checkpoint {stop}");
    }
}

#[test]
fn eager_choices_charge_every_payload_and_preserve_deferred_condition_errors() {
    let input = Expr::IIf(
        Box::new(Expr::Literal(Expression::Boolean(true))),
        Box::new(text("selected")),
        Box::new(text("unselected")),
    );
    let mut budget = ResourceBudget::new(8, 0, 1000);
    assert!(matches!(
        binder(Some(&mut budget), None).constant_heading(&input, 1),
        Err(QueryError::Limit(error)) if error.resource == Resource::Bytes
    ));

    let bad_condition = Expr::Literal(Expression::GuidLiteral(
        "00000000-0000-0000-0000-000000000000".into(),
    ));
    let failure = Expr::Binary(
        BinaryOperator::Divide,
        Box::new(Expr::Literal(Expression::Integer(1))),
        Box::new(Expr::Literal(Expression::Integer(0))),
    );
    let input = Expr::ConditionalCall(
        tablecraft_expressions::functions::conditional::ConditionalFunction::Switch,
        vec![bad_condition, failure],
    );
    assert_eq!(
        binder(None, None).constant_heading(&input, 1),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    );
}

#[test]
fn every_conditional_checkpoint_can_cancel_without_source_access() {
    let input = Expr::Nz(
        Box::new(Expr::Literal(Expression::Null)),
        Some(Box::new(Expr::IIf(
            Box::new(Expr::Literal(Expression::Boolean(true))),
            Box::new(text("selected")),
            Box::new(text("unselected")),
        ))),
    );
    let calls = Cell::new(0);
    let run = |stop| {
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        binder(None, Some(&cancelled)).constant_heading(&input, 1)
    };
    assert!(run(usize::MAX).is_ok());
    let count = calls.get();
    for stop in 1..=count {
        calls.set(0);
        assert_eq!(run(stop), Err(QueryError::Cancelled), "checkpoint {stop}");
    }
}

#[path = "tests/negation.rs"]
mod negation;

#[path = "tests/text.rs"]
mod text_constants;
