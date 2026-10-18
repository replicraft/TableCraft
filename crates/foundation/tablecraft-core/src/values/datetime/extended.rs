//! Gregorian extended date/time with seven fractional-second digits.

use super::super::ValueError;
use super::calendar;

/// A Gregorian, timezone-free date/time with seven fractional-second digits.
///
/// Years are 1..=9999. A fractional unit is 100 ns. This logical carrier does
/// not implement Access's native 42-byte encoding or calendar conversions.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "DateTimeRepr"))]
pub struct ExtendedDateTime {
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    fraction_100ns: u32,
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DateTimeRepr {
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    fraction_100ns: u32,
}

#[cfg(feature = "serde")]
impl TryFrom<DateTimeRepr> for ExtendedDateTime {
    type Error = ValueError;

    fn try_from(repr: DateTimeRepr) -> Result<Self, Self::Error> {
        Self::new(
            repr.year,
            repr.month,
            repr.day,
            repr.hour,
            repr.minute,
            repr.second,
            repr.fraction_100ns,
        )
    }
}

impl ExtendedDateTime {
    /// Validate Gregorian calendar components and a 0..=9,999,999 fraction.
    pub fn new(
        year: u16,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
        fraction_100ns: u32,
    ) -> Result<Self, ValueError> {
        let days =
            calendar::days_in_month(year, month).ok_or(ValueError::InvalidExtendedDateTime)?;
        if !(calendar::MIN_YEAR..=calendar::MAX_YEAR).contains(&year)
            || day == 0
            || day > days
            || hour >= calendar::HOURS_PER_DAY
            || minute >= calendar::MINUTES_PER_HOUR
            || second >= calendar::SECONDS_PER_MINUTE
            || fraction_100ns >= calendar::TICKS_100NS_PER_SECOND
        {
            return Err(ValueError::InvalidExtendedDateTime);
        }
        Ok(Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            fraction_100ns,
        })
    }

    /// Return year, month and day without introducing a timezone.
    pub const fn date(self) -> (u16, u8, u8) {
        (self.year, self.month, self.day)
    }

    /// Return hour, minute, second and 100 ns units within that second.
    pub const fn time(self) -> (u8, u8, u8, u32) {
        (self.hour, self.minute, self.second, self.fraction_100ns)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extended_dates_validate_calendar_and_full_fraction_without_timezone() {
        let min = ExtendedDateTime::new(1, 1, 1, 0, 0, 0, 0).unwrap();
        let max = ExtendedDateTime::new(9999, 12, 31, 23, 59, 59, 9_999_999).unwrap();
        assert_eq!(min.date(), (1, 1, 1));
        assert_eq!(max.date(), (9999, 12, 31));
        assert_eq!(max.time(), (23, 59, 59, 9_999_999));
        for year in [4, 400, 1600, 2000, 2400] {
            assert!(ExtendedDateTime::new(year, 2, 29, 0, 0, 0, 0).is_ok());
        }
        for (year, month, day, hour, minute, second, fraction) in [
            (0, 1, 1, 0, 0, 0, 0),
            (10000, 1, 1, 0, 0, 0, 0),
            (1900, 2, 29, 0, 0, 0, 0),
            (2100, 2, 29, 0, 0, 0, 0),
            (2024, 0, 1, 0, 0, 0, 0),
            (2024, 13, 1, 0, 0, 0, 0),
            (2024, 4, 31, 0, 0, 0, 0),
            (2024, 1, 0, 0, 0, 0, 0),
            (2024, 1, 1, 24, 0, 0, 0),
            (2024, 1, 1, 0, 60, 0, 0),
            (2024, 1, 1, 0, 0, 60, 0),
            (2024, 1, 1, 0, 0, 0, 10_000_000),
        ] {
            assert_eq!(
                ExtendedDateTime::new(year, month, day, hour, minute, second, fraction),
                Err(ValueError::InvalidExtendedDateTime)
            );
        }
    }
}
