use super::support::*;
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::EvalError;
#[test]
fn callback_cancellation_and_cumulative_provider_limits_do_not_publish_results() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    let parsed = parse_query("SELECT A='a' FROM T WHERE Id=1", ParseLimits::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    resolver.cancel_on_resolve.set(true);
    let callback = || resolver.cancelled.get();
    assert!(matches!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                collation_resolver: Some(&resolver),
                cancelled: Some(&callback),
                ..Default::default()
            }
        ),
        Err(QueryError::Cancelled)
    ));
    resolver.cancel_on_resolve.set(false);
    resolver.cancelled.set(false);
    *resolver.budget.borrow_mut() = ResourceBudget::new(0, 0, 2);
    for _ in 0..2 {
        assert!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    collation_resolver: Some(&resolver),
                    ..Default::default()
                }
            )
            .is_ok()
        );
    }
    assert!(matches!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                collation_resolver: Some(&resolver),
                ..Default::default()
            }
        ),
        Err(QueryError::Limit(_))
    ));
}
#[test]
fn every_observed_query_checkpoint_can_cancel_and_runtime_step_limits_apply() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    let parsed = parse_query(
        "SELECT A FROM T WHERE A IN ('a','b') ORDER BY A",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let visits = Cell::new(0usize);
    let stop = Cell::new(None);
    let cancelled = || {
        let n = visits.get() + 1;
        visits.set(n);
        stop.get() == Some(n)
    };
    let options = || ExecutionOptions {
        collation: Some(&DEFAULT),
        collation_resolver: Some(&resolver),
        cancelled: Some(&cancelled),
        ..Default::default()
    };
    plan.execute(&source, &[], options()).unwrap();
    let count = visits.get();
    assert!(count > 1);
    for point in 1..=count {
        visits.set(0);
        stop.set(Some(point));
        *resolver.budget.borrow_mut() = ResourceBudget::new(0, 0, 10000);
        let result = plan.execute(&source, &[], options());
        assert!(
            matches!(
                result,
                Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "checkpoint {point}"
        );
    }
    assert!(matches!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&DEFAULT),
                collation_resolver: Some(&resolver),
                limits: tablecraft_query::QueryLimits {
                    steps: 8,
                    ..Default::default()
                },
                ..Default::default()
            }
        ),
        Err(QueryError::Limit(_))
    ));
}
