//! Execution policies for text functions, independent of native file metadata.
use super::code_page::CharacterCodePage;
use crate::comparison::Collation;

/// Caller-selected text semantics; no ambient locale or machine setting is read.
#[derive(Clone, Copy)]
pub struct TextContext<'a> {
    /// Numeric Text input and CStr/named numeric output profile.
    pub number_format: Option<crate::functions::conversions::text::NumberFormat>,
    /// Gregorian date/time input/output profile, independent of numeric formatting.
    pub date_format: Option<super::cstr::DateTextFormat>,
    /// Two-digit year interpretation for explicit date Text input.
    pub year_window: crate::functions::dates::TwoDigitYearWindow,
    /// Caller-issued first day for WeekdayName system/default numbering.
    pub first_day_of_week: Option<crate::functions::dates::FirstDayOfWeek>,
    /// Caller-issued year when a date Text spelling omits its year.
    pub reference_year: Option<crate::functions::dates::parsing::ReferenceYear>,
    /// Distinct database/text comparison and search provider, if supplied.
    pub collation: Option<&'a dyn Collation>,
    /// Explicit numeric-byte character decoding policy; absent keeps ASCII-only.
    pub character_code_page: Option<CharacterCodePage>,
}

impl Default for TextContext<'_> {
    fn default() -> Self {
        Self {
            number_format: None,
            date_format: None,
            year_window: crate::functions::dates::TwoDigitYearWindow::ACCESS_DEFAULT,
            reference_year: None,
            first_day_of_week: None,
            collation: None,
            character_code_page: None,
        }
    }
}
