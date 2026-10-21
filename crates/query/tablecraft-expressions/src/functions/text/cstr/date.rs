//! Explicit Gregorian short-date/long-time spellings, without ambient locale access.
use super::super::{date_format, output::TextOutput};
pub use crate::functions::dates::parsing::DateTextFormat;
use crate::{EvalError, functions::dates::scalar_calendar};
use tablecraft_core::values::Value;

/// Borrowed CStr date/time output, emitted only under a charged host sink.
#[derive(Debug, PartialEq, Eq)]
pub struct DateText<'a> {
    value: &'a Value,
    format: DateTextFormat,
}
impl<'a> DateText<'a> {
    pub(super) fn new(value: &'a Value, format: DateTextFormat) -> Self {
        Self { value, format }
    }
    /// Format to seconds using checked Gregorian nearest-second half-expand rounding.
    /// The Automation epoch date emits time only; other midnight values emit date only.
    pub fn write(&self, output: &mut impl TextOutput) -> Result<(), EvalError> {
        output.step()?;
        date_format::write_general(scalar_calendar(self.value)?, self.format, output)
    }
}
