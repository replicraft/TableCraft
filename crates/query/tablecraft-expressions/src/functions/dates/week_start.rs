//! Caller-issued first-day setting for explicit system-dependent calendar calls.
use crate::EvalError;

/// Validated Access weekday numbering: Sunday is 1 and Saturday is 7.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FirstDayOfWeek(u8);
impl FirstDayOfWeek {
    /// First valid weekday number (Sunday).
    pub const MIN: u8 = 1;
    /// Last valid weekday number (Saturday).
    pub const MAX: u8 = 7;
    /// Supply a concrete host setting, rather than the system sentinel zero.
    pub fn new(day: u8) -> Result<Self, EvalError> {
        if !(Self::MIN..=Self::MAX).contains(&day) {
            return Err(EvalError::Domain);
        }
        Ok(Self(day))
    }
    /// Access weekday number in 1..=7.
    pub const fn number(self) -> u8 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn host_setting_is_concrete_and_in_range() {
        for day in [0, 8, u8::MAX] {
            assert_eq!(FirstDayOfWeek::new(day), Err(EvalError::Domain));
        }
        for day in 1..=7 {
            assert_eq!(FirstDayOfWeek::new(day).unwrap().number(), day);
        }
    }
}
