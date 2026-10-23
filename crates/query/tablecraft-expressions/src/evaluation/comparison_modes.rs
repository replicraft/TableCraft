//! Numeric Text comparison options retain their owning function's validation order.
use super::{Evaluator, text_arguments};
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        text::{TextFunction, TextResult, compare},
    },
};
use tablecraft_core::{
    limits::Resource,
    values::{ExpressionValue, Value},
};

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
        let null_operand =
            matches!(arguments[0], Value::Null) || matches!(arguments[1], Value::Null);
        self.comparison_inputs(arguments.len(), null_operand, |index| Ok(&arguments[index]))
    }

    /// Admit the comparison mode before the NULL gate and ordered string operands.
    pub fn apply_expression_comparison(
        &mut self,
        arguments: &[ExpressionValue],
    ) -> Result<Value, EvalError> {
        if !TextFunction::StrComp.accepts_arity(arguments.len()) {
            return Err(EvalError::UnsupportedExpression);
        }
        self.text_expression_limits(arguments)?;

        let null_operand = matches!(arguments[0], ExpressionValue::Value(Value::Null))
            || matches!(arguments[1], ExpressionValue::Value(Value::Null));
        let empty_text = Value::Text(String::new());
        let empty_number = Value::Int32(0);
        let result = self.comparison_inputs(arguments.len(), null_operand, |index| {
            let empty = if index < 2 {
                &empty_text
            } else {
                &empty_number
            };
            text_arguments::scalar(&arguments[index], empty)
        })?;

        match result {
            TextResult::Null => Ok(Value::Null),
            TextResult::Comparison(value) => Ok(Value::Int16(value)),
            _ => Err(EvalError::UnsupportedExpression),
        }
    }

    fn comparison_inputs<'a>(
        &mut self,
        count: usize,
        null_operand: bool,
        mut argument: impl FnMut(usize) -> Result<&'a Value, EvalError>,
    ) -> Result<TextResult<'static>, EvalError> {
        self.check(0)?;
        let option = if count > 2 { Some(argument(2)?) } else { None };
        let normalized = self.text_long_override(option)?;
        let mode = compare::mode(normalized.as_ref().or(option), None)?;

        if null_operand {
            return Ok(TextResult::Null);
        }

        let left = argument(0)?;
        let converted_left = self.string_operand(left)?;
        let right = argument(1)?;
        let converted_right = self.string_operand(right)?;

        compare::apply_operands(
            converted_left.as_ref().unwrap_or(left),
            converted_right.as_ref().unwrap_or(right),
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

#[cfg(test)]
mod expression_tests;
