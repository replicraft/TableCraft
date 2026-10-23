//! Checked numbered-message identity; separate from standard CVErr data codes.
use crate::EvalError;

const MAX_NUMBER: i32 = u16::MAX as i32;

/// A Long error-message number: negative codes and 0..=65535 are admitted.
/// This preserves signed identity without interpreting host HRESULT bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ErrorMessageCode(i32);

impl ErrorMessageCode {
    /// Validate the numbered Error function's range after checked Long conversion.
    pub fn new(number: i32) -> Result<Self, EvalError> {
        if number > MAX_NUMBER {
            return Err(EvalError::Overflow);
        }
        Ok(Self(number))
    }

    /// Return the exact signed number supplied to message lookup.
    pub const fn number(self) -> i32 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn signed_bounds_and_positive_ceiling_preserve_message_identity() {
        for number in [i32::MIN, -2147221504, -65536, -1, 0, 1, 65535] {
            assert_eq!(ErrorMessageCode::new(number).unwrap().number(), number);
        }
        for number in [65536, i32::MAX] {
            assert_eq!(ErrorMessageCode::new(number), Err(EvalError::Overflow));
        }
    }

    proptest! {
        #![proptest_config(crate::test_support::config())]
        #[test]
        fn all_admitted_long_values_round_trip_without_unsigned_reinterpretation(number in i32::MIN..=65535) {
            prop_assert_eq!(ErrorMessageCode::new(number).unwrap().number(), number);
        }
    }
}
