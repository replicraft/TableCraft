//! Timezone-free legacy DATE representation and validation.

use super::super::ValueError;
use super::calendar;
use jiff::{
    SignedDuration, Span,
    civil::{DateTime, Time},
};
use std::cmp::Ordering;

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
    #[inline]
    pub fn from_calendar(value: DateTime) -> Result<Self, ValueError> {
        let day = value
            .date()
            .duration_since(calendar::AUTOMATION_EPOCH)
            .as_secs()
            / calendar::SECONDS_PER_DAY;
        // A checked civil time is less than one day: its integral nanoseconds
        // fit in i64 (and exactly in f64), without an i128 duration conversion.
        let seconds = (i64::from(value.hour()) * i64::from(calendar::MINUTES_PER_HOUR)
            + i64::from(value.minute()))
            * i64::from(calendar::SECONDS_PER_MINUTE)
            + i64::from(value.second());
        let nanos = seconds * calendar::NANOS_PER_SECOND + i64::from(value.subsec_nanosecond());
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
    #[inline]
    pub fn calendar_rounded_to_nanos(self) -> Result<DateTime, ValueError> {
        let mut nanos =
            (self.fraction_of_day() * calendar::NANOS_PER_DAY as f64).round_ties_even() as i64;
        // Rounding can carry midnight into the next civil day. Fold that carry
        // into the one date operation instead of rechecking it through DateTime
        // duration arithmetic. Time::checked_add still validates the civil time.
        // The validated fraction is in [0, 1), so nearest-nanosecond rounding
        // gives [0, NANOS_PER_DAY]. There can be only one day of carry.
        let mut day = i64::from(self.day_offset());
        if nanos == calendar::NANOS_PER_DAY {
            day += 1;
            nanos = 0;
        }
        let time = Time::MIN
            .checked_add(SignedDuration::from_nanos(nanos))
            .map_err(|_| ValueError::InvalidLegacyDateTime)?;
        // Civil day offsets cover the full Access range. Jiff's absolute-duration
        // date path bounds the offset relative to Unix days, which is narrower.
        let days = Span::new()
            .try_days(day)
            .map_err(|_| ValueError::InvalidLegacyDateTime)?;
        let date = calendar::AUTOMATION_EPOCH
            .checked_add(days)
            .map_err(|_| ValueError::InvalidLegacyDateTime)?;
        Ok(date.to_datetime(time))
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
    use jiff::SignedDuration;
    use jiff::civil::Date;
    use proptest::prelude::*;

    // Keep the general Jiff duration route as a test oracle for the specialized
    // adapter. Neither reference decomposes civil time into component fields.
    fn reference_decode(value: LegacyDateTime) -> Result<DateTime, ValueError> {
        let days = Span::new().try_days(i64::from(value.day_offset())).unwrap();
        let date = calendar::AUTOMATION_EPOCH.checked_add(days).unwrap();
        let nanos = (value.fraction_of_day() * 86_400_000_000_000.0).round_ties_even() as i64;
        date.to_datetime(Time::MIN)
            .checked_add(SignedDuration::from_nanos(nanos))
            .map_err(|_| ValueError::InvalidLegacyDateTime)
    }

    fn reference_encode(value: DateTime) -> Result<LegacyDateTime, ValueError> {
        let day = value
            .date()
            .duration_since(Date::constant(1899, 12, 30))
            .as_secs()
            / 86_400;
        let fraction =
            value.time().duration_since(Time::MIN).as_nanos() as f64 / 86_400_000_000_000.0;
        LegacyDateTime::from_ole_days(if day < 0 {
            day as f64 - fraction
        } else {
            day as f64 + fraction
        })
    }

    #[test]
    fn calendar_projection_covers_centuries_and_both_field_endpoints() {
        for (days, expected) in [
            (-657_434.0, Date::constant(100, 1, 1).at(0, 0, 0, 0)),
            (-1.25, Date::constant(1899, 12, 29).at(6, 0, 0, 0)),
            (60.5, Date::constant(1900, 2, 28).at(12, 0, 0, 0)),
            (61.0, Date::constant(1900, 3, 1).at(0, 0, 0, 0)),
            (36_585.75, Date::constant(2000, 2, 29).at(18, 0, 0, 0)),
            (36_586.0, Date::constant(2000, 3, 1).at(0, 0, 0, 0)),
            (2_958_465.0, Date::constant(9999, 12, 31).at(0, 0, 0, 0)),
        ] {
            let value = LegacyDateTime::from_ole_days(days).unwrap();
            assert_eq!(value.calendar_rounded_to_nanos().unwrap(), expected);
            assert_eq!(LegacyDateTime::from_calendar(expected).unwrap(), value);
        }
        // A valid civil input can round beyond the storage envelope. Keep the
        // checked error instead of clamping to the last representable double.
        assert_eq!(
            LegacyDateTime::from_calendar(Date::MAX.to_datetime(Time::MAX)),
            Err(ValueError::InvalidLegacyDateTime)
        );
        let last = LegacyDateTime::from_ole_days(2_958_466.0_f64.next_down()).unwrap();
        assert_eq!(last.calendar_rounded_to_nanos(), reference_decode(last));
        assert_eq!(last.calendar_rounded_to_nanos().unwrap().date(), Date::MAX);
    }

    #[test]
    fn nanosecond_ties_and_midnight_carries_preserve_stored_bits() {
        for sign in [-1.0, 1.0] {
            for (nanos, rounded) in [(0.5, 0), (1.5, 2), (2.5, 2), (3.5, 4)] {
                let days = sign * nanos / 86_400_000_000_000.0;
                let value = LegacyDateTime::from_ole_days(days).unwrap();
                assert_eq!(
                    value.calendar_rounded_to_nanos().unwrap(),
                    Date::constant(1899, 12, 30).at(0, 0, 0, rounded)
                );
                assert_eq!(value.to_bits(), days.to_bits());
            }
            let days = sign * 1.0_f64.next_down();
            let value = LegacyDateTime::from_ole_days(days).unwrap();
            assert_eq!(
                value.calendar_rounded_to_nanos().unwrap(),
                Date::constant(1899, 12, 31).at(0, 0, 0, 0)
            );
            assert_eq!(value.to_bits(), days.to_bits());
        }
    }

    proptest! {
        #![proptest_config(crate::test_support::config())]
        #[test]
        fn calendar_decode_matches_general_jiff_arithmetic(
            day in LegacyDateTime::MIN_DAY..=LegacyDateTime::MAX_DAY,
            fraction in 0_u64..(1_u64 << 53),
        ) {
            let fraction = fraction as f64 / (1_u64 << 53) as f64;
            let days = if day < 0 { f64::from(day) - fraction } else { f64::from(day) + fraction };
            if let Ok(value) = LegacyDateTime::from_ole_days(days) {
                prop_assert_eq!(value.calendar_rounded_to_nanos(), reference_decode(value));
                prop_assert_eq!(value.to_bits(), days.to_bits());
            }
        }

        #[test]
        fn calendar_decode_matches_for_arbitrary_valid_bits(bits in any::<u64>()) {
            if let Ok(value) = LegacyDateTime::from_bits(bits) {
                prop_assert_eq!(value.calendar_rounded_to_nanos(), reference_decode(value));
            }
        }

        #[test]
        fn calendar_encode_matches_general_jiff_durations(
            year in 99_i16..=9999,
            ordinal in 1_i16..=365,
            hour in 0_i8..24,
            minute in 0_i8..60,
            second in 0_i8..60,
            nanos in 0_i32..1_000_000_000,
        ) {
            let date = Date::new(year, 1, 1).unwrap().with().day_of_year(ordinal).build().unwrap();
            let value = date.at(hour, minute, second, nanos);
            prop_assert_eq!(LegacyDateTime::from_calendar(value), reference_encode(value));
        }
    }

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
        assert_eq!(calendar.date(), Date::new(1899, 12, 29).unwrap());
        assert_eq!(
            (calendar.hour(), calendar.minute(), calendar.second()),
            (6, 0, 0)
        );
        assert_eq!(LegacyDateTime::from_calendar(calendar).unwrap(), early);
        for (year, month, day) in [(100, 1, 1), (9999, 12, 31)] {
            let value = Date::new(year, month, day).unwrap().to_datetime(Time::MIN);
            assert_eq!(
                LegacyDateTime::from_calendar(value)
                    .unwrap()
                    .calendar_rounded_to_nanos()
                    .unwrap(),
                value
            );
        }
        assert!(
            LegacyDateTime::from_calendar(Date::new(99, 12, 31).unwrap().to_datetime(Time::MIN))
                .is_err()
        );
    }
}
