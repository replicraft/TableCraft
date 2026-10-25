use super::*;
use std::cell::Cell;

#[test]
fn decimal_views_preserve_boundaries_and_remove_only_fractional_zeros() {
    for (coefficient, scale, expected) in [
        (10, 1, (1, 0)),
        (100, 2, (1, 0)),
        (-1200, 3, (-12, 1)),
        (10, 0, (10, 0)),
        (0, 28, (0, 0)),
        (1, 28, (1, 28)),
        (Decimal::MAX_COEFFICIENT, 28, (Decimal::MAX_COEFFICIENT, 28)),
        (-Decimal::MAX_COEFFICIENT, 0, (-Decimal::MAX_COEFFICIENT, 0)),
    ] {
        let value = Decimal::new(coefficient, scale).unwrap();
        let mut budget = ResourceBudget::new(0, 0, 100);
        assert_eq!(
            canonical_decimal(value, &mut budget, None).unwrap(),
            expected
        );
        assert_eq!(value.coefficient(), coefficient);
        assert_eq!(value.scale(), scale);
    }
}

#[test]
fn every_fractional_normalization_step_observes_work_and_cancellation() {
    let value = Decimal::new(10_i128.pow(28), 28).unwrap();
    let mut exact = ResourceBudget::new(0, 0, 28);
    assert_eq!(canonical_decimal(value, &mut exact, None).unwrap(), (1, 0));

    let mut short = ResourceBudget::new(0, 0, 27);
    assert!(matches!(
        canonical_decimal(value, &mut short, None),
        Err(QueryError::Limit(_))
    ));
    for checkpoint in 1..=28 {
        let calls = Cell::new(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == checkpoint
        };
        let mut budget = ResourceBudget::new(0, 0, 100);
        assert!(matches!(
            canonical_decimal(value, &mut budget, Some(&cancelled)),
            Err(QueryError::Cancelled)
        ));
        assert_eq!(calls.get(), checkpoint);
    }
}
