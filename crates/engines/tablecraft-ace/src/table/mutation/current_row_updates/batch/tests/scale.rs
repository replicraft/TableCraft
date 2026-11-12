//! A complete all-row operation crosses the old assignment ceiling without a quota.
use super::{image, same, updates};
use crate::{FittedColumnUpdates, NativeImage};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::long_column_batch as raw;

#[test]
fn every_original_in_a_sixteen_thousand_row_table_is_updated_atomically() {
    let case = raw::all_rows_above_legacy_limit();
    assert_eq!(case.cells.len(), 16_385);
    assert!(case.cells.len() > NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS);
    let requests = updates(&case);
    let groups = [FittedColumnUpdates::Fixed {
        column: case.column,
        updates: &requests,
    }];
    // These are explicit caller work pools for a complete large operation;
    // the legacy writer's assignment count is not reused as a source limit.
    let mut work = ResourceBudget::new(4 * 1024 * 1024 * 1024, 5_000_000, 2_000_000_000);
    let plan = image(&case.before)
        .plan_retained_long_column_batch_updates(case.table, &groups, &mut work, &|| false)
        .unwrap();
    assert_eq!(plan.changed_rows(), 16_385);
    same(plan.before_bytes(), &case.before);
    same(plan.after_bytes(), &case.expected);
}
