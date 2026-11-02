//! Full public complete INSERT cost/cancellation, including explicit policy reads.
use super::super::{POOLS, planning_inspection};
use super::{budget, cases, requests, scope};
use std::{array, cell::Cell, io::Cursor};
use tablecraft_ace::{EditError, InspectError, NativeImage, RecoverableEditError};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_testkit::access::genuine_text_azl::Case;

fn plan(
    case: &Case,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), RecoverableEditError> {
    let image = NativeImage::capture(&mut Cursor::new(&case.before), scope(), work, cancelled)
        .map_err(EditError::from)?;
    let result = image.plan_recoverable_ordinary_complete_row_inserts(
        case.table,
        requests(case),
        work,
        cancelled,
    )?;
    assert_eq!(result.after_bytes(), case.after_bytes());
    Ok(())
}

#[test]
#[ignore = "requires14 independently qualified explicit-AZL source views"]
fn explicit_property_and_empty_request_inserts_pay_exact_pools_and_cancel_before_effects() {
    let mut contexts = 0;
    for case in cases().into_iter().filter(|case| case.after.is_some()) {
        let mut measured = budget();
        let initial = POOLS.map(|pool| measured.remaining(pool));
        let polls = Cell::new(0_usize);
        plan(&case, &mut measured, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
        let costs: [u64; 3] =
            array::from_fn(|index| initial[index] - measured.remaining(POOLS[index]));
        assert!(costs.iter().all(|cost| *cost > 0));
        let terminal = polls.get();
        assert!(terminal > 2);
        let mut exact = ResourceBudget::new(costs[0], costs[1], costs[2]);
        plan(&case, &mut exact, &|| false).unwrap();
        assert_eq!(POOLS.map(|pool| exact.remaining(pool)), [0, 0, 0]);
        for index in 0..3 {
            let mut less = costs;
            less[index] -= 1;
            let error = plan(
                &case,
                &mut ResourceBudget::new(less[0], less[1], less[2]),
                &|| false,
            )
            .unwrap_err();
            assert!(
                matches!(planning_inspection(error), InspectError::Limit(error) if error.resource == POOLS[index])
            );
        }
        for stop in [1, terminal.div_ceil(2), terminal] {
            let seen = Cell::new(0_usize);
            let error = plan(&case, &mut budget(), &|| {
                seen.set(seen.get() + 1);
                seen.get() >= stop
            })
            .unwrap_err();
            assert_eq!(planning_inspection(error), InspectError::Cancelled);
            assert_eq!(seen.get(), stop);
        }
        contexts += 1;
    }
    assert_eq!(contexts, 20);
}
