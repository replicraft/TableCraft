//! Full public original-presence planning and proof, not internal cost mirroring.
use super::super::{POOLS, planning_inspection};
use super::{Case, Recipe, budget, cases, requests, scope};
use std::{array, cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, InspectError, NativeImage, RecoverableEditError, RecoverableNativeOmittedInsertPlan,
};
use tablecraft_core::limits::ResourceBudget;

fn plan(
    case: &Case,
    empty: bool,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RecoverableNativeOmittedInsertPlan, RecoverableEditError> {
    let original = if empty {
        vec![]
    } else {
        requests(Recipe::OmittedConstants)
    };
    let image = NativeImage::capture(&mut Cursor::new(&case.before), scope(), work, cancelled)
        .map_err(EditError::from)?;
    image.plan_recoverable_ordinary_omitted_row_inserts("Defaults", original, work, cancelled)
}

#[test]
#[ignore = "requires eight pinned default-presence images and independent SDK32 qualification"]
fn defaults_and_empty_admission_pay_exact_pools_and_cancel_before_publication() {
    let mut contexts = 0;
    for case in cases() {
        for empty in [false, true] {
            let expected = if empty {
                case.before.as_slice()
            } else {
                case.expected(Recipe::OmittedConstants)
            };
            let mut measured = budget();
            let initial = POOLS.map(|pool| measured.remaining(pool));
            let polls = Cell::new(0_usize);
            let complete = plan(&case, empty, &mut measured, &|| {
                polls.set(polls.get() + 1);
                false
            })
            .unwrap();
            assert_eq!(complete.before_bytes(), case.before);
            assert_eq!(complete.after_bytes(), expected);
            assert_eq!(complete.inserted_rows(), usize::from(!empty));
            let costs: [u64; 3] =
                array::from_fn(|index| initial[index] - measured.remaining(POOLS[index]));
            assert!(costs.iter().all(|cost| *cost > 0));
            assert!(polls.get() > 2);
            drop(complete);

            let mut exact = ResourceBudget::new(costs[0], costs[1], costs[2]);
            let complete = plan(&case, empty, &mut exact, &|| false).unwrap();
            assert_eq!(complete.after_bytes(), expected);
            assert_eq!(POOLS.map(|pool| exact.remaining(pool)), [0, 0, 0]);
            drop(complete);
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
                .unwrap();
                assert!(
                    matches!(planning_inspection(error), InspectError::Limit(error) if error.resource == POOLS[index])
                );
            }
            for stop in [1, polls.get().div_ceil(2), polls.get()] {
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
