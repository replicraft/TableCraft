use super::*;
use std::cell::Cell;

#[test]
fn duplicate_and_colliding_entries_keep_encounter_order_through_rebuilds() {
    let mut directory = Directory::default();
    let mut budget = ResourceBudget::new(1_000_000, 1_000, 1_000_000);
    for row in 0..300 {
        let fingerprint = if row % 3 == 0 { 0 } else { 1_u64 << 32 };
        directory
            .insert(fingerprint, row, &mut budget, None)
            .unwrap();
    }
    let mut position = directory.first(0);
    let mut all = Vec::new();
    let mut matching = Vec::new();
    while let Some(index) = position {
        check(&mut budget, None).unwrap();
        let entry = directory.entry(index);
        all.push(entry.row);
        if entry.fingerprint == 0 {
            matching.push(entry.row);
        }
        position = entry.next;
    }
    assert_eq!(all, (0..300).collect::<Vec<_>>());
    assert_eq!(matching, (0..300).step_by(3).collect::<Vec<_>>());
}

#[test]
fn lazy_directory_and_growth_charge_actual_storage_without_refunds() {
    let mut directory = Directory::default();
    let mut budget = ResourceBudget::new(0, 1_000, 1_000);
    assert_eq!(directory.first(0), None);
    assert_eq!(budget.remaining(Resource::Bytes), 0);
    assert!(matches!(
        directory.insert(0, 0, &mut budget, None),
        Err(QueryError::Limit(_))
    ));

    let mut budget = ResourceBudget::new(1_000_000, 1_000, 1_000_000);
    directory.insert(0, 0, &mut budget, None).unwrap();
    assert_eq!(
        1_000_000 - budget.remaining(Resource::Bytes),
        (8 * size_of::<Bucket>() + size_of::<Entry>()) as u64
    );
    let before = budget.remaining(Resource::Bytes);
    directory.insert(0, 1, &mut budget, None).unwrap();
    assert_eq!(
        before - budget.remaining(Resource::Bytes),
        size_of::<Entry>() as u64
    );
}

#[test]
fn every_observed_growth_checkpoint_can_cancel_without_a_completed_index() {
    let observed = Cell::new(0);
    let observe = || {
        observed.set(observed.get() + 1);
        false
    };
    let mut directory = Directory::default();
    let mut budget = ResourceBudget::new(1_000_000, 1_000, 1_000_000);
    for row in 0..32 {
        directory
            .insert(0, row, &mut budget, Some(&observe))
            .unwrap();
    }
    for stop in 1..=observed.get() {
        let calls = Cell::new(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let mut directory = Directory::default();
        let mut budget = ResourceBudget::new(1_000_000, 1_000, 1_000_000);
        let result =
            (0..32).try_for_each(|row| directory.insert(0, row, &mut budget, Some(&cancelled)));
        assert_eq!(result, Err(QueryError::Cancelled));
    }
}
