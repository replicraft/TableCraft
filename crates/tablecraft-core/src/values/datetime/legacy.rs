//! Timezone-free legacy DATE representation and validation.

use super::super::ValueError;

/// A timezone-free legacy Automation DATE preserving its exact double bits.
///
/// The signed truncated day is relative to 1899-12-30; the absolute fractional
/// part is time of day, including for negative values. No timestamp conversion
/// or numeric ordering is provided. Equality is representational, not temporal.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "DateBits"))]
pub struct LegacyDateTime(u64);

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(transparent)]
struct DateBits(u64);

#[cfg(feature = "serde")]
impl TryFrom<DateBits> for LegacyDateTime {
    type Error = ValueError;

    fn try_from(bits: DateBits) -> Result<Self, Self::Error> {
        Self::from_bits(bits.0)
    }
}

impl LegacyDateTime {
    /// First whole day in the year-100-through-9999 calendar envelope.
    pub const MIN_DAY: i32 = -657_434;
    /// Last whole day in the year-100-through-9999 calendar envelope.
    pub const MAX_DAY: i32 = 2_958_465;

    /// Validate an Automation DATE without changing its floating-point bits.
    pub fn from_ole_days(days: f64) -> Result<Self, ValueError> {
        if !days.is_finite()
            || !(f64::from(Self::MIN_DAY)..=f64::from(Self::MAX_DAY)).contains(&days.trunc())
        {
            return Err(ValueError::InvalidLegacyDateTime);
        }
        Ok(Self(days.to_bits()))
    }

    /// Validate a DATE from exact double bits.
    pub fn from_bits(bits: u64) -> Result<Self, ValueError> {
        Self::from_ole_days(f64::from_bits(bits))
    }

    /// Return the original double bits.
    pub const fn to_bits(self) -> u64 {
        self.0
    }

    /// Return the original Automation DATE double, not a Unix timestamp.
    pub const fn ole_days(self) -> f64 {
        f64::from_bits(self.0)
    }

    /// Return the signed, truncated calendar-day offset from 1899-12-30.
    pub fn day_offset(self) -> i32 {
        self.ole_days().trunc() as i32
    }

    /// Return the nonnegative time-of-day fraction without quantizing it.
    pub fn fraction_of_day(self) -> f64 {
        self.ole_days().fract().abs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_dates_keep_negative_date_rules_and_exact_bits() {
        for (days, day, fraction) in [
            (-2.5, -2, 0.5),
            (-0.25, 0, 0.25),
            (0.25, 0, 0.25),
            (2.5, 2, 0.5),
        ] {
            let value = LegacyDateTime::from_ole_days(days).unwrap();
            assert_eq!(value.day_offset(), day);
            assert_eq!(value.fraction_of_day(), fraction);
            assert_eq!(LegacyDateTime::from_bits(value.to_bits()).unwrap(), value);
            assert_eq!(value.ole_days().to_bits(), days.to_bits());
        }
        assert_ne!(
            LegacyDateTime::from_ole_days(-0.0).unwrap(),
            LegacyDateTime::from_ole_days(0.0).unwrap()
        );
        for days in [-657_434.999, -657_434.0, 2_958_465.999, 2_958_465.0] {
            assert!(LegacyDateTime::from_ole_days(days).is_ok());
        }
        for days in [
            -657_435.0,
            2_958_466.0,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ] {
            assert_eq!(
                LegacyDateTime::from_ole_days(days),
                Err(ValueError::InvalidLegacyDateTime)
            );
        }
    }
}

#[cfg(test)]
mod properties {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(crate::test_support::config())]
        #[test]
        fn validated_dates_preserve_their_input_bits(bits in any::<u64>()) {
            if let Ok(value) = LegacyDateTime::from_bits(bits) {
                prop_assert_eq!(value.to_bits(), bits);
                prop_assert!((LegacyDateTime::MIN_DAY..=LegacyDateTime::MAX_DAY).contains(&value.day_offset()));
                prop_assert!((0.0..1.0).contains(&value.fraction_of_day()));
            }
        }
    }
}
