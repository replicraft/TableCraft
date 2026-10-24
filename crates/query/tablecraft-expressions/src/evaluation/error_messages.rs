//! Explicit numbered diagnostic lookup, separate from runtime failure state.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{conversions::Conversion, error_messages::ErrorMessageCode},
};
use tablecraft_core::values::{ExpressionValue, Value};

impl Evaluator<'_> {
    /// Convert the explicit number using Long rules, then copy the selected Text
    /// under the ordinary shared allocation and cancellation ceilings.
    pub fn apply_error_message(&mut self, number: &Value) -> Result<Value, EvalError> {
        self.error_message_input(|| Ok(number))
    }

    /// Admit already evaluated states at the numbered Long conversion boundary.
    /// Error data needs an explicit code conversion; host Empty uses numeric zero.
    pub fn apply_expression_error_message(
        &mut self,
        number: &ExpressionValue,
    ) -> Result<Value, EvalError> {
        let empty = Value::Int16(0);
        self.error_message_input(|| match number {
            ExpressionValue::Value(value) => Ok(value),
            ExpressionValue::Empty => Ok(&empty),
            ExpressionValue::Error(_) => Err(EvalError::TypeMismatch),
            ExpressionValue::StringBytes(_) => Err(EvalError::NonScalarExpression),
            ExpressionValue::Missing => Err(EvalError::NonScalarExpression),
        })
    }

    fn error_message_input<'a>(
        &mut self,
        number: impl FnOnce() -> Result<&'a Value, EvalError>,
    ) -> Result<Value, EvalError> {
        self.check(1)?;
        let Value::Int32(number) = self.apply_conversion(Conversion::Long, number()?)? else {
            return Err(EvalError::TypeMismatch);
        };
        let code = ErrorMessageCode::new(number)?;
        if code.number() == 0 {
            return self.text("");
        }
        let messages = self.error_messages.ok_or(EvalError::ErrorCatalogRequired)?;
        let message = messages.message(code)?;
        self.check(1)?;
        self.text(message)
    }
}

#[cfg(test)]
mod tests;
