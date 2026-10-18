//! Shared calendar units and the timezone-free Automation epoch.
use time::{Date, Month};

pub(super) const MIN_YEAR: u16 = 1;
pub(super) const MAX_YEAR: u16 = 9999;
pub(super) const HOURS_PER_DAY: u8 = 24;
pub(super) const MINUTES_PER_HOUR: u8 = 60;
pub(super) const SECONDS_PER_MINUTE: u8 = 60;
pub(super) const TICKS_100NS_PER_SECOND: u32 = 10_000_000;
pub(super) const NANOS_PER_TICK: u32 = 100;
pub(super) const NANOS_PER_SECOND: i64 = 1_000_000_000;
pub(super) const NANOS_PER_DAY: i64 =
    HOURS_PER_DAY as i64 * MINUTES_PER_HOUR as i64 * SECONDS_PER_MINUTE as i64 * NANOS_PER_SECOND;
pub(super) const AUTOMATION_EPOCH: Date = match Date::from_calendar_date(1899, Month::December, 30)
{
    Ok(date) => date,
    Err(_) => panic!("invalid compile-time Automation epoch"),
};
