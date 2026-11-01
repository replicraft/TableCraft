//! Preserve the real preview tick/work cancellation and charge contract.
use super::remove;
use crate::{
    EditError, InspectError,
    table::indexes::page::{Entry, KeyRef},
};
use std::cell::Cell;
use tablecraft_core::limits::{Resource, ResourceBudget};

fn entry(row: u32) -> Entry<Option<i64>> {
    Entry {
        value: KeyRef {
            key: Some(i64::from(row)),
            row,
        },
        child: 0,
    }
}

#[test]
fn the_second_poll_precedes_reference_work_even_at_zero_budget_or_zero_work() {
    for references in [&[7_u32][..], &[][..]] {
        let mut entries = vec![entry(7)];
        let mut budget = ResourceBudget::new(0, 0, 1);
        let polls = Cell::new(0);
        let cancelled = || {
            polls.set(polls.get() + 1);
            polls.get() == 2
        };
        assert!(matches!(
            remove(&mut entries, references, &mut budget, &cancelled),
            Err(EditError::Inspect(InspectError::Cancelled))
        ));
        assert_eq!(polls.get(), 2);
        assert_eq!(budget.remaining(Resource::Steps), 0);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].value.row, 7);
    }
}

#[test]
fn stable_removal_keeps_existing_two_polls_and_work_per_original_entry() {
    let mut entries = vec![entry(7), entry(8), entry(9)];
    let mut budget = ResourceBudget::new(0, 0, 20);
    let polls = Cell::new(0);
    let cancelled = || {
        polls.set(polls.get() + 1);
        false
    };
    remove(&mut entries, &[8], &mut budget, &cancelled).unwrap();
    assert_eq!(polls.get(), 6);
    assert_eq!(budget.remaining(Resource::Steps), 14);
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.value.row)
            .collect::<Vec<_>>(),
        [7, 9]
    );
}
