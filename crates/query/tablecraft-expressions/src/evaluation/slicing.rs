//! Contextual numeric Text positions borrow their source without cloning payloads.
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
    pub(super) fn contextual_slice<'a>(
        &mut self,
        function: TextFunction,
        arguments: &'a [Value],
    ) -> Result<TextResult<'a>, EvalError> {
        self.check(0)?;
        let Value::Text(source) = &arguments[0] else {
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
        slicing::apply(function, source, position, length, &mut || {
            if self.cancelled.is_some_and(|cancelled| cancelled()) {
                return Err(EvalError::Cancelled);
            }
            self.budget.charge(Resource::Steps, 1)?;
            Ok(())
        })
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
