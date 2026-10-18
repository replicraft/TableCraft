//! Gregorian extended date/time with seven fractional-second digits.

use super::super::ValueError;

/// A Gregorian, timezone-free date/time with seven fractional-second digits.
///
/// Years are 1..=9999. A fractional unit is 100 ns. This logical carrier does
/// not implement Access's native 42-byte encoding or calendar conversions.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ExtendedDateTime {
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    fraction_100ns: u32,
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
        let leap =
            year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
        let days = match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if leap => 29,
            2 => 28,
            _ => 0,
        };
        if !(1..=9999).contains(&year)
            || day == 0
            || day > days
            || hour > 23
            || minute > 59
            || second > 59
            || fraction_100ns > 9_999_999
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
