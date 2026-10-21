//! Explicit Gregorian text syntax and caller-owned omitted-year context.
use super::super::TwoDigitYearWindow;
use crate::{EvalError, functions::conversions::text::NumberFormat};

/// Explicit Gregorian date/time input/output profile, without OS lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateTextFormat {
    /// Canonical numeric ISO dates (also for long output) and 24-hour time.
    Iso,
    /// Month/day/four-digit-year, English long names and 12-hour time with AM/PM.
    EnUs,
    /// Day/month/four-digit-year, German long names and 24-hour time.
    DeDe,
}
impl DateTextFormat {
    /// Resolve a supported ASCII-insensitive policy name without allocation.
    pub fn resolve(name: &str) -> Option<Self> {
        [
            ("iso", Self::Iso),
            ("en-us", Self::EnUs),
            ("de-de", Self::DeDe),
        ]
        .into_iter()
        .find_map(|(spelling, format)| name.eq_ignore_ascii_case(spelling).then_some(format))
    }
}

/// Validated full Gregorian year used when a date spelling omits its year.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReferenceYear(u16);
impl ReferenceYear {
    /// Earliest supported legacy DATE calendar year.
    pub const MIN: u16 = 100;
    /// Latest supported legacy DATE calendar year.
    pub const MAX: u16 = 9999;
    /// Choose an actual year in the legacy DATE envelope, without clock access.
    pub fn new(year: u16) -> Result<Self, EvalError> {
        if !(Self::MIN..=Self::MAX).contains(&year) {
            return Err(EvalError::Domain);
        }
        Ok(Self(year))
    }
    /// Read the full Gregorian year; this is not a two-digit-year spelling.
    pub const fn year(self) -> u16 {
        self.0
    }
}
/// Explicit date-input settings; prepared plans never capture these values.
#[derive(Clone, Copy)]
pub struct DateParseContext {
    /// Required for Text date/time input, independent of numeric fallback syntax.
    pub date_format: Option<DateTextFormat>,
    /// Two-digit year resolution, shared with DateSerial.
    pub year_window: TwoDigitYearWindow,
    /// Caller-issued current/reference year for omitted-year spellings.
    pub reference_year: Option<ReferenceYear>,
    /// Enables CDate's whole numeric Text fallback after date/time recognition.
    pub number_format: Option<NumberFormat>,
}
impl Default for DateParseContext {
    fn default() -> Self {
        Self {
            date_format: None,
            year_window: TwoDigitYearWindow::ACCESS_DEFAULT,
            reference_year: None,
            number_format: None,
        }
    }
}
