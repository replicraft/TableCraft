//! Independent Gregorian arithmetic and exact ASCII timestamp field framing.
use crate::{EditError, WriteBlocker};
use tablecraft_core::values::ExtendedDateTime;

const DAY_DIGITS: usize = 19;
const SECOND_DIGITS: usize = 12;
const FRACTION_DIGITS: usize = 7;
const SEPARATOR_BYTES: usize = 1;
const TRAILER: &[u8] = b":7\0";
const SECONDS_START: usize = DAY_DIGITS + SEPARATOR_BYTES;
const FRACTION_START: usize = SECONDS_START + SECOND_DIGITS;
const TRAILER_START: usize = FRACTION_START + FRACTION_DIGITS;
const FIELD_BYTES: usize = TRAILER_START + TRAILER.len();
const DAYS_PER_YEAR: u32 = 365;
const FOUR_YEAR_CYCLE: u32 = 4;
const CENTURY: u32 = 100;
const GREGORIAN_CYCLE: u32 = 400;
const MONTH_LENGTHS: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
const FEBRUARY: usize = 1;
const MINUTES_PER_HOUR: u32 = 60;
const SECONDS_PER_MINUTE: u32 = 60;
const RADIX: u32 = 10;

pub(super) fn write(target: &mut [u8], value: &ExtendedDateTime) -> Result<(), EditError> {
    if target.len() != FIELD_BYTES {
        return Err(EditError::Preservation);
    }

    let (year, month, day) = value.date();
    let year = u32::from(year);
    let prior_years = year - 1;
    let mut days = prior_years * DAYS_PER_YEAR + prior_years / FOUR_YEAR_CYCLE
        - prior_years / CENTURY
        + prior_years / GREGORIAN_CYCLE;
    let leap = year.is_multiple_of(FOUR_YEAR_CYCLE)
        && (!year.is_multiple_of(CENTURY) || year.is_multiple_of(GREGORIAN_CYCLE));
    for (position, length) in MONTH_LENGTHS
        .iter()
        .enumerate()
        .take(usize::from(month - 1))
    {
        days += *length;
        if position == FEBRUARY && leap {
            days += 1;
        }
    }
    days += u32::from(day - 1);

    let (hour, minute, second, fraction) = value.time();
    let seconds = (u32::from(hour) * MINUTES_PER_HOUR + u32::from(minute)) * SECONDS_PER_MINUTE
        + u32::from(second);
    digits(&mut target[..DAY_DIGITS], days)?;
    target[DAY_DIGITS] = b':';
    digits(&mut target[SECONDS_START..FRACTION_START], seconds)?;
    digits(&mut target[FRACTION_START..TRAILER_START], fraction)?;
    target[TRAILER_START..].copy_from_slice(TRAILER);
    Ok(())
}

fn digits(target: &mut [u8], mut value: u32) -> Result<(), EditError> {
    for digit in target.iter_mut().rev() {
        *digit = b'0' + (value % RADIX) as u8;
        value /= RADIX;
    }
    if value != 0 {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::write;
    use crate::EditError;
    use tablecraft_core::values::ExtendedDateTime;

    #[test]
    fn timestamp_extremes_have_literal_full_width_digits_and_precision_trailer() {
        for (value, expected) in [
            (
                ExtendedDateTime::new(1, 1, 1, 0, 0, 0, 0).unwrap(),
                b"0000000000000000000:0000000000000000000:7\0".as_slice(),
            ),
            (
                ExtendedDateTime::new(9999, 12, 31, 23, 59, 59, 9_999_999).unwrap(),
                b"0000000000003652058:0000000863999999999:7\0".as_slice(),
            ),
        ] {
            let mut actual = [0xa5; 42];
            write(&mut actual, &value).unwrap();
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn gregorian_centuries_and_fraction_units_are_not_legacy_day_fractions() {
        for (value, expected) in [
            (
                ExtendedDateTime::new(1900, 3, 1, 1, 2, 3, 1).unwrap(),
                b"0000000000000693654:0000000037230000001:7\0".as_slice(),
            ),
            (
                ExtendedDateTime::new(2000, 2, 29, 12, 34, 56, 1_234_567).unwrap(),
                b"0000000000000730178:0000000452961234567:7\0".as_slice(),
            ),
        ] {
            let mut actual = [0xa5; 42];
            write(&mut actual, &value).unwrap();
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn wrong_destination_width_refuses_before_changing_the_literal_span() {
        let value = ExtendedDateTime::new(2024, 2, 29, 0, 0, 0, 0).unwrap();
        for width in [0, 41, 43] {
            let mut bytes = vec![0xa5; width];
            assert_eq!(write(&mut bytes, &value), Err(EditError::Preservation));
            assert_eq!(bytes, vec![0xa5; width]);
        }
    }
}
