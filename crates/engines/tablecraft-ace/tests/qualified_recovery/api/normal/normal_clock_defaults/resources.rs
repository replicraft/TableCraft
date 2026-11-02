//! Complete public capture/intent/default/provider/stage/fresh proof controls.
use super::super::{POOLS, planning_inspection};
use super::{OperationClock, budget, fixtures, requests, scope, sources};
use std::{array, cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, InspectError, NativeImage, RecoverableEditError, RecoverableNativeClockInsertPlan,
};
use tablecraft_core::limits::ResourceBudget;

fn plan(
    case: &fixtures::Case<'_>,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RecoverableNativeClockInsertPlan, RecoverableEditError> {
    let image = NativeImage::capture(&mut Cursor::new(case.before), scope(), work, cancelled)
        .map_err(EditError::from)?;
    image.plan_recoverable_ordinary_clock_row_inserts(
        "Items",
        requests(case),
        &OperationClock::new(case),
        work,
        cancelled,
    )
}

#[test]
#[ignore = "requires source18 before complete public clock resource and cancellation checks"]
fn full_clock_and_no_read_plans_pay_exact_pools_and_poll_all_cancellation_boundaries() {
    let sources = sources();
    let mut contexts = 0;
    for case in sources.cases() {
        let mut measured = budget();
        let initial = POOLS.map(|resource| measured.remaining(resource));
        let polls = Cell::new(0_usize);
        assert_eq!(
            plan(&case, &mut measured, &|| {
                polls.set(polls.get() + 1);
                false
            })
            .unwrap()
            .after_bytes(),
            case.after
        );
        let costs: [u64; 3] = array::from_fn(|at| initial[at] - measured.remaining(POOLS[at]));
        assert!(costs.iter().all(|cost| *cost > 0));
        let mut exact = ResourceBudget::new(costs[0], costs[1], costs[2]);
        assert_eq!(
            plan(&case, &mut exact, &|| false).unwrap().after_bytes(),
            case.after
        );
        assert_eq!(POOLS.map(|resource| exact.remaining(resource)), [0, 0, 0]);
        for at in 0..3 {
            let mut short = costs;
            short[at] -= 1;
            let error = plan(
                &case,
                &mut ResourceBudget::new(short[0], short[1], short[2]),
                &|| false,
            )
            .err()
            .unwrap();
            assert!(
                matches!(planning_inspection(error), InspectError::Limit(error) if error.resource == POOLS[at])
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
            assert!(matches!(
                planning_inspection(error),
                InspectError::Cancelled
            ));
            assert_eq!(seen.get(), stop);
        }
        contexts += 1;
    }
    assert_eq!(contexts, 18);
}
