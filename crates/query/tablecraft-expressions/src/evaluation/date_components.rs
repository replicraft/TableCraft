//! Date component sources use the same explicit, bounded conversion as CDate.
use super::{Evaluator, date_input::DateInput};
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
        self.date_function_input(function, DateInput::Scalar(value))
    }

    /// Admit date states after the operation's entry cancellation/work check.
    pub fn apply_expression_date_function(
        &mut self,
        function: DateFunction,
        value: &ExpressionValue,
    ) -> Result<Value, EvalError> {
        self.date_function_input(function, DateInput::Expression(value))
    }

    fn date_function_input(
        &mut self,
        function: DateFunction,
        value: DateInput<'_>,
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        let value = value.materialize(self)?;
        let converted = self.converted_date_source(&value)?;
        function.apply(converted.as_ref().unwrap_or(&value))
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod expression_tests;

#[cfg(test)]
mod retained_tests;
