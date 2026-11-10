//! Check Long extent arithmetic without allocating enormous element buffers.
use super::checked_upper_bound;
use crate::EvalError;

#[test]
fn extent_uses_both_long_bounds_without_a_signed_length_cutoff() {
    let count = usize::try_from(i64::from(i32::MAX) + 1).unwrap();
    assert_eq!(checked_upper_bound(0, count), Ok(i32::MAX));
    assert_eq!(checked_upper_bound(-1, count), Ok(i32::MAX - 1));
    assert_eq!(checked_upper_bound(1, count), Err(EvalError::ArrayBounds));
    assert_eq!(
        checked_upper_bound(i32::MIN, 0),
        Err(EvalError::ArrayBounds)
    );
    assert_eq!(checked_upper_bound(i32::MIN, 1), Ok(i32::MIN));
    assert_eq!(
        checked_upper_bound(i32::MAX, 2),
        Err(EvalError::ArrayBounds)
    );
    assert_eq!(
        checked_upper_bound(0, usize::MAX),
        Err(EvalError::ArrayBounds)
    );
}
