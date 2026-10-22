//! Date/time constructor components share checked, metered Integer conversion.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{conversions::Conversion, dates::DateConstructor},
};
use tablecraft_core::values::Value;

impl Evaluator<'_> {
    /// Construct a DATE from three already evaluated components. Numeric Text
    /// uses the explicit number policy; raw DATE components use their serials.
    /// All components validate in order before relative calendar normalization.
    pub fn apply_date_constructor(
        &mut self,
        function: DateConstructor,
        arguments: &[Value],
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        function.apply_with_converter(arguments, self.year_window, |value| {
            match self.apply_conversion(Conversion::Integer, value)? {
                Value::Int16(value) => Ok(value),
                _ => Err(EvalError::TypeMismatch),
            }
        })
    }
}

#[cfg(test)]
mod tests;
