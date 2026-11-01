//! Full recoverable DELETE planning pays exact resources and cancellation.
use super::{budget, fixtures, plan};
use std::{array, cell::Cell};
use tablecraft_ace::{EditError, InspectError, NativeIntentError, RecoverableEditError};
use tablecraft_core::limits::{Resource, ResourceBudget};

const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];

fn inspection(error: RecoverableEditError) -> InspectError {
    match error {
        RecoverableEditError::Edit(EditError::Inspect(error))
        | RecoverableEditError::Intent(NativeIntentError::Inspect(error))
        | RecoverableEditError::Intent(NativeIntentError::Request(EditError::Inspect(error))) => {
            error
        }
        error => panic!("expected resource or cancellation failure: {error:?}"),
    }
}

#[test]
#[ignore = "requires all four independently qualified ordinary DELETE images"]
fn complete_delete_plan_exact_and_one_less_pools_and_entry_middle_terminal_cancellation() {
    for case in fixtures::cases() {
        let mut measured = budget();
        let initial = POOLS.map(|pool| measured.remaining(pool));
        assert_eq!(
            plan(&case, &mut measured, &|| false).unwrap().after_bytes(),
            case.after
        );
        let costs: [u64; 3] =
            array::from_fn(|index| initial[index] - measured.remaining(POOLS[index]));
        assert!(costs.iter().all(|cost| *cost > 0));
        let mut exact = ResourceBudget::new(costs[0], costs[1], costs[2]);
        assert_eq!(
            plan(&case, &mut exact, &|| false).unwrap().after_bytes(),
            case.after
        );
        assert_eq!(POOLS.map(|pool| exact.remaining(pool)), [0, 0, 0]);
        for index in 0..3 {
            let mut below = costs;
            below[index] -= 1;
            let error = plan(
                &case,
                &mut ResourceBudget::new(below[0], below[1], below[2]),
                &|| false,
            )
            .err()
            .expect("one-less allowance must not publish a plan");
            assert!(
                matches!(inspection(error), InspectError::Limit(error) if error.resource == POOLS[index])
            );
        }
        let polls = Cell::new(0_usize);
        plan(&case, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
        for stop in [1, polls.get().div_ceil(2), polls.get()] {
            let seen = Cell::new(0_usize);
            let error = plan(&case, &mut budget(), &|| {
                seen.set(seen.get() + 1);
                seen.get() >= stop
            })
            .err()
            .expect("cancellation must not publish a plan");
            assert_eq!(inspection(error), InspectError::Cancelled);
        }
    }
}
