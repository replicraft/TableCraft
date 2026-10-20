//! Budgeted materialization of borrowed text-function results.
use super::Evaluator;
use crate::{
    EvalError,
    functions::text::{self, TextFunction, TextResult},
};
use tablecraft_core::{limits::Resource, values::Value};

impl Evaluator<'_> {
    /// Apply a closed text built-in using this evaluator's shared scan/allocation
    /// limits. Operands must already be evaluated in written order.
    pub fn apply_text_function(
        &mut self,
        function: TextFunction,
        arguments: &[Value],
    ) -> Result<Value, EvalError> {
        for value in arguments {
            if matches!(value, Value::Text(text) if text.len() > self.limits.max_text_bytes) {
                return Err(EvalError::TextLimit);
            }
        }
        let result = text::apply_with_collation(function, arguments, self.collation, || {
            if self.cancelled.is_some_and(|cancelled| cancelled()) {
                return Err(EvalError::Cancelled);
            }
            self.budget.charge(Resource::Steps, 1)?;
            Ok(())
        })?;
        match result {
            TextResult::Null => Ok(Value::Null),
            TextResult::Comparison(value) => Ok(Value::Int16(value)),
            TextResult::Length(length) | TextResult::Position(length) => Ok(Value::Int32(length)),
            TextResult::Text(text) => self.text(text),
            TextResult::Replacement(replacement) => {
                let mut output = super::text_output::Output {
                    budget: self.budget,
                    cancelled: self.cancelled,
                    max_bytes: self.limits.max_text_bytes,
                    text: String::new(),
                };
                replacement.write(self.collation, &mut output)?;
                Ok(Value::Text(output.text))
            }
        }
    }
}
