use super::*;
use std::cell::Cell;

#[test]
fn wide_alignment_reserves_scratch_before_computation_and_checks_cancellation() {
    let large = Decimal::new(79228162514264337593543950335, 0).unwrap();
    let tiny = Decimal::new(1, 28).unwrap();
    let mut exhausted = ResourceBudget::new(0, 0, 100);
    assert!(matches!(
        offset(tiny, large, &mut exhausted, None),
        Err(QueryError::Limit(error)) if error.resource == Resource::Bytes
    ));

    let calls = Cell::new(0);
    let cancelled = || {
        calls.set(calls.get() + 1);
        false
    };
    let mut budget = ResourceBudget::new(10000, 0, 100);
    assert!(offset(tiny, large, &mut budget, Some(&cancelled)).is_ok());
    let total = calls.get();
    assert!(total >= 3);
    for stop in 1..=total {
        calls.set(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let mut budget = ResourceBudget::new(10000, 0, 100);
        assert_eq!(
            offset(tiny, large, &mut budget, Some(&cancelled)),
            Err(QueryError::Cancelled),
            "checkpoint {stop}"
        );
    }
}

#[test]
fn narrow_offsets_need_no_heap_allowance_and_keep_the_small_exact_difference() {
    let large = Decimal::new(79228162514264337593543950335, 28).unwrap();
    let next = Decimal::new(79228162514264337593543950334, 28).unwrap();
    let mut budget = ResourceBudget::new(0, 0, 100);
    assert_eq!(offset(large, next, &mut budget, None), Ok(1e-28));
}
