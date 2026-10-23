//! Explicit numbered diagnostic lookup, separate from runtime failure state.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{conversions::Conversion, error_messages::ErrorMessageCode},
};
use tablecraft_core::values::Value;

impl Evaluator<'_> {
    /// Convert the explicit number using Long rules, then copy the selected Text
    /// under the ordinary shared allocation and cancellation ceilings.
    pub fn apply_error_message(&mut self, number: &Value) -> Result<Value, EvalError> {
        self.check(1)?;
        let Value::Int32(number) = self.apply_conversion(Conversion::Long, number)? else {
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
