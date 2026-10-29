use crate::{InspectError, format::data_type::EXTENDED_DATE_TIME_BYTES};
#[cfg(feature = "ace16-write-preview")]
use jiff::Unit;
use jiff::{Span, civil::Date};
use tablecraft_core::values::ExtendedDateTime;

const DAYS_END: usize = 19;
const SECONDS_START: usize = DAYS_END + 1;
const SECONDS_END: usize = SECONDS_START + 12;
const FRACTION_END: usize = SECONDS_END + 7;
const TRAILER: &[u8] = b":7\0";
const EPOCH: Date = Date::constant(1, 1, 1);
// Gregorian days from 0001-01-01 through 9999-12-31; no timezone is involved.
const MAX_DAY_OFFSET: u64 = 3_652_058;
const SECONDS_PER_MINUTE: u64 = 60;
const MINUTES_PER_HOUR: u64 = 60;
const HOURS_PER_DAY: u64 = 24;
const SECONDS_PER_HOUR: u64 = SECONDS_PER_MINUTE * MINUTES_PER_HOUR;
const SECONDS_PER_DAY: u64 = SECONDS_PER_HOUR * HOURS_PER_DAY;
const RADIX: u64 = 10;

// Canonical Access extended timestamp: 19 day digits, ':', 12 second digits,
// seven 100ns digits and ':7\0'. Validate every byte instead of accepting an
// overflow, normalization into another day, or an unqualified precision tag.
pub(super) fn decode(bytes: &[u8]) -> Result<ExtendedDateTime, InspectError> {
    if bytes.len() != EXTENDED_DATE_TIME_BYTES
        || bytes[DAYS_END] != b':'
        || &bytes[FRACTION_END..] != TRAILER
    {
        return Err(InspectError::Malformed);
    }
    let days = digits(&bytes[..DAYS_END])?;
    let seconds = digits(&bytes[SECONDS_START..SECONDS_END])?;
    let fraction = digits(&bytes[SECONDS_END..FRACTION_END])?;
    if days > MAX_DAY_OFFSET || seconds >= SECONDS_PER_DAY {
        return Err(InspectError::Malformed);
    }
    let date = EPOCH
        .checked_add(Span::new().days(days as i64))
        .map_err(|_| InspectError::Malformed)?;
    ExtendedDateTime::new(
        date.year() as u16,
        date.month() as u8,
        date.day() as u8,
        (seconds / SECONDS_PER_HOUR) as u8,
        (seconds % SECONDS_PER_HOUR / SECONDS_PER_MINUTE) as u8,
        (seconds % SECONDS_PER_MINUTE) as u8,
        fraction as u32,
    )
    .map_err(|_| InspectError::Malformed)
}

/// Encode an already validated civil carrier, without timezone or DATE coercion.
#[cfg(feature = "ace16-write-preview")]
pub(super) fn encode(
    value: &ExtendedDateTime,
) -> Result<[u8; EXTENDED_DATE_TIME_BYTES], InspectError> {
    let (year, month, day) = value.date();
    let date =
        Date::new(year as i16, month as i8, day as i8).map_err(|_| InspectError::Malformed)?;
    let elapsed = date
        .since((Unit::Day, EPOCH))
        .map_err(|_| InspectError::Malformed)?;
    let days = u64::try_from(elapsed.get_days()).map_err(|_| InspectError::Malformed)?;
    let (hour, minute, second, fraction) = value.time();
    let seconds = u64::from(hour) * SECONDS_PER_HOUR
        + u64::from(minute) * SECONDS_PER_MINUTE
        + u64::from(second);

    let mut bytes = [b'0'; EXTENDED_DATE_TIME_BYTES];
    write_digits(&mut bytes[..DAYS_END], days)?;
    bytes[DAYS_END] = b':';
    write_digits(&mut bytes[SECONDS_START..SECONDS_END], seconds)?;
    write_digits(&mut bytes[SECONDS_END..FRACTION_END], u64::from(fraction))?;
    bytes[FRACTION_END..].copy_from_slice(TRAILER);
    Ok(bytes)
}

#[cfg(feature = "ace16-write-preview")]
fn write_digits(bytes: &mut [u8], mut value: u64) -> Result<(), InspectError> {
    for digit in bytes.iter_mut().rev() {
        *digit = b'0' + (value % RADIX) as u8;
        value /= RADIX;
    }
    if value != 0 {
        return Err(InspectError::Malformed);
    }
    Ok(())
}

fn digits(bytes: &[u8]) -> Result<u64, InspectError> {
    bytes.iter().try_fold(0_u64, |value, digit| {
        if !digit.is_ascii_digit() {
            return Err(InspectError::Malformed);
        }
        value
            .checked_mul(RADIX)
            .and_then(|value| value.checked_add(u64::from(digit - b'0')))
            .ok_or(InspectError::Malformed)
    })
}

#[cfg(test)]
mod tests;
