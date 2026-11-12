//! Explicit immutable expression policies, independent of rendered cell preferences.
use tablecraft_expressions::functions::{
    conversions::text::NumberFormat, dates::parsing::DateTextFormat,
};
use thiserror::Error;

/// Named conversion/formatting policies captured by one worker environment.
/// Missing policies remain missing; no OS, UI language or collation is inferred.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QueryExpressionFormats {
    /// Exact numeric Text conversion and output policy.
    pub number: Option<NumberFormat>,
    /// Exact Gregorian date/time Text conversion and output policy.
    pub date: Option<DateTextFormat>,
}

/// Unsupported explicit configuration name, without retaining its input payload.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum QueryExpressionFormatsError {
    /// The number profile is not recognized by the actual expression owner.
    #[error("unsupported query number-format profile")]
    Number,
    /// The date profile is not recognized by the actual expression owner.
    #[error("unsupported query date-format profile")]
    Date,
}

impl QueryExpressionFormats {
    /// Resolve actual expression-owner profile names for application startup.
    /// Absent names keep typed missing-format evaluation behavior.
    ///
    /// # Errors
    /// Returns a typed failure for an unsupported explicit profile; never falls
    /// back to another policy or consults display/system locale preferences.
    pub fn from_profile_names(
        number: Option<&str>,
        date: Option<&str>,
    ) -> Result<Self, QueryExpressionFormatsError> {
        let number = number
            .map(|name| NumberFormat::resolve(name).ok_or(QueryExpressionFormatsError::Number))
            .transpose()?;
        let date = date
            .map(|name| DateTextFormat::resolve(name).ok_or(QueryExpressionFormatsError::Date))
            .transpose()?;

        Ok(Self { number, date })
    }
}
