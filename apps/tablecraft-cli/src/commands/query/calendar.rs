//! Validated CLI calendar settings; no ambient machine preference lookup.
use tablecraft_expressions::functions::dates::TwoDigitYearWindow;

pub(super) fn parse_window(text: &str) -> Result<TwoDigitYearWindow, String> {
    text.parse::<u16>()
        .ok()
        .and_then(|last| TwoDigitYearWindow::new(last).ok())
        .ok_or_else(|| "expected the final year of a 100-year window in 199..=9999".to_owned())
}
