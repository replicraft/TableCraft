//! Explicit timezone-free ISO literal profile; no locale-dependent date guessing.
use crate::EvalError;
use jiff::civil::{Date, DateTime, Time};
use tablecraft_core::values::LegacyDateTime;

const DATE_BYTES: usize = 10;
const DATE_TIME_BYTES: usize = 19;
const MIN_FRACTIONAL_BYTES: usize = DATE_TIME_BYTES + 2;
const MAX_LITERAL_BYTES: usize = 29;
const SEPARATOR_OFFSETS: [(usize, u8); 6] = [
    (4, b'-'),
    (7, b'-'),
    (10, b' '),
    (13, b':'),
    (16, b':'),
    (19, b'.'),
];
const SECOND_TENS_OFFSET: usize = 17;

/// Parse YYYY-MM-DD or YYYY-MM-DD HH:MM:SS[.fraction], at most nine fractional
/// digits. Reject ambiguous local dates, offsets, timezones and invalid calendars.
/// This bounded profile does not claim every native Access date-literal spelling.
pub fn parse_literal(text: &str) -> Result<LegacyDateTime, EvalError> {
    let bytes = text.as_bytes();
    if !matches!(bytes.len(), DATE_BYTES | DATE_TIME_BYTES | MIN_FRACTIONAL_BYTES..=MAX_LITERAL_BYTES)
        || bytes.iter().enumerate().any(|(i, b)| {
            match SEPARATOR_OFFSETS.iter().find(|(offset, _)| *offset == i) {
                Some((_, expected)) => b != expected,
                None => !b.is_ascii_digit(),
            }
        })
        // Jiff's temporal parser normalizes leap seconds; this profile rejects them.
        || bytes.get(SECOND_TENS_OFFSET).is_some_and(|b| *b > b'5')
    {
        return Err(EvalError::UnsupportedLiteral);
    }
    let calendar = if bytes.len() == DATE_BYTES {
        text.parse::<Date>().map(|date| date.to_datetime(Time::MIN))
    } else {
        text.parse::<DateTime>()
    }
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
            "2026-01-01 23:59:60",
            "2026-01-01 00:00:00.",
            "2026-01-01 00:00:00.1234567890",
            "2026-01-01T00:00:00",
            "2026-01-01 00:00:00+00:00",
            "2026-01-01 00:00:00Z",
        ] {
            assert_eq!(parse_literal(text), Err(EvalError::UnsupportedLiteral));
        }
    }
}
