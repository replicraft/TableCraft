//! SQL date-token interpretation is separate from runtime text-date policies.
use super::{DateTextFormat, ReferenceYear, text};
use crate::EvalError;
use tablecraft_core::values::LegacyDateTime;

mod date;

const MAX_QUERY_LITERAL_BYTES: usize = 64;

pub(crate) fn parse(
    value: &str,
    reference_year: Option<ReferenceYear>,
) -> Result<LegacyDateTime, EvalError> {
    if value.len() > MAX_QUERY_LITERAL_BYTES || !value.is_ascii() {
        return Err(EvalError::UnsupportedLiteral);
    }

    let calendar = text::parse_with(value, Some(DateTextFormat::EnUs), false, |value| {
        date::parse(value, reference_year)
    })
    .map_err(|_| EvalError::UnsupportedLiteral)?;

    LegacyDateTime::from_calendar(calendar).map_err(|_| EvalError::UnsupportedLiteral)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datetime::{parse_query_literal, parse_query_literal_with_reference_year};
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
            "32/13/2026",
            "1//2/2026",
            "Jan/Feb/2026",
            "1/2/10000",
            "Mar 7",
            "1/2",
            "01.02.2026",
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

    #[test]
    fn token_years_and_component_priorities_have_stable_compile_time_values() {
        for year in 0..=99 {
            let expected = if year < 30 { 2000 + year } else { 1900 + year };
            assert_eq!(
                parse_query_literal(&format!("1/2/{year:02}")).unwrap(),
                LegacyDateTime::from_calendar(date(expected, 1, 2).at(0, 0, 0, 0)).unwrap()
            );
        }

        for (text, expected) in [
            ("49/1/2", date(1949, 1, 2)),
            ("31/12/26", date(1931, 12, 26)),
            ("2024/2/29", date(2024, 2, 29)),
            ("13/1/2026", date(2026, 1, 13)),
            ("1 / 2 / 0029", date(2029, 1, 2)),
            ("0099-01-01", date(1999, 1, 1)),
            ("49 Jan 2", date(1949, 1, 2)),
            ("2 49 Jan", date(1949, 1, 2)),
            ("Jan 2, 49", date(1949, 1, 2)),
            ("Feb 2024", date(2024, 2, 1)),
            ("2024/2", date(2024, 2, 1)),
            ("Feb 99", date(1999, 2, 1)),
        ] {
            assert_eq!(
                parse_query_literal(text).unwrap(),
                LegacyDateTime::from_calendar(expected.at(0, 0, 0, 0)).unwrap(),
                "{text}"
            );
        }
    }

    #[test]
    fn explicit_default_year_controls_calendar_validity_and_two_component_priorities() {
        for (year, text, expected) in [
            (2024, "2/29", date(2024, 2, 29)),
            (2024, "29/2 6:05 PM", date(2024, 2, 29)),
            (2023, "Mar 7", date(2023, 3, 7)),
            (2023, "7 Mar", date(2023, 3, 7)),
            (2023, "Feb 29", date(2029, 2, 1)),
            (2024, "Feb 29", date(2024, 2, 29)),
            (2023, "Feb 49", date(1949, 2, 1)),
            (2023, "1/2/49", date(1949, 1, 2)),
            (100, "1/2", date(100, 1, 2)),
            (9999, "12/31", date(9999, 12, 31)),
        ] {
            let actual =
                parse_query_literal_with_reference_year(text, ReferenceYear::new(year).unwrap())
                    .unwrap();
            assert_eq!(
                actual.calendar_rounded_to_nanos().unwrap().date(),
                expected,
                "{text}"
            );
        }

        for text in ["2/29", "29/2", "32/13", "Mar/Feb", "1//2"] {
            assert_eq!(
                parse_query_literal_with_reference_year(text, ReferenceYear::new(2023).unwrap()),
                Err(EvalError::UnsupportedLiteral),
                "{text}"
            );
        }
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
