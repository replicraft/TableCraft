//! Contextual scalar-to-Text conversion for &, separate from numeric/text +.
mod bytes;

use super::Evaluator;
use crate::{EvalError, functions::text::TextFunction, text};
use tablecraft_core::values::Value;

impl Evaluator<'_> {
    pub(super) fn concatenate(&mut self, left: &Value, right: &Value) -> Result<Value, EvalError> {
        if matches!((left, right), (Value::Null, Value::Null)) {
            return Ok(Value::Null);
        }
        // Operands have already evaluated in written order. Retain existing
        // Text/NULL without copying; other scalar conversions use the same
        // explicit policies, cancellation and allocation accounting as CStr.
        let converted_left = self.concatenation_operand(left)?;
        let converted_right = self.concatenation_operand(right)?;
        let left = converted_left.as_ref().unwrap_or(left);
        let right = converted_right.as_ref().unwrap_or(right);
        let length = |value: &Value| match value {
            Value::Null => Ok(0),
            Value::Text(text) => Ok(text.len()),
            _ => Err(EvalError::TypeMismatch),
        };
        let length = length(left)?
            .checked_add(length(right)?)
            .ok_or(EvalError::TextLimit)?;
        self.charge_text(length)?;
        text::concatenate(left, right, self.limits.max_text_bytes)
    }

    fn concatenation_operand(&mut self, value: &Value) -> Result<Option<Value>, EvalError> {
        if matches!(value, Value::Text(_) | Value::Null) {
            Ok(None)
        } else {
            self.apply_text_function(TextFunction::CStr, std::slice::from_ref(value))
                .map(Some)
        }
    }
}

#[cfg(test)]
mod tests;
