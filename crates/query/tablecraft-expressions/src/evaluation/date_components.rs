//! Date component sources use the same explicit, bounded conversion as CDate.
use super::{Evaluator, date_conversions::expression_date_source};
use crate::{EvalError, functions::dates::DateFunction};
use tablecraft_core::values::{ExpressionValue, Value};

impl Evaluator<'_> {
    /// Extract a Gregorian component from an evaluated date, numeric or Text source.
    /// Existing dates retain their carrier/precision; NULL propagates without a
    /// conversion policy. Other inputs share CDate's explicit syntax and limits.
    pub fn apply_date_function(
        &mut self,
        function: DateFunction,
        value: &Value,
    ) -> Result<Value, EvalError> {
        self.date_function_input(function, || Ok(value))
    }

    /// Admit date states after the operation's entry cancellation/work check.
    pub fn apply_expression_date_function(
        &mut self,
        function: DateFunction,
        value: &ExpressionValue,
    ) -> Result<Value, EvalError> {
        let empty = Value::Int16(0);
        self.date_function_input(function, || expression_date_source(value, &empty))
    }

    fn date_function_input<'a>(
        &mut self,
        function: DateFunction,
        value: impl FnOnce() -> Result<&'a Value, EvalError>,
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        let value = value()?;
        let converted = self.converted_date_source(value)?;
        function.apply(converted.as_ref().unwrap_or(value))
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod expression_tests;
