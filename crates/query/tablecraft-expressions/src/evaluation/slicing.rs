//! Explicit scalar string conversion and checked positions share slice budgets.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        text::{TextFunction, TextResult, slicing},
    },
};
use tablecraft_core::{limits::Resource, values::Value};

impl Evaluator<'_> {
    pub(super) fn contextual_slice(
        &mut self,
        function: TextFunction,
        arguments: &[Value],
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        if matches!(arguments[0], Value::Null) {
            return Ok(Value::Null);
        }
        let converted = self.string_operand(&arguments[0])?;
        let Value::Text(source) = converted.as_ref().unwrap_or(&arguments[0]) else {
            return Err(EvalError::TypeMismatch);
        };
        let position = self.slice_position(&arguments[1])?;
        if function == TextFunction::Mid && position < 1 {
            return Err(EvalError::Domain);
        }
        let length = arguments
            .get(2)
            .map(|value| self.slice_position(value))
            .transpose()?;
        let result = slicing::apply(function, source, position, length, &mut || {
            if self.cancelled.is_some_and(|cancelled| cancelled()) {
                return Err(EvalError::Cancelled);
            }
            self.budget.charge(Resource::Steps, 1)?;
            Ok(())
        })?;
        let TextResult::Text(slice) = result else {
            return Err(EvalError::UnsupportedExpression);
        };
        self.text(slice)
    }

    fn slice_position(&mut self, value: &Value) -> Result<i32, EvalError> {
        if matches!(value, Value::Null) {
            return Err(EvalError::TypeMismatch);
        }
        match self.apply_conversion(Conversion::Long, value)? {
            Value::Int32(value) => Ok(value),
            _ => Err(EvalError::TypeMismatch),
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod scalar_tests;
