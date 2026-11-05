//! Inert typed facts for host display choices; never an edit or query policy.
use jiff::civil::DateTime;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NativeFormat {
    #[default]
    Unspecified,
    GeneralDate,
    ShortDate,
    MediumDate,
    LongDate,
    ShortTime,
    MediumTime,
    LongTime,
    Currency,
    Euro,
    /// Exact stored `€ #,##0.00;-€ #,##0.00` display pattern.
    EuroPrefixTwoDecimals,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateRepresentation {
    LegacyBits(u64),
    Extended,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScalarPresentation {
    Currency {
        scaled: i64,
        native_format: NativeFormat,
    },
    DateTime {
        calendar: DateTime,
        representation: DateRepresentation,
        native_format: NativeFormat,
    },
}
