//! Public original-capture through independent-proof resource/cancellation bounds.
use super::super::{POOLS, planning_inspection};
use super::{budget, cases, requests, scope};
use std::{array, cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, InspectError, NativeImage, RecoverableEditError, RecoverableNativeCompleteInsertPlan,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_testkit::access::genuine_binary_indexed::Case;

fn plan(
    case: &Case,
    empty: bool,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RecoverableNativeCompleteInsertPlan, RecoverableEditError> {
    let rows = if empty { Vec::new() } else { requests(case) };
    let image = NativeImage::capture(&mut Cursor::new(&case.before), scope(), work, cancelled)
        .map_err(EditError::from)?;
    image.plan_recoverable_ordinary_complete_row_inserts("BinaryKeys", rows, work, cancelled)
}

#[test]
#[ignore = "requires eighteen native/SDK-qualified Binary source views"]
fn binary_batches_and_empty_requests_pay_exact_pools_and_cancel_before_publication() {
    let mut contexts = 0;
    for case in cases().into_iter().filter(|case| case.recipe == "batch") {
        for empty in [false, true] {
            let expected = if empty { &case.before } else { &case.after };
            let mut measured = budget();
            let initial = POOLS.map(|pool| measured.remaining(pool));
            let polls = Cell::new(0_usize);
            let result = plan(&case, empty, &mut measured, &|| {
                polls.set(polls.get() + 1);
                false
            })
            .unwrap();
            assert_eq!(result.after_bytes(), expected);
            let costs: [u64; 3] =
                array::from_fn(|index| initial[index] - measured.remaining(POOLS[index]));
            assert!(costs.iter().all(|cost| *cost > 0));
            let terminal = polls.get();
            assert!(terminal > 2);
            let mut exact = ResourceBudget::new(costs[0], costs[1], costs[2]);
            assert_eq!(
                plan(&case, empty, &mut exact, &|| false)
                    .unwrap()
                    .after_bytes(),
                expected
            );
            assert_eq!(POOLS.map(|pool| exact.remaining(pool)), [0, 0, 0]);
            for index in 0..3 {
                let mut less = costs;
                less[index] -= 1;
                let error = plan(
                    &case,
                    empty,
                    &mut ResourceBudget::new(less[0], less[1], less[2]),
                    &|| false,
                )
                .err()
                .unwrap();
                assert!(matches!(
                    planning_inspection(error),
                    InspectError::Limit(error) if error.resource == POOLS[index],
                ));
            }
            for stop in [1, terminal.div_ceil(2), terminal] {
                let seen = Cell::new(0_usize);
                let error = plan(&case, empty, &mut budget(), &|| {
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
    assert_eq!(contexts, 4);
}
