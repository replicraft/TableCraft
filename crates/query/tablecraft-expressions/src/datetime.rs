//! Explicit timezone-free ISO literal profile; no locale-dependent date guessing.
use crate::EvalError;
use tablecraft_core::values::LegacyDateTime;
use time::{
    Date, PrimitiveDateTime, format_description::BorrowedFormatItem, macros::format_description,
};

const MAX_LITERAL_BYTES: usize = 29;
const DATE: &[BorrowedFormatItem<'static>] = format_description!("[year]-[month]-[day]");
const DATE_TIME: &[BorrowedFormatItem<'static>] =
    format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
const FRACTIONAL_DATE_TIME: &[BorrowedFormatItem<'static>] =
    format_description!("[year]-[month]-[day] [hour]:[minute]:[second].[subsecond]");

/// Parse YYYY-MM-DD or YYYY-MM-DD HH:MM:SS[.fraction], at most nine fractional
/// digits. Reject ambiguous local dates, offsets, timezones and invalid calendars.
/// This bounded profile does not claim every native Access date-literal spelling.
pub fn parse_literal(text: &str) -> Result<LegacyDateTime, EvalError> {
    if text.len() > MAX_LITERAL_BYTES {
        return Err(EvalError::UnsupportedLiteral);
    }
    let calendar = Date::parse(text, DATE)
        .map(Date::midnight)
        .or_else(|_| PrimitiveDateTime::parse(text, DATE_TIME))
        .or_else(|_| PrimitiveDateTime::parse(text, FRACTIONAL_DATE_TIME))
        .map_err(|_| EvalError::UnsupportedLiteral)?;
    LegacyDateTime::from_calendar(calendar).map_err(|_| EvalError::UnsupportedLiteral)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn literal_profile_is_explicit_and_negative_dates_keep_positive_time_of_day() {
        for (text, days) in [
            ("1899-12-30", 0.0),
            ("1899-12-29 06:00:00", -1.25),
            ("1900-01-01 06:00:00.000", 2.25),
        ] {
            assert_eq!(parse_literal(text).unwrap().ole_days(), days);
        }
        for text in [
            "01/02/2026",
            "2026-02-30",
            "0099-01-01",
            "2026-01-01T00:00:00Z",
            "2026-01-01 24:00:00",
        ] {
            assert_eq!(parse_literal(text), Err(EvalError::UnsupportedLiteral));
        }
    }
}
