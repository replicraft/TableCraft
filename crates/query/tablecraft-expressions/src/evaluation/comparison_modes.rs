//! Numeric Text comparison options retain their owning function's validation order.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        text::{TextResult, compare},
    },
};
use tablecraft_core::{limits::Resource, values::Value};

impl Evaluator<'_> {
    pub(super) fn text_long_override(
        &mut self,
        value: Option<&Value>,
    ) -> Result<Option<Value>, EvalError> {
        match value {
            Some(value @ Value::Text(_)) => {
                self.check(0)?;
                self.apply_conversion(Conversion::Long, value).map(Some)
            }
            _ => Ok(None),
        }
    }

    pub(super) fn contextual_comparison(
        &mut self,
        arguments: &[Value],
    ) -> Result<TextResult<'static>, EvalError> {
        self.check(0)?;
        let normalized = self.text_long_override(arguments.get(2))?;
        let mode = compare::mode(normalized.as_ref().or_else(|| arguments.get(2)), None)?;
        if matches!(arguments[0], Value::Null) || matches!(arguments[1], Value::Null) {
            return Ok(TextResult::Null);
        }
        let left = self.string_operand(&arguments[0])?;
        let right = self.string_operand(&arguments[1])?;
        compare::apply_operands(
            left.as_ref().unwrap_or(&arguments[0]),
            right.as_ref().unwrap_or(&arguments[1]),
            mode,
            self.collation,
            &mut || {
                if self.cancelled.is_some_and(|cancelled| cancelled()) {
                    return Err(EvalError::Cancelled);
                }
                self.budget.charge(Resource::Steps, 1)?;
                Ok(())
            },
        )
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod operand_tests;
