//! Explicit expression-state conversion, without changing scalar/storage admission.
use super::{Evaluator, text_output::Output};
use crate::{
    EvalError,
    functions::text::{TextFunction, cstr},
};
use tablecraft_core::values::{ExpressionValue, Value};

impl Evaluator<'_> {
    /// Borrow Text operands; materialize other scalar spellings under the same
    /// CStr policies and cumulative budget as explicit conversion.
    pub(super) fn string_operand(&mut self, value: &Value) -> Result<Option<Value>, EvalError> {
        if matches!(value, Value::Text(_)) {
            Ok(None)
        } else {
            self.apply_text_function(TextFunction::CStr, std::slice::from_ref(value))
                .map(Some)
        }
    }

    /// Convert expression data deliberately. Error is formatted as a standard
    /// code, Empty as empty Text; operational failures are never caught here.
    /// Ordinary scalar values retain the existing CStr policies and validation.
    pub fn apply_expression_cstr(&mut self, value: &ExpressionValue) -> Result<Value, EvalError> {
        self.check(0)?;
        match value {
            ExpressionValue::Value(value) => {
                self.apply_text_function(TextFunction::CStr, std::slice::from_ref(value))
            }
            ExpressionValue::Empty => self.text(""),
            ExpressionValue::Missing => Err(EvalError::TypeMismatch),
            ExpressionValue::Error(code) => {
                let mut output = Output {
                    budget: self.budget,
                    cancelled: self.cancelled,
                    max_bytes: self.limits.max_text_bytes,
                    text: String::new(),
                };
                cstr::write_standard_error(*code, &mut output)?;
                Ok(Value::Text(output.text))
            }
        }
    }
}

#[cfg(test)]
mod tests;
