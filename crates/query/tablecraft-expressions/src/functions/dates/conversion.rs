//! Shared timezone-free conversions for the explicit date-function profile.
use crate::EvalError;
use jiff::{
    RoundMode, Unit,
    civil::{DateTime, DateTimeRound},
};
use tablecraft_core::values::{ExtendedDateTime, LegacyDateTime, Value};

const NANOS_PER_100NS_UNIT: i32 = 100;

pub(super) fn legacy_calendar(value: LegacyDateTime) -> Result<DateTime, EvalError> {
    value
        .calendar_rounded_to_nanos()
        .map_err(|_| EvalError::Overflow)?
        .round(
            DateTimeRound::new()
                .smallest(Unit::Second)
                .mode(RoundMode::HalfExpand),
        )
        .map_err(|_| EvalError::Overflow)
}
pub(super) fn extended_calendar(value: ExtendedDateTime) -> Result<DateTime, EvalError> {
    let (year, month, day) = value.date();
    let (hour, minute, second, fraction) = value.time();
    DateTime::new(
        year as i16,
        month as i8,
        day as i8,
        hour as i8,
        minute as i8,
        second as i8,
        fraction as i32 * NANOS_PER_100NS_UNIT,
    )
    .map_err(|_| EvalError::Overflow)
}
pub(super) fn extended_value(value: DateTime) -> Result<ExtendedDateTime, EvalError> {
    ExtendedDateTime::new(
        value.year().try_into().map_err(|_| EvalError::Overflow)?,
        value.month() as u8,
        value.day() as u8,
        value.hour() as u8,
        value.minute() as u8,
        value.second() as u8,
        (value.subsec_nanosecond() / NANOS_PER_100NS_UNIT) as u32,
    )
    .map_err(|_| EvalError::Overflow)
}

pub(super) fn calendar(value: &Value) -> Result<DateTime, EvalError> {
    match value {
        Value::DateTime(value) => legacy_calendar(*value),
        Value::DateTimeExtended(value) => extended_calendar(*value),
        _ => Err(EvalError::TypeMismatch),
    }
}
