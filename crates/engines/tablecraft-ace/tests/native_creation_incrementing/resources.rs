//! Measure each full public creation boundary rather than internal helper costs.
use super::{budget, sources};
use std::{array, cell::Cell};
use tablecraft_ace::{CreateDatabaseError, CreatedDatabase, InspectError};
use tablecraft_core::limits::{Resource, ResourceBudget};

const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];

#[test]
#[ignore = "requires source14 before matching Increment creator resource checks"]
fn both_public_creators_pay_exact_pools_and_poll_entry_middle_terminal_cancellation() {
    let sources = sources();
    let expected = sources.bytes(5, "increment-zero-before");
    for multiple in [false, true] {
        assert_resources(expected, |work, cancelled| {
            super::invoke(multiple, false, work, cancelled)
        });
    }
}

pub(super) fn assert_resources(
    expected: &[u8],
    invoke: impl Fn(
        &mut ResourceBudget,
        &dyn Fn() -> bool,
    ) -> Result<CreatedDatabase, CreateDatabaseError>,
) {
    let mut measured = budget();
    let initial = POOLS.map(|pool| measured.remaining(pool));
    let polls = Cell::new(0_usize);
    assert_eq!(
        invoke(&mut measured, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap()
        .as_bytes(),
        expected
    );
    let costs: [u64; 3] = array::from_fn(|at| initial[at] - measured.remaining(POOLS[at]));
    assert!(costs.iter().all(|cost| *cost > 0));
    let mut exact = ResourceBudget::new(costs[0], costs[1], costs[2]);
    assert_eq!(invoke(&mut exact, &|| false).unwrap().as_bytes(), expected);
    assert_eq!(POOLS.map(|pool| exact.remaining(pool)), [0, 0, 0]);
    for at in 0..3 {
        let mut short = costs;
        short[at] -= 1;
        let result = invoke(
            &mut ResourceBudget::new(short[0], short[1], short[2]),
            &|| false,
        );
        assert!(matches!(
            result,
            Err(CreateDatabaseError::Inspect(InspectError::Limit(error)))
                if error.resource == POOLS[at]
        ));
    }
    let terminal = polls.get();
    assert!(terminal > 2);
    for stop in [1, terminal.div_ceil(2), terminal] {
        let seen = Cell::new(0_usize);
        assert!(matches!(
            invoke(&mut budget(), &|| {
                seen.set(seen.get() + 1);
                seen.get() >= stop
            }),
            Err(CreateDatabaseError::Inspect(InspectError::Cancelled))
        ));
        assert_eq!(seen.get(), stop);
    }
}
