//! Typed scalar validation failures and expression-state categories.

use std::fmt;

/// An expression-only state that must not silently become a stored NULL.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpressionState {
    /// An uninitialized language value.
    Empty,
    /// An omitted optional argument.
    Missing,
    /// An expression error value.
    Error,
}

/// Closed scalar validation failures without the rejected data in diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueError {
    /// Decimal coefficient exceeds the 28-digit Access field envelope.
    DecimalOutOfRange,
    /// Decimal scale is greater than 28.
    DecimalScaleOutOfRange,
    /// Requested rescaling would discard nonzero digits.
    DecimalPrecisionLoss,
    /// Currency arithmetic exceeded its signed 64-bit scaled range.
    CurrencyOverflow,
    /// Non-finite or out-of-calendar-range Automation DATE.
    InvalidLegacyDateTime,
    /// Invalid Gregorian date, time or fractional second.
    InvalidExtendedDateTime,
    /// An expression-only state cannot be converted to a stored scalar.
    ExpressionNotStorable(ExpressionState),
}

impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::DecimalOutOfRange => "decimal coefficient exceeds 28 digits",
            Self::DecimalScaleOutOfRange => "decimal scale must be between 0 and 28",
            Self::DecimalPrecisionLoss => "decimal rescaling would lose precision",
            Self::CurrencyOverflow => "currency arithmetic overflow",
            Self::InvalidLegacyDateTime => "invalid legacy date/time",
            Self::InvalidExtendedDateTime => "invalid extended date/time",
            Self::ExpressionNotStorable(_) => "expression-only state cannot be stored",
        };
        f.write_str(message)
    }
}

impl std::error::Error for ValueError {}
