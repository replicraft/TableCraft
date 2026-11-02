//! Full public fitting admission, encoding and independent proof resource boundary.
use super::super::{POOLS, planning_inspection};
use super::{Case, budget, cases, groups, scope};
use std::{array, cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, InspectError, NativeImage, RecoverableEditError, RecoverableNativeEditPlan,
};
use tablecraft_core::limits::ResourceBudget;

fn plan(
    case: &Case,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
    let requests = groups(case);
    let image = NativeImage::capture(&mut Cursor::new(&case.before), scope(), work, cancelled)
        .map_err(EditError::from)?;
    image.plan_recoverable_ordinary_fitted_updates("Items", requests, work, cancelled)
}

#[test]
#[ignore = "requires external UPDATE32 source admission before matching producers"]
fn external_retained_capacity_and_empty_groups_have_exact_pools_and_cancellation() {
    let mut contexts = 0;
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.recipe, "joint-chain4077-4078" | "empty-group"))
    {
        let mut measured = budget();
        let initial = POOLS.map(|pool| measured.remaining(pool));
        let polls = Cell::new(0_usize);
        let complete = plan(&case, &mut measured, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
        assert_eq!(complete.after_bytes(), case.after);
        assert_eq!(complete.changed_rows(), case.changed);
        let costs: [u64; 3] =
            array::from_fn(|index| initial[index] - measured.remaining(POOLS[index]));
        assert!(costs.iter().all(|cost| *cost > 0));
        drop(complete);
        let mut exact = ResourceBudget::new(costs[0], costs[1], costs[2]);
        assert_eq!(
            plan(&case, &mut exact, &|| false).unwrap().after_bytes(),
            case.after
        );
        assert_eq!(POOLS.map(|pool| exact.remaining(pool)), [0, 0, 0]);
        for index in 0..3 {
            let mut short = costs;
            short[index] -= 1;
            let error = plan(
                &case,
                &mut ResourceBudget::new(short[0], short[1], short[2]),
                &|| false,
            )
            .err()
            .unwrap();
            assert!(
                matches!(planning_inspection(error), InspectError::Limit(failure) if failure.resource == POOLS[index])
            );
        }
        let terminal = polls.get();
        assert!(terminal > 2);
        for stop in [1, terminal.div_ceil(2), terminal] {
            let seen = Cell::new(0_usize);
            let error = plan(&case, &mut budget(), &|| {
                seen.set(seen.get() + 1);
                seen.get() >= stop
            })
            .err()
            .unwrap();
            assert_eq!(planning_inspection(error), InspectError::Cancelled);
            assert_eq!(seen.get(), stop);
        }
        contexts += 1;
    }
    assert_eq!(contexts, 8);
}
