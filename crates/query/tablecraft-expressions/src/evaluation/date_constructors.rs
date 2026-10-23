//! Date/time constructor components share checked, metered Integer conversion.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{conversions::Conversion, dates::DateConstructor},
};
use tablecraft_core::values::{ExpressionValue, Value};

impl Evaluator<'_> {
    /// Construct a DATE from three already evaluated components. Numeric Text
    /// uses the explicit number policy; raw DATE components use their serials.
    /// All components validate in order before relative calendar normalization.
    pub fn apply_date_constructor(
        &mut self,
        function: DateConstructor,
        arguments: &[Value],
    ) -> Result<Value, EvalError> {
        self.apply_constructor_arguments(function, arguments.len(), |index| Ok(&arguments[index]))
    }

    /// Admit evaluated states at the original component conversion position.
    /// SQL callers must evaluate every supplied expression before entry.
    pub fn apply_expression_date_constructor(
        &mut self,
        function: DateConstructor,
        arguments: &[ExpressionValue],
    ) -> Result<Value, EvalError> {
        let empty = Value::Int16(0);
        self.apply_constructor_arguments(function, arguments.len(), |index| {
            match &arguments[index] {
                ExpressionValue::Value(value) => Ok(value),
                ExpressionValue::Empty => Ok(&empty),
                ExpressionValue::Error(_) => Err(EvalError::TypeMismatch),
                ExpressionValue::Missing => Err(EvalError::NonScalarExpression),
            }
        })
    }

    fn apply_constructor_arguments<'a>(
        &mut self,
        function: DateConstructor,
        count: usize,
        mut argument: impl FnMut(usize) -> Result<&'a Value, EvalError>,
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        function.apply_with_input(count, &mut argument, self.year_window, |value| {
            match self.apply_conversion(Conversion::Integer, value)? {
                Value::Int16(value) => Ok(value),
                _ => Err(EvalError::TypeMismatch),
            }
        })
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod expression_tests;
