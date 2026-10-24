use super::support::*;
use std::{cell::Cell, cmp::Ordering};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::{EvalError, comparison::Collation, predicates::LikeMode};

#[test]
fn cancellation_inside_comparison_callbacks_aborts_all_result_consumers() {
    struct Cancelling(Cell<bool>);

    impl Collation for Cancelling {
        fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
            self.0.set(true);
            Ok(Ordering::Equal)
        }
    }

    let source = Fixture::new();
    for sql in [
        "SELECT A='a' FROM T WHERE Id=1",
        "SELECT StrComp(A,'a',2) FROM T WHERE Id=1",
        "SELECT A FROM T ORDER BY A",
        "SELECT DISTINCT A FROM T",
        "SELECT A,Count(*) FROM T GROUP BY A",
        "SELECT A LIKE 'a' FROM T WHERE Id=1",
    ] {
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        let collation = Cancelling(Cell::new(false));
        let cancelled = || collation.0.get();
        assert!(
            matches!(
                plan.execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        collation: Some(&collation),
                        cancelled: Some(&cancelled),
                        like_mode: Some(LikeMode::Ansi89),
                        ..Default::default()
                    }
                ),
                Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "{sql}"
        );
        assert!(collation.0.get(), "provider was not reached: {sql}");
    }
}

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
