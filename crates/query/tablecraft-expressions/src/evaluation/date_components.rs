//! Date component sources use the same explicit, bounded conversion as CDate.
use super::Evaluator;
use crate::{EvalError, functions::dates::DateFunction};
use tablecraft_core::values::Value;

impl Evaluator<'_> {
    /// Extract a Gregorian component from an evaluated date, numeric or Text source.
    /// Existing dates retain their carrier/precision; NULL propagates without a
    /// conversion policy. Other inputs share CDate's explicit syntax and limits.
    pub fn apply_date_function(
        &mut self,
        function: DateFunction,
        value: &Value,
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        let converted = self.converted_date_source(value)?;
        function.apply(converted.as_ref().unwrap_or(value))
    }
}

#[cfg(test)]
mod tests;
