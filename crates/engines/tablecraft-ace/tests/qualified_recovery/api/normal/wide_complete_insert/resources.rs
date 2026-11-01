//! Full public complete scalar INSERT planning/proof bounds, including empty admission.
use super::super::{POOLS, planning_inspection};
use super::{budget, fixtures, scope};
use fixtures::View;
use std::{array, cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, InspectError, NativeImage, RecoverableEditError, RecoverableNativeCompleteInsertPlan,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_testkit::access::canonical_ordinary_wide_insert::Case;

// The public boundary includes image capture, original TCNA intent binding,
// complete Source admission, DATA placement/staging, fresh proof and final
// recoverable plan publication. Literal request construction is test input.
fn plan(
    case: &View,
    empty: bool,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RecoverableNativeCompleteInsertPlan, RecoverableEditError> {
    let requests = if empty {
        vec![]
    } else {
        fixtures::requests(case)
    };
    let image = NativeImage::capture(
        &mut Cursor::new(&case.source.before),
        scope(),
        work,
        cancelled,
    )
    .map_err(EditError::from)?;
    image.plan_recoverable_ordinary_complete_row_inserts("Items", requests, work, cancelled)
}

#[test]
fn full_tcna4_scalar_insert_and_empty_requests_pay_exact_pools_and_cancel_before_publication() {
    let cases = fixtures::cases().into_iter().filter(|case| {
        matches!(
            case.case,
            Case::FixedExact | Case::DecimalExact | Case::EofFixed
        )
    });
    let mut contexts = 0;
    for case in cases {
        for empty in [false, true] {
            let expected = if empty {
                &case.source.before
            } else {
                &case.source.after
            };
            let count = if empty { 0 } else { 1 };
            let mut measured = budget();
            let initial = POOLS.map(|pool| measured.remaining(pool));
            let polls = Cell::new(0_usize);
            let successful = plan(&case, empty, &mut measured, &|| {
                polls.set(polls.get() + 1);
                false
            })
            .unwrap();
            assert_eq!(successful.before_bytes(), case.source.before);
            assert_eq!(successful.after_bytes(), expected);
            assert_eq!(successful.inserted_rows(), count);
            assert_eq!(successful.intent().rows().len(), count);
            let costs: [u64; 3] =
                array::from_fn(|index| initial[index] - measured.remaining(POOLS[index]));
            assert!(costs.iter().all(|cost| *cost > 0));
            let terminal = polls.get();
            assert!(terminal > 2);
            drop(successful);

            let mut exact = ResourceBudget::new(costs[0], costs[1], costs[2]);
            let successful = plan(&case, empty, &mut exact, &|| false).unwrap();
            assert_eq!(successful.before_bytes(), case.source.before);
            assert_eq!(successful.after_bytes(), expected);
            assert_eq!(successful.inserted_rows(), count);
            assert_eq!(successful.intent().rows().len(), count);
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
                .expect("one-less native allowance must not publish an INSERT plan");
                assert!(matches!(
                    planning_inspection(error),
                    InspectError::Limit(error) if error.resource == POOLS[index]
                ));
            }

            // Derive thresholds from the complete successful public call, not
            // assumed phase costs, internal operations or a synthetic counter.
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
    assert_eq!(contexts, 12);
}
