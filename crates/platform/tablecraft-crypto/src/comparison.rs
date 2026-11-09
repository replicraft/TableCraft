//! Constant-time content comparison.
use subtle::ConstantTimeEq;

/// Compare equal-length contents without content-dependent early termination.
/// Length equality is public and checked separately by the primitive.
pub fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    bool::from(left.ct_eq(right))
}
