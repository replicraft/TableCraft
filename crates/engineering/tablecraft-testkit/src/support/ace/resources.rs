//! Measure actual public plans, then verify exact limits and cancellation.
use std::{array, cell::Cell};
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};

const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];

/// The caller owns the real public API, original inputs and output assertions.
/// Each invocation creates an independent complete plan under a fresh allowance.
pub fn assert_public_boundary<P>(
    fresh_budget: fn() -> ResourceBudget,
    mut plan: impl FnMut(&mut ResourceBudget, &dyn Fn() -> bool) -> Result<P, InspectError>,
    assert_measured: impl Fn(&P),
    assert_exact: impl Fn(&P),
) {
    let mut measured = fresh_budget();
    let initial = POOLS.map(|pool| measured.remaining(pool));
    let polls = Cell::new(0_usize);
    let complete = plan(&mut measured, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    assert_measured(&complete);
    let costs: [u64; 3] = array::from_fn(|index| initial[index] - measured.remaining(POOLS[index]));
    assert!(costs.iter().all(|cost| *cost > 0));
    drop(complete);

    let mut exact = ResourceBudget::new(costs[0], costs[1], costs[2]);
    let complete = plan(&mut exact, &|| false).unwrap();
    assert_exact(&complete);
    assert_eq!(POOLS.map(|pool| exact.remaining(pool)), [0, 0, 0]);
    drop(complete);

    for index in 0..3 {
        let mut short = costs;
        short[index] -= 1;
        let error = plan(
            &mut ResourceBudget::new(short[0], short[1], short[2]),
            &|| false,
        )
        .err()
        .expect("one-less public budget must not publish a plan");
        assert!(matches!(error, InspectError::Limit(failure) if failure.resource == POOLS[index]));
    }

    let terminal = polls.get();
    assert!(terminal > 2);
    for stop in [1, terminal.div_ceil(2), terminal] {
        let seen = Cell::new(0_usize);
        let error = plan(&mut fresh_budget(), &|| {
            seen.set(seen.get() + 1);
            seen.get() >= stop
        })
        .err()
        .expect("cancellation must precede public plan publication");
        assert_eq!(error, InspectError::Cancelled);
        assert_eq!(seen.get(), stop);
    }
}
