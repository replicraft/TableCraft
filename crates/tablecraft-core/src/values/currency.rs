//! Exact fixed-scale Currency and checked arithmetic.

use std::fmt;

use super::{ValueError, fixed::display_fixed};

/// Currency as an exact signed integer scaled by 10,000, with no currency symbol.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Currency(i64);

impl Currency {
    /// Construct from units of one ten-thousandth, without float conversion.
    pub const fn from_scaled(scaled: i64) -> Self {
        Self(scaled)
    }

    /// Read units of one ten-thousandth.
    pub const fn scaled(self) -> i64 {
        self.0
    }

    /// Add exact Currency values, rejecting overflow rather than promoting.
    pub fn checked_add(self, rhs: Self) -> Result<Self, ValueError> {
        self.0
            .checked_add(rhs.0)
            .map(Self)
            .ok_or(ValueError::CurrencyOverflow)
    }

    /// Subtract exact Currency values, rejecting overflow rather than wrapping.
    pub fn checked_sub(self, rhs: Self) -> Result<Self, ValueError> {
        self.0
            .checked_sub(rhs.0)
            .map(Self)
            .ok_or(ValueError::CurrencyOverflow)
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display_fixed(i128::from(self.0), 4, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn currency_preserves_entire_signed_range_and_rejects_overflow() {
        let min = Currency::from_scaled(i64::MIN);
        let max = Currency::from_scaled(i64::MAX);
        let unit = Currency::from_scaled(1);
        assert_eq!(min.scaled(), i64::MIN);
        assert_eq!(max.scaled(), i64::MAX);
        assert_eq!(min.to_string(), "-922337203685477.5808");
        assert_eq!(max.to_string(), "922337203685477.5807");
        assert_eq!(unit.to_string(), "0.0001");
        assert_eq!(Currency::from_scaled(-1).to_string(), "-0.0001");
        assert_eq!(max.checked_add(unit), Err(ValueError::CurrencyOverflow));
        assert_eq!(min.checked_sub(unit), Err(ValueError::CurrencyOverflow));
        assert_eq!(min.checked_add(max).unwrap().scaled(), -1);
        assert_eq!(max.checked_sub(max).unwrap().scaled(), 0);
    }
}
