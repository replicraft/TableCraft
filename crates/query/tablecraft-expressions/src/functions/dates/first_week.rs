//! Caller-issued rule for explicit system-dependent first-week requests.
use crate::EvalError;

/// Validated Access first-week rule: Jan-1 week, four-day week, or full week.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FirstWeekOfYear(u8);
impl FirstWeekOfYear {
    /// Week containing January 1.
    pub const MIN: u8 = 1;
    /// First complete week of the year.
    pub const MAX: u8 = 3;
    /// Choose a concrete rule in 1..=3, excluding the system sentinel zero.
    pub fn new(rule: u8) -> Result<Self, EvalError> {
        if !(Self::MIN..=Self::MAX).contains(&rule) {
            return Err(EvalError::Domain);
        }
        Ok(Self(rule))
    }
    /// Access rule number: 1=Jan-1, 2=four-day, 3=full-week.
    pub const fn number(self) -> u8 {
        self.0
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concrete_rules_exclude_system_and_invalid_settings() {
        for rule in [0, 4, u8::MAX] {
            assert_eq!(FirstWeekOfYear::new(rule), Err(EvalError::Domain));
        }
        for rule in 1..=3 {
            assert_eq!(FirstWeekOfYear::new(rule).unwrap().number(), rule);
        }
    }
}
