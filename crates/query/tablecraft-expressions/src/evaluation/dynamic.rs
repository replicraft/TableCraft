//! Closed expression-string evaluation shares the enclosing evaluator's state.
mod syntax;

use super::Evaluator;
use crate::{EvalError, functions::text::TextFunction};
use tablecraft_core::values::{ExpressionValue, Value};

impl Evaluator<'_> {
    /// Evaluate a closed Text expression under this evaluator's existing policies,
    /// clock/random providers, allocation/work budget and cancellation callback.
    /// Numeric input uses explicit CStr policy before parsing. Dynamic names,
    /// database reads and object/UDF activation are outside this profile.
    pub fn apply_eval_expression(&mut self, value: &Value) -> Result<ExpressionValue, EvalError> {
        self.dynamic_expression(value, 0)
    }

    pub(super) fn dynamic_expression(
        &mut self,
        value: &Value,
        depth: usize,
    ) -> Result<ExpressionValue, EvalError> {
        self.check(depth)?;
        let converted;
        let text = match value {
            Value::Text(text) => text,
            Value::Null => return Err(EvalError::InvalidNullConversion),
            Value::DateTime(_) | Value::DateTimeExtended(_) | Value::Guid(_) | Value::Binary(_) => {
                return Err(EvalError::TypeMismatch);
            }
            _ => {
                converted =
                    self.apply_text_function(TextFunction::CStr, std::slice::from_ref(value))?;
                let Value::Text(text) = &converted else {
                    return Err(EvalError::TypeMismatch);
                };
                text
            }
        };
        let parsed = self.parse_dynamic(text, depth)?;
        // Validate the entire closed shape before any clock/random/binding call.
        self.validate_dynamic(&parsed, depth + 1)?;
        self.expression_operand(&parsed, depth + 1)
    }
}

#[cfg(test)]
mod tests;
