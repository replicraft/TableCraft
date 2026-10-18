//! Timezone-free legacy DATE representation and validation.

use super::super::ValueError;
use super::calendar;
use std::cmp::Ordering;
use time::{Duration, PrimitiveDateTime, Time};

/// A timezone-free legacy Automation DATE preserving its exact double bits.
///
/// The signed truncated day is relative to 1899-12-30; the absolute fractional
/// part is time of day, including for negative values. No timestamp conversion
/// is performed implicitly. Equality is representational, not temporal.
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

    /// Convert a timezone-free calendar value to the canonical Automation encoding.
    /// Fractions are rounded by IEEE double arithmetic; unrepresentable endpoints
    /// fail rather than clamping. Negative days subtract the positive time fraction.
    pub fn from_calendar(value: PrimitiveDateTime) -> Result<Self, ValueError> {
        let day = (value.date() - calendar::AUTOMATION_EPOCH).whole_days();
        let nanos = (value.time() - Time::MIDNIGHT).whole_nanoseconds();
        let fraction = nanos as f64 / calendar::NANOS_PER_DAY as f64;
        Self::from_ole_days(if day < 0 {
            day as f64 - fraction
        } else {
            day as f64 + fraction
        })
    }

    /// Convert to a timezone-free calendar, rounding the time fraction to nearest
    /// nanosecond with ties to even. Stored bits are never changed. A rounded
    /// calendar overflow returns an error; this is not an exact byte round trip.
    pub fn calendar_rounded_to_nanos(self) -> Result<PrimitiveDateTime, ValueError> {
        let date = calendar::AUTOMATION_EPOCH
            .checked_add(Duration::days(i64::from(self.day_offset())))
            .ok_or(ValueError::InvalidLegacyDateTime)?;
        let nanos =
            (self.fraction_of_day() * calendar::NANOS_PER_DAY as f64).round_ties_even() as i64;
        date.midnight()
            .checked_add(Duration::nanoseconds(nanos))
            .ok_or(ValueError::InvalidLegacyDateTime)
    }

    /// Compare calendar-day/time fractions, independently of representational Eq.
    /// For example -0.25 and +0.25 denote the same time on the epoch date.
    pub fn temporal_cmp(self, other: Self) -> Ordering {
        self.day_offset()
            .cmp(&other.day_offset())
            .then_with(|| self.fraction_of_day().total_cmp(&other.fraction_of_day()))
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

#[cfg(test)]
mod calendar_tests {
    use super::*;
    use time::{Date, Month};
    #[test]
    fn temporal_order_and_calendar_conversion_do_not_change_representation_equality() {
        let early = LegacyDateTime::from_ole_days(-1.25).unwrap();
        let late = LegacyDateTime::from_ole_days(-1.75).unwrap();
        assert_eq!(early.temporal_cmp(late), Ordering::Less);
        let negative = LegacyDateTime::from_ole_days(-0.25).unwrap();
        let positive = LegacyDateTime::from_ole_days(0.25).unwrap();
        assert_ne!(negative, positive);
        assert_eq!(negative.temporal_cmp(positive), Ordering::Equal);
        let calendar = early.calendar_rounded_to_nanos().unwrap();
        assert_eq!(
            calendar.date(),
            Date::from_calendar_date(1899, Month::December, 29).unwrap()
        );
        assert_eq!(calendar.time().as_hms(), (6, 0, 0));
        assert_eq!(LegacyDateTime::from_calendar(calendar).unwrap(), early);
        for (year, month, day) in [(100, Month::January, 1), (9999, Month::December, 31)] {
            let value = Date::from_calendar_date(year, month, day)
                .unwrap()
                .midnight();
            assert_eq!(
                LegacyDateTime::from_calendar(value)
                    .unwrap()
                    .calendar_rounded_to_nanos()
                    .unwrap(),
                value
            );
        }
        assert!(
            LegacyDateTime::from_calendar(
                Date::from_calendar_date(99, Month::December, 31)
                    .unwrap()
                    .midnight()
            )
            .is_err()
        );
    }
}
