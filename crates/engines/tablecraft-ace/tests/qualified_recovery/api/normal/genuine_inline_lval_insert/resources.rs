//! Measure the complete public boundary, not implementation-specific work counts.
use super::super::{POOLS, planning_inspection};
use super::{budget, cases, requests, scope};
use std::{array, cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, InspectError, NativeImage, RecoverableEditError, RecoverableNativeInsertPlan,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_testkit::access::genuine_inline_lval_insert::Case;

fn plan(
    case: &Case,
    empty: bool,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RecoverableNativeInsertPlan, RecoverableEditError> {
    let original = if empty {
        Vec::new()
    } else {
        requests(case.recipe)
    };
    let image = NativeImage::capture(&mut Cursor::new(&case.before), scope(), work, cancelled)
        .map_err(EditError::from)?;
    image.plan_recoverable_ordinary_row_inserts("InlineValues", original, work, cancelled)
}

#[test]
#[ignore = "requires eighteen native/SDK-qualified inline LVAL views"]
fn inline_insert_and_empty_pay_exact_pools_and_poll_entry_middle_and_last() {
    for case in cases()
        .into_iter()
        .filter(|case| case.recipe == "compressed-limit")
    {
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
            assert!(costs.iter().all(|value| *value > 0));
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
                let mut lower = costs;
                lower[index] -= 1;
                let error = plan(
                    &case,
                    empty,
                    &mut ResourceBudget::new(lower[0], lower[1], lower[2]),
                    &|| false,
                )
                .err()
                .unwrap();
                assert!(
                    matches!(planning_inspection(error), InspectError::Limit(error) if error.resource == POOLS[index])
                );
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
        }
    }
}
