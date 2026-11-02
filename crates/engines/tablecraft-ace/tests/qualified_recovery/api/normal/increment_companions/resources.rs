//! Complete native capture/original intent/Source/resolver/fresh proof measurement.
use super::super::{POOLS, planning_inspection};
use super::{budget, cases, fixtures, requests, scope};
use std::{array, cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, InspectError, NativeImage, RecoverableEditError, RecoverableNativeOmittedInsertPlan,
};
use tablecraft_core::limits::ResourceBudget;

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
#[ignore = "requires companion original20/AFTER20 native and SDK qualification before matching generation"]
fn full_increment_and_empty_plans_have_exact_pools_and_cancellation() {
    let cases = cases();
    let mut contexts = 0;
    for case in &cases {
        for empty in [false, true] {
            let expected = if empty {
                case.before.as_slice()
            } else {
                case.after.as_slice()
            };
            let mut measured = budget();
            let initial = POOLS.map(|pool| measured.remaining(pool));
            let polls = Cell::new(0_usize);
            let complete = plan(case, empty, &mut measured, &|| {
                polls.set(polls.get() + 1);
                false
            })
            .unwrap();
            assert_eq!(complete.before_bytes(), case.before.as_slice());
            assert_eq!(complete.after_bytes(), expected);
            assert_eq!(complete.inserted_rows(), usize::from(!empty));
            let costs: [u64; 3] =
                array::from_fn(|index| initial[index] - measured.remaining(POOLS[index]));
            assert!(costs.iter().all(|cost| *cost > 0));
            drop(complete);
            let mut exact = ResourceBudget::new(costs[0], costs[1], costs[2]);
            assert_eq!(
                plan(case, empty, &mut exact, &|| false)
                    .unwrap()
                    .after_bytes(),
                expected
            );
            assert_eq!(POOLS.map(|pool| exact.remaining(pool)), [0, 0, 0]);
            for index in 0..3 {
                let mut short = costs;
                short[index] -= 1;
                let error = plan(
                    case,
                    empty,
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
                let error = plan(case, empty, &mut budget(), &|| {
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
    }
    assert_eq!(contexts, 40);
}
