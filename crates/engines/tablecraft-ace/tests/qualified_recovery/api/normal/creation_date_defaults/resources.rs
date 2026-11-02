//! Complete native capture/original intent/Source/resolver/fresh proof measurement.
use super::super::{POOLS, planning_inspection};
use super::{budget, fixtures, requests, scope, sources};
use std::{array, cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, InspectError, NativeImage, RecoverableEditError, RecoverableNativeOmittedInsertPlan,
};
use tablecraft_core::limits::ResourceBudget;

fn plan(
    before: &[u8],
    empty: bool,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RecoverableNativeOmittedInsertPlan, RecoverableEditError> {
    let rows = if empty {
        Vec::new()
    } else {
        requests("omitted")
    };
    let image = NativeImage::capture(&mut Cursor::new(before), scope(), work, cancelled)
        .map_err(EditError::from)?;
    image.plan_recoverable_ordinary_omitted_row_inserts("Items", rows, work, cancelled)
}

#[test]
#[ignore = "requires source36 native parser/bit/Source gates before matching producers"]
fn full_date_default_and_empty_plans_have_exact_pools_and_cancellation() {
    let sources = sources();
    let mut contexts = 0;
    for header in [5, 6] {
        for case in fixtures::CASES {
            let before = sources.bytes(header, case, "before");
            for empty in [false, true] {
                let expected =
                    sources.bytes(header, case, if empty { "before" } else { "omitted" });
                let mut measured = budget();
                let initial = POOLS.map(|pool| measured.remaining(pool));
                let polls = Cell::new(0_usize);
                let complete = plan(&before, empty, &mut measured, &|| {
                    polls.set(polls.get() + 1);
                    false
                })
                .unwrap();
                assert_eq!(complete.before_bytes(), before);
                assert_eq!(complete.after_bytes(), expected);
                assert_eq!(complete.inserted_rows(), usize::from(!empty));
                let costs: [u64; 3] =
                    array::from_fn(|index| initial[index] - measured.remaining(POOLS[index]));
                assert!(costs.iter().all(|cost| *cost > 0));
                drop(complete);
                let mut exact = ResourceBudget::new(costs[0], costs[1], costs[2]);
                assert_eq!(
                    plan(&before, empty, &mut exact, &|| false)
                        .unwrap()
                        .after_bytes(),
                    expected
                );
                assert_eq!(POOLS.map(|pool| exact.remaining(pool)), [0, 0, 0]);
                for index in 0..3 {
                    let mut short = costs;
                    short[index] -= 1;
                    let error = plan(
                        &before,
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
                    let error = plan(&before, empty, &mut budget(), &|| {
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
    }
    assert_eq!(contexts, 24);
}
