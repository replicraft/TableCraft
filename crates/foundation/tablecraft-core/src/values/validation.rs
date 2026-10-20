//! Typed scalar validation failures and expression-state categories.

use thiserror::Error;

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
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ValueError {
    /// Decimal coefficient magnitude exceeds 96 bits.
    #[error("decimal coefficient magnitude exceeds 96 bits")]
    DecimalOutOfRange,
    /// Decimal scale is greater than 28.
    #[error("decimal scale must be between 0 and 28")]
    DecimalScaleOutOfRange,
    /// Requested rescaling would discard nonzero digits.
    #[error("decimal rescaling would lose precision")]
    DecimalPrecisionLoss,
    /// Currency arithmetic exceeded its signed 64-bit scaled range.
    #[error("currency arithmetic overflow")]
    CurrencyOverflow,
    /// Non-finite or out-of-calendar-range Automation DATE.
    #[error("invalid legacy date/time")]
    InvalidLegacyDateTime,
    /// Invalid Gregorian date, time or fractional second.
    #[error("invalid extended date/time")]
    InvalidExtendedDateTime,
    /// An expression-only state cannot be converted to a stored scalar.
    #[error("expression-only state cannot be stored")]
    ExpressionNotStorable(ExpressionState),
}
