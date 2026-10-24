//! SQL literal dates have U.S. order and complete years, independent of host locale.
use super::{DateParseContext, DateTextFormat, date, text};
use crate::EvalError;
use jiff::civil::Date;
use tablecraft_core::values::LegacyDateTime;

const MAX_QUERY_LITERAL_BYTES: usize = 64;
const COMPLETE_YEAR_DIGITS: usize = 4;
const MIN_COMPLETE_YEAR_DIGITS: usize = "100".len();
const ISO_DATE_BYTES: usize = "YYYY-MM-DD".len();
const ISO_DAY_SEPARATOR: usize = "YYYY-MM".len();

pub(crate) fn parse(value: &str) -> Result<LegacyDateTime, EvalError> {
    if value.len() > MAX_QUERY_LITERAL_BYTES || !value.is_ascii() {
        return Err(EvalError::UnsupportedLiteral);
    }

    let context = DateParseContext {
        date_format: Some(DateTextFormat::EnUs),
        ..Default::default()
    };

    let calendar = text::parse_with(value, context.date_format, false, |part| {
        let iso = part.as_bytes().get(COMPLETE_YEAR_DIGITS) == Some(&b'-')
            && part.as_bytes()[..COMPLETE_YEAR_DIGITS]
                .iter()
                .all(u8::is_ascii_digit);

        if iso {
            if part.len() != ISO_DATE_BYTES || part.as_bytes().get(ISO_DAY_SEPARATOR) != Some(&b'-')
            {
                return Err(EvalError::UnsupportedLiteral);
            }
            return part
                .parse::<Date>()
                .map_err(|_| EvalError::UnsupportedLiteral);
        }

        let year = part
            .split(|ch: char| ch.is_ascii_whitespace() || matches!(ch, '/' | '-' | ','))
            .rfind(|token| !token.is_empty())
            .ok_or(EvalError::UnsupportedLiteral)?;

        if !(MIN_COMPLETE_YEAR_DIGITS..=COMPLETE_YEAR_DIGITS).contains(&year.len())
            || !year.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(EvalError::UnsupportedLiteral);
        }

        date::parse(part, context)
    })
    .map_err(|_| EvalError::UnsupportedLiteral)?;

    LegacyDateTime::from_calendar(calendar).map_err(|_| EvalError::UnsupportedLiteral)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datetime::parse_query_literal;
    use jiff::civil::date;
    use proptest::prelude::*;

    #[test]
    fn sql_dates_use_us_order_and_complete_english_dates_without_host_settings() {
        for (text, expected) in [
            ("1/2/2026", date(2026, 1, 2).at(0, 0, 0, 0)),
            ("02/29/2000", date(2000, 2, 29).at(0, 0, 0, 0)),
            ("Mar-7-2017", date(2017, 3, 7).at(0, 0, 0, 0)),
            ("7-Mar-2017", date(2017, 3, 7).at(0, 0, 0, 0)),
            (
                "September 30, 2024 2:24PM",
                date(2024, 9, 30).at(14, 24, 0, 0),
            ),
            ("12/29/1899 06:00:00", date(1899, 12, 29).at(6, 0, 0, 0)),
            ("12/31/9999 23:59:59", date(9999, 12, 31).at(23, 59, 59, 0)),
            ("1/1/0100", date(100, 1, 1).at(0, 0, 0, 0)),
            ("1/1/100", date(100, 1, 1).at(0, 0, 0, 0)),
            ("12:00 AM", date(1899, 12, 30).at(0, 0, 0, 0)),
            ("12:00 PM", date(1899, 12, 30).at(12, 0, 0, 0)),
            (
                "23:59:59.125",
                date(1899, 12, 30).at(23, 59, 59, 125_000_000),
            ),
            ("2024-02-29 6:05 PM", date(2024, 2, 29).at(18, 5, 0, 0)),
        ] {
            assert_eq!(
                parse_query_literal(text).unwrap(),
                LegacyDateTime::from_calendar(expected).unwrap(),
                "{text}"
            );
        }
    }

    #[test]
    fn invalid_or_context_dependent_literals_do_not_use_regional_or_clock_guesses() {
        for text in [
            "",
            "2/29/1900",
            "13/1/2026",
            "31/12/2026",
            "1/2/26",
            "Mar-7-17",
            "Mar 7",
            "1/2",
            "1/1/0099",
            "01.02.2026",
            "2024-2-29",
            "2024-02-29T12:00:00Z",
            "2024-02-29 12:00:00+01:00",
            "12/31/2024 24:00",
            "12/31/2024 23:59:60",
            "0:30 PM",
            "13:30 PM",
            "11:30:00.1234567890",
            "1.23.45",
            "Mär-7-2024",
        ] {
            assert_eq!(
                parse_query_literal(text),
                Err(EvalError::UnsupportedLiteral),
                "{text}"
            );
        }
        assert_eq!(
            parse_query_literal(&" ".repeat(65)),
            Err(EvalError::UnsupportedLiteral)
        );
    }

    proptest! {
        #![proptest_config(crate::test_support::config())]

        #[test]
        fn complete_us_dates_match_independent_civil_construction(
            year in 100i16..=9999,
            month in 1i8..=12,
            day in 1i8..=28,
            hour in 0i8..24,
            minute in 0i8..60,
            second in 0i8..60,
        ) {
            let text = format!("{month}/{day}/{year:04} {hour}:{minute:02}:{second:02}");
            let expected = date(year, month, day).at(hour, minute, second, 0);

            prop_assert_eq!(
                parse_query_literal(&text).unwrap(),
                LegacyDateTime::from_calendar(expected).unwrap()
            );
        }
    }
}
