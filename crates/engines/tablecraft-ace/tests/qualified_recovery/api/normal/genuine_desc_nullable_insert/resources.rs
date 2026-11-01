//! Full optional/directional indexed INSERT, including empty requests, is bounded.
use super::super::{POOLS, planning_inspection};
use super::{budget, fixtures, scope};
use std::{array, cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, InspectError, NativeImage, RecoverableEditError, RecoverableNativeInsertPlan,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_testkit::access::genuine_desc_nullable_insert::Case;

/// The measured public boundary includes image capture, original-intent binding,
/// complete Source/index admission, staging, fresh proof and plan publication.
/// Literal request construction is test input preparation, not native work.
fn plan(
    case: &Case,
    empty: bool,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RecoverableNativeInsertPlan, RecoverableEditError> {
    let requests = if empty {
        Vec::new()
    } else {
        fixtures::requests(2)
    };
    let image = NativeImage::capture(&mut Cursor::new(&case.before), scope(), work, cancelled)
        .map_err(EditError::from)?;
    image.plan_recoverable_ordinary_row_inserts("Items", requests, work, cancelled)
}

#[test]
#[ignore = "requires source native6 and SDK36 nullable/DESC INSERT qualification"]
fn full_nullable_desc_insert_and_empty_requests_pay_exact_pools_and_cancel_before_publication() {
    let cases = fixtures::cases();
    let mut contexts = 0;
    for case in cases {
        for empty in [false, true] {
            let expected = if empty { &case.before } else { &case.batch };
            let mut measured = budget();
            let initial = POOLS.map(|pool| measured.remaining(pool));
            let polls = Cell::new(0_usize);
            let successful = plan(&case, empty, &mut measured, &|| {
                polls.set(polls.get() + 1);
                false
            })
            .unwrap();
            assert_eq!(successful.before_bytes(), case.before);
            assert_eq!(successful.after_bytes(), expected);
            assert_eq!(successful.inserted_rows(), if empty { 0 } else { 2 });
            assert_eq!(successful.intent().rows().len(), if empty { 0 } else { 2 });
            let costs: [u64; 3] =
                array::from_fn(|index| initial[index] - measured.remaining(POOLS[index]));
            assert!(costs.iter().all(|cost| *cost > 0));
            let terminal = polls.get();
            assert!(terminal > 2);
            drop(successful);

            let mut exact = ResourceBudget::new(costs[0], costs[1], costs[2]);
            let successful = plan(&case, empty, &mut exact, &|| false).unwrap();
            assert_eq!(successful.before_bytes(), case.before);
            assert_eq!(successful.after_bytes(), expected);
            assert_eq!(successful.inserted_rows(), if empty { 0 } else { 2 });
            assert_eq!(POOLS.map(|pool| exact.remaining(pool)), [0, 0, 0]);
            drop(successful);

            for index in 0..POOLS.len() {
                let mut allowance = costs;
                allowance[index] -= 1;
                let error = plan(
                    &case,
                    empty,
                    &mut ResourceBudget::new(allowance[0], allowance[1], allowance[2]),
                    &|| false,
                )
                .err()
                .expect("one-less native allowance must not publish a recoverable plan");
                assert!(matches!(
                    planning_inspection(error),
                    InspectError::Limit(error) if error.resource == POOLS[index]
                ));
            }

            // Observe the real complete successful call, rather than inventing
            // a stage-specific threshold or duplicating implementation costs.
            for stop in [1, terminal.div_ceil(2), terminal] {
                let seen = Cell::new(0_usize);
                let error = plan(&case, empty, &mut budget(), &|| {
                    seen.set(seen.get() + 1);
                    seen.get() >= stop
                })
                .err()
                .expect("entry/middle/final cancellation must precede plan publication");
                assert_eq!(planning_inspection(error), InspectError::Cancelled);
                assert_eq!(seen.get(), stop);
            }
            contexts += 1;
        }
    }
    assert_eq!(contexts, 4);
}
