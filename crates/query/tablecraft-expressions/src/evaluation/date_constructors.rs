//! Date/time constructor components share checked, metered Integer conversion.
use super::{Evaluator, date_input::DateInput};
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
        self.apply_constructor_arguments(function, arguments.len(), |index| {
            DateInput::Scalar(&arguments[index])
        })
    }

    /// Admit evaluated states at the original component conversion position.
    /// SQL callers must evaluate every supplied expression before entry.
    pub fn apply_expression_date_constructor(
        &mut self,
        function: DateConstructor,
        arguments: &[ExpressionValue],
    ) -> Result<Value, EvalError> {
        self.apply_constructor_arguments(function, arguments.len(), |index| {
            DateInput::Expression(&arguments[index])
        })
    }

    fn apply_constructor_arguments<'a>(
        &mut self,
        function: DateConstructor,
        count: usize,
        mut argument: impl FnMut(usize) -> DateInput<'a>,
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        function.apply_with_components(count, self.year_window, |index| {
            let value = argument(index).materialize(self)?;
            if matches!(value.as_ref(), Value::Null) {
                return Err(EvalError::InvalidNullConversion);
            }

            match self.apply_conversion(Conversion::Integer, &value)? {
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

#[cfg(test)]
mod retained_tests;
