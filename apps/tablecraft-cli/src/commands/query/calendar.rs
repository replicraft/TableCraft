//! Validated CLI calendar settings; no ambient machine preference lookup.
use tablecraft_expressions::functions::dates::TwoDigitYearWindow;

pub(super) fn parse_window(text: &str) -> Result<TwoDigitYearWindow, String> {
    text.parse::<u16>()
        .ok()
        .and_then(|last| TwoDigitYearWindow::new(last).ok())
        .ok_or_else(|| "expected the final year of a 100-year window in 199..=9999".to_owned())
}

pub(super) fn parse_first_day(
    text: &str,
) -> Result<tablecraft_expressions::functions::dates::FirstDayOfWeek, String> {
    text.parse::<u8>()
        .ok()
        .and_then(|day| tablecraft_expressions::functions::dates::FirstDayOfWeek::new(day).ok())
        .ok_or_else(|| "expected a first day from 1 (Sunday) through 7 (Saturday)".into())
}
