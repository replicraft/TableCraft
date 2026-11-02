//! Full public fitting admission, encoding and independent proof resource boundary.
use super::super::planning_inspection;
use super::{Case, budget, cases, intent_groups, scope};
use std::io::Cursor;
use tablecraft_ace::{EditError, NativeImage, RecoverableEditError, RecoverableNativeEditPlan};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_testkit::support::ace::resources::assert_public_boundary;

fn plan(
    case: &Case,
    reverse: bool,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
    let requests = intent_groups(case, reverse);
    let image = NativeImage::capture(&mut Cursor::new(&case.before), scope(), work, cancelled)
        .map_err(EditError::from)?;
    image.plan_recoverable_ordinary_fitted_updates("Items", requests, work, cancelled)
}

#[test]
#[ignore = "requires independently qualified external mixed20 source"]
fn external_mixed_has_exact_public_pools_and_cancellation() {
    let mut contexts = 0;
    for case in cases() {
        for reverse in [false, true] {
            assert_public_boundary(
                budget,
                |work, cancelled| {
                    plan(&case, reverse, work, cancelled).map_err(planning_inspection)
                },
                |complete| {
                    assert_eq!(complete.after_bytes(), case.after);
                    assert_eq!(complete.changed_rows(), super::fixture::changed_rows(&case));
                },
                |complete| assert_eq!(complete.after_bytes(), case.after),
            );
            contexts += 1;
        }
    }
    assert_eq!(contexts, 32);
}
