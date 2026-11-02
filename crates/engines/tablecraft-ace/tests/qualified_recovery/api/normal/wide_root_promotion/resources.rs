//! Full public capture/intent/Source/physical/fresh-proof resource boundary.
use super::super::planning_inspection;
use super::{budget, cases, requests, scope};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, NativeImage, RecoverableEditError, RecoverableNativeCompleteInsertPlan,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_testkit::fixtures::access::genuine_wide_root_promotion::Case;
use tablecraft_testkit::support::ace::resources::assert_public_boundary;

fn plan(
    case: &Case,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RecoverableNativeCompleteInsertPlan, RecoverableEditError> {
    let image = NativeImage::capture(&mut Cursor::new(&case.before), scope(), work, cancelled)
        .map_err(EditError::from)?;
    image.plan_recoverable_ordinary_complete_row_inserts(
        case.table,
        requests(case),
        work,
        cancelled,
    )
}

#[test]
#[ignore = "requires six read-qualified coordinated wider promotion source files"]
fn wide_promotions_full_public_pools_exact_one_less_and_entry_middle_terminal_cancel() {
    let cases = cases();
    let mut contexts = 0;
    for case in cases {
        assert_public_boundary(
            budget,
            |work, cancelled| plan(&case, work, cancelled).map_err(planning_inspection),
            |planned| {
                assert_eq!(planned.before_bytes(), case.before);
                assert_eq!(planned.after_bytes(), case.after);
                assert_eq!(planned.inserted_rows(), case.inserted);
            },
            |planned| {
                assert_eq!(planned.before_bytes(), case.before);
                assert_eq!(planned.after_bytes(), case.after);
                assert_eq!(planned.inserted_rows(), case.inserted);
            },
        );
        contexts += 1;
    }
    assert_eq!(contexts, 8);
}
