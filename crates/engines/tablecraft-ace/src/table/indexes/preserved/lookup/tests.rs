use super::Lookup;
use crate::InspectError;
use std::cell::Cell;
use tablecraft_core::limits::{Resource, ResourceBudget};

#[test]
fn ordered_and_unordered_roots_keep_first_duplicate_and_missing_semantics() {
    for expected in [
        [(10, None), (20, Some(1)), (20, Some(2)), (30, Some(3))],
        [(20, Some(1)), (10, None), (30, Some(3)), (20, Some(2))],
    ] {
        let mut budget = ResourceBudget::new(0, 0, 100);
        let lookup = Lookup::capture(&expected, &mut budget, &|| false).unwrap();
        for reference in [0, 10, 20, 25, 30, 40] {
            let original = expected.iter().position(|(root, _)| *root == reference);
            assert_eq!(
                lookup.position(reference, &mut budget, &|| false).unwrap(),
                original,
                "reference={reference}"
            );
        }
        assert_eq!(budget.remaining(Resource::Bytes), 0);
    }
}

#[test]
fn ordered_population_lookup_fits_paid_logarithmic_work_without_allocation() {
    let expected = (0..5_000)
        .map(|reference| (reference * 2, None))
        .collect::<Vec<_>>();
    let mut budget = ResourceBudget::new(0, 0, 100_000);
    let lookup = Lookup::capture(&expected, &mut budget, &|| false).unwrap();
    for (position, (reference, _)) in expected.iter().enumerate().rev() {
        assert_eq!(
            lookup.position(*reference, &mut budget, &|| false).unwrap(),
            Some(position)
        );
    }
    assert_eq!(
        lookup.position(9_999, &mut budget, &|| false).unwrap(),
        None
    );
    assert_eq!(budget.remaining(Resource::Bytes), 0);
}

#[test]
fn discovery_and_both_lookup_modes_keep_budget_and_late_cancellation() {
    let expected = [(10, None), (20, None), (30, None), (40, None)];
    let mut empty = ResourceBudget::new(0, 0, 0);
    assert!(matches!(
        Lookup::capture(&expected, &mut empty, &|| false),
        Err(InspectError::Limit(_))
    ));
    let calls = Cell::new(0);
    let cancel = || {
        calls.set(calls.get() + 1);
        calls.get() > 2
    };
    assert!(matches!(
        Lookup::capture(&expected, &mut ResourceBudget::new(0, 0, 100), &cancel),
        Err(InspectError::Cancelled)
    ));

    for expected in [expected, [(40, None), (10, None), (30, None), (20, None)]] {
        let lookup =
            Lookup::capture(&expected, &mut ResourceBudget::new(0, 0, 100), &|| false).unwrap();
        assert!(matches!(
            lookup.position(99, &mut empty, &|| false),
            Err(InspectError::Limit(_))
        ));
        calls.set(0);
        assert!(matches!(
            lookup.position(99, &mut ResourceBudget::new(0, 0, 100), &cancel),
            Err(InspectError::Cancelled)
        ));
    }
}
