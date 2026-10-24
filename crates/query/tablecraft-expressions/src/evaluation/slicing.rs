//! Explicit scalar string conversion and checked positions share slice budgets.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        text::{TextFunction, TextResult, slicing},
    },
};
use tablecraft_core::{
    limits::Resource,
    values::{ExpressionValue, Value},
};

impl Evaluator<'_> {
    pub(super) fn contextual_slice(
        &mut self,
        function: TextFunction,
        arguments: &[Value],
    ) -> Result<Value, EvalError> {
        self.slice_inputs(function, arguments.len(), |index| Ok(&arguments[index]))
    }

    /// Admit eager expression operands in source, position and length order.
    /// Empty string sources and numeric controls have separate coercion targets.
    pub fn apply_expression_slice(
        &mut self,
        function: TextFunction,
        arguments: &[ExpressionValue],
    ) -> Result<Value, EvalError> {
        if !matches!(
            function,
            TextFunction::Left | TextFunction::Right | TextFunction::Mid
        ) || !function.accepts_arity(arguments.len())
        {
            return Err(EvalError::UnsupportedExpression);
        }
        for value in arguments {
            if matches!(value, ExpressionValue::Value(Value::Text(text)) if text.len() > self.limits.max_text_bytes)
            {
                return Err(EvalError::TextLimit);
            }
        }
        let empty_text = Value::Text(String::new());
        let empty_number = Value::Int32(0);
        self.slice_inputs(function, arguments.len(), |index| match &arguments[index] {
            ExpressionValue::Value(value) => Ok(value),
            ExpressionValue::Empty => Ok(if index == 0 {
                &empty_text
            } else {
                &empty_number
            }),
            ExpressionValue::Error(_) => Err(EvalError::TypeMismatch),
            ExpressionValue::Missing => Err(EvalError::NonScalarExpression),
        })
    }

    fn slice_inputs<'a>(
        &mut self,
        function: TextFunction,
        count: usize,
        mut argument: impl FnMut(usize) -> Result<&'a Value, EvalError>,
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        let source = argument(0)?;
        if matches!(source, Value::Null) {
            return Ok(Value::Null);
        }
        let converted = self.string_operand(source)?;
        let Value::Text(source) = converted.as_ref().unwrap_or(source) else {
            return Err(EvalError::TypeMismatch);
        };
        let position = self.slice_position(argument(1)?)?;
        if function == TextFunction::Mid && position < 1 {
            return Err(EvalError::Domain);
        }
        let length = if count > 2 {
            Some(self.slice_position(argument(2)?)?)
        } else {
            None
        };
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

    pub(super) fn slice_position(&mut self, value: &Value) -> Result<i32, EvalError> {
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

#[cfg(test)]
mod expression_tests;
