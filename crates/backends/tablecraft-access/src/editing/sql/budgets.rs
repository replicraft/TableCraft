//! Query and native allowances compose through the real recoverable adapter.
use super::{RecoverableError, plan_recoverable_batch_update_with_budgets};
use std::io::Cursor;
use tablecraft_ace::NativeImage;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use tablecraft_query::{QueryError, UpdateError};
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::mutation_fixture;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(174),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn separate_query_and_native_allowances_retain_the_original_complete_plan() {
    let before = mutation_fixture::database();
    let query = parse_query(
        "UPDATE Items SET Score=77 WHERE Id=1",
        ParseLimits::default(),
    )
    .unwrap();
    let mut native = budget();
    let mut query_budget = budget();
    let reviewed = plan_recoverable_batch_update_with_budgets(
        image(&before),
        query.query(),
        &mut native,
        &mut query_budget,
        None,
        &|| false,
    )
    .unwrap();
    assert_eq!(reviewed.matched_rows(), 1);
    let plan = reviewed.into_backend();
    assert_eq!(plan.before_bytes(), before);
    assert_eq!(
        plan.after_bytes(),
        mutation_fixture::with_rows(&[(1, 77), (2, 20), (3, 30)])
    );
    assert_eq!(plan.changed_rows(), 1);
    assert!(native.remaining(Resource::Bytes) < budget().remaining(Resource::Bytes));
    assert!(query_budget.remaining(Resource::Rows) < budget().remaining(Resource::Rows));
}

#[test]
fn native_snapshot_and_query_failures_remain_distinct_without_cross_pool_grants() {
    let before = mutation_fixture::database();
    let query = parse_query(
        "UPDATE Items SET Score=77 WHERE Id=1",
        ParseLimits::default(),
    )
    .unwrap();
    let mut native = ResourceBudget::new(0, 0, 0);
    let mut query_budget = budget();
    let result = plan_recoverable_batch_update_with_budgets(
        image(&before),
        query.query(),
        &mut native,
        &mut query_budget,
        None,
        &|| false,
    );
    assert!(matches!(result, Err(RecoverableError::Editor(_))));
    assert_eq!(query_budget, budget());

    let mut native = budget();
    let mut query_budget = ResourceBudget::new(0, 0, 0);
    let result = plan_recoverable_batch_update_with_budgets(
        image(&before),
        query.query(),
        &mut native,
        &mut query_budget,
        None,
        &|| false,
    );
    assert!(matches!(
        result,
        Err(RecoverableError::Plan(UpdateError::Query(
            QueryError::Limit(_)
        )))
    ));
    assert_eq!(query_budget, ResourceBudget::new(0, 0, 0));
}
