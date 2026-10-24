//! Contextual Replace operands share explicit string conversion and streaming output.
mod raw;

use super::{Evaluator, text_arguments, text_output::Output};
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        text::{TextFunction, replacement},
    },
};
use tablecraft_core::values::{ExpressionValue, Value};

impl Evaluator<'_> {
    pub(super) fn contextual_replacement(
        &mut self,
        arguments: &[Value],
    ) -> Result<Value, EvalError> {
        self.replacement_inputs(arguments.len(), |index| Ok(&arguments[index]))
    }

    /// Admit eager expression states in option order, then source/find/replacement.
    pub fn apply_expression_replacement(
        &mut self,
        arguments: &[ExpressionValue],
    ) -> Result<Value, EvalError> {
        if !TextFunction::Replace.accepts_arity(arguments.len()) {
            return Err(EvalError::UnsupportedExpression);
        }
        self.text_expression_limits(arguments)?;
        if arguments[..3]
            .iter()
            .any(|value| matches!(value, ExpressionValue::StringBytes(_)))
        {
            let value = self.apply_retained_replacement(arguments)?;
            return self.publish_byte_operand(value);
        }

        let empty_text = Value::Text(String::new());
        let empty_number = Value::Int32(0);
        self.replacement_inputs(arguments.len(), |index| {
            let empty = if index < 3 {
                &empty_text
            } else {
                &empty_number
            };
            text_arguments::scalar(&arguments[index], empty)
        })
    }

    fn replacement_inputs<'a>(
        &mut self,
        count: usize,
        mut argument: impl FnMut(usize) -> Result<&'a Value, EvalError>,
    ) -> Result<Value, EvalError> {
        let options = self.replacement_controls(count, &mut argument)?;

        let source = argument(0)?;
        let converted_source = self.string_operand(source)?;
        let find = argument(1)?;
        let converted_find = self.string_operand(find)?;
        let replacement = argument(2)?;
        let converted_replacement = self.string_operand(replacement)?;
        let recipe = options.with_operands(
            converted_source.as_ref().unwrap_or(source),
            converted_find.as_ref().unwrap_or(find),
            converted_replacement.as_ref().unwrap_or(replacement),
        )?;

        let mut output = Output {
            budget: self.budget,
            cancelled: self.cancelled,
            max_bytes: self.limits.max_text_bytes,
            text: String::new(),
        };
        recipe.write(self.collation, &mut output)?;
        Ok(Value::Text(output.text))
    }

    fn replacement_controls<'a>(
        &mut self,
        count: usize,
        mut argument: impl FnMut(usize) -> Result<&'a Value, EvalError>,
    ) -> Result<replacement::Options, EvalError> {
        self.check(0)?;
        let start = if count > 3 {
            Some(self.replacement_position(argument(3)?)?)
        } else {
            None
        };
        let substitutions = if count > 4 {
            Some(self.replacement_position(argument(4)?)?)
        } else {
            None
        };
        let mode = if count > 5 { Some(argument(5)?) } else { None };
        let normalized_mode = self.text_long_override(mode)?;
        replacement::prepare_values(start, substitutions, normalized_mode.as_ref().or(mode))
    }

    fn replacement_position(&mut self, value: &Value) -> Result<i32, EvalError> {
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

#[cfg(test)]
mod raw_tests;

#[cfg(test)]
mod provider_tests;
