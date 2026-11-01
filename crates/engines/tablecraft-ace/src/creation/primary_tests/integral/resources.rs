//! Full public creator measurements, including empty wide-primary tables.
use super::{CreateDatabaseError, budget, invoke, sources};
use crate::InspectError;
use std::{array, cell::Cell};
use tablecraft_core::limits::{Resource, ResourceBudget};

const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];

#[test]
fn wide_primary_creators_pay_measured_exact_pools_and_refuse_one_less() {
    let mut contexts = 0;
    for case in sources::CASES {
        let source = sources::source(case);
        if source.violation.is_some() {
            continue;
        }
        for multiple in [false, true] {
            let mut measured = budget();
            let initial = POOLS.map(|pool| measured.remaining(pool));
            assert_eq!(
                invoke(&source, multiple, &mut measured, &|| false)
                    .unwrap()
                    .as_bytes(),
                source.bytes,
            );
            let costs: [u64; 3] =
                array::from_fn(|position| initial[position] - measured.remaining(POOLS[position]));
            assert!(costs.iter().all(|cost| *cost > 0));
            let mut exact = ResourceBudget::new(costs[0], costs[1], costs[2]);
            assert_eq!(
                invoke(&source, multiple, &mut exact, &|| false)
                    .unwrap()
                    .as_bytes(),
                source.bytes,
            );
            assert_eq!(POOLS.map(|pool| exact.remaining(pool)), [0, 0, 0]);
            for index in 0..3 {
                let mut short = costs;
                short[index] -= 1;
                let mut short = ResourceBudget::new(short[0], short[1], short[2]);
                assert!(matches!(
                    invoke(&source, multiple, &mut short, &|| false),
                    Err(CreateDatabaseError::Inspect(InspectError::Limit(failure)))
                        if failure.resource == POOLS[index]
                ));
            }
            contexts += 1;
        }
    }
    assert_eq!(contexts, 8);
}

#[test]
fn wide_primary_creators_cancel_at_entry_middle_and_actual_terminal_poll() {
    let mut contexts = 0;
    for case in sources::CASES {
        let source = sources::source(case);
        if source.violation.is_some() {
            continue;
        }
        for multiple in [false, true] {
            let seen = Cell::new(0_usize);
            assert_eq!(
                invoke(&source, multiple, &mut budget(), &|| {
                    seen.set(seen.get() + 1);
                    false
                })
                .unwrap()
                .as_bytes(),
                source.bytes,
            );
            let terminal = seen.get();
            assert!(terminal > 2);
            for stop in [1, terminal.div_ceil(2), terminal] {
                seen.set(0);
                assert!(matches!(
                    invoke(&source, multiple, &mut budget(), &|| {
                        seen.set(seen.get() + 1);
                        seen.get() >= stop
                    }),
                    Err(CreateDatabaseError::Inspect(InspectError::Cancelled)),
                ));
                assert_eq!(seen.get(), stop);
            }
            contexts += 1;
        }
    }
    assert_eq!(contexts, 8);
}
