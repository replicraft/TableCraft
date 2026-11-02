//! Complete native capture/original intent/Source/resolver/fresh proof measurement.
use super::super::planning_inspection;
use super::{budget, cases, fixtures, requests, scope};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, NativeImage, RecoverableEditError, RecoverableNativeOmittedInsertPlan,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_testkit::support::ace::resources::assert_public_boundary;

fn plan(
    case: &fixtures::Case,
    empty: bool,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RecoverableNativeOmittedInsertPlan, RecoverableEditError> {
    let rows = if empty {
        Vec::new()
    } else {
        requests(case, false)
    };
    let image = NativeImage::capture(&mut Cursor::new(&case.before), scope(), work, cancelled)
        .map_err(EditError::from)?;
    image.plan_recoverable_ordinary_omitted_row_inserts(case.table, rows, work, cancelled)
}

#[test]
#[ignore = "requires multi-index original2/AFTER2 native and SDK qualification before matching generation"]
fn full_multi_index_increment_and_empty_plans_have_exact_pools_and_cancellation() {
    let cases = cases();
    let mut contexts = 0;
    for case in &cases {
        for empty in [false, true] {
            let expected = if empty {
                case.before.as_slice()
            } else {
                case.after.as_slice()
            };
            assert_public_boundary(
                budget,
                |work, cancelled| plan(case, empty, work, cancelled).map_err(planning_inspection),
                |complete| {
                    assert_eq!(complete.before_bytes(), case.before.as_slice());
                    assert_eq!(complete.after_bytes(), expected);
                    assert_eq!(complete.inserted_rows(), if empty { 0 } else { 2 });
                },
                |complete| assert_eq!(complete.after_bytes(), expected),
            );
            contexts += 1;
        }
    }
    assert_eq!(contexts, 4);
}
