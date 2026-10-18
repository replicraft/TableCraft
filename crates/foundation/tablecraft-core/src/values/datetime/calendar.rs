//! Gregorian calendar and time-unit facts for the extended carrier validator.
pub(super) const MIN_YEAR: u16 = 1;
pub(super) const MAX_YEAR: u16 = 9999;
pub(super) const HOURS_PER_DAY: u8 = 24;
pub(super) const MINUTES_PER_HOUR: u8 = 60;
pub(super) const SECONDS_PER_MINUTE: u8 = 60;
pub(super) const TICKS_100NS_PER_SECOND: u32 = 10_000_000;
const YEARS_PER_LEAP_INTERVAL: u16 = 4;
const YEARS_PER_CENTURY: u16 = 100;
const YEARS_PER_GREGORIAN_CYCLE: u16 = 400;
const JANUARY: u8 = 1;
const FEBRUARY: u8 = 2;
const MARCH: u8 = 3;
const APRIL: u8 = 4;
const MAY: u8 = 5;
const JUNE: u8 = 6;
const JULY: u8 = 7;
const AUGUST: u8 = 8;
const SEPTEMBER: u8 = 9;
const OCTOBER: u8 = 10;
const NOVEMBER: u8 = 11;
const DECEMBER: u8 = 12;
const LONG_MONTH_DAYS: u8 = 31;
const SHORT_MONTH_DAYS: u8 = 30;
const FEBRUARY_DAYS: u8 = 28;
const LEAP_FEBRUARY_DAYS: u8 = FEBRUARY_DAYS + 1;

pub(super) fn days_in_month(year: u16, month: u8) -> Option<u8> {
    let leap = year.is_multiple_of(YEARS_PER_LEAP_INTERVAL)
        && (!year.is_multiple_of(YEARS_PER_CENTURY)
            || year.is_multiple_of(YEARS_PER_GREGORIAN_CYCLE));
    Some(match month {
        JANUARY | MARCH | MAY | JULY | AUGUST | OCTOBER | DECEMBER => LONG_MONTH_DAYS,
        APRIL | JUNE | SEPTEMBER | NOVEMBER => SHORT_MONTH_DAYS,
        FEBRUARY if leap => LEAP_FEBRUARY_DAYS,
        FEBRUARY => FEBRUARY_DAYS,
        _ => return None,
    })
}
