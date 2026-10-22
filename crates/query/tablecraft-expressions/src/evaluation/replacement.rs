//! Contextual Replace operands share explicit string conversion and streaming output.
use super::{Evaluator, text_output::Output};
use crate::{
    EvalError,
    functions::{conversions::Conversion, text::replacement},
};
use tablecraft_core::values::Value;

impl Evaluator<'_> {
    pub(super) fn contextual_replacement(
        &mut self,
        arguments: &[Value],
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        let start = arguments
            .get(3)
            .map(|value| self.replacement_position(value))
            .transpose()?;
        let count = arguments
            .get(4)
            .map(|value| self.replacement_position(value))
            .transpose()?;
        let mode = self.text_long_override(arguments.get(5))?;
        let options = replacement::prepare_options(arguments, start, count, mode.as_ref())?;
        let source = self.string_operand(&arguments[0])?;
        let find = self.string_operand(&arguments[1])?;
        let replacement = self.string_operand(&arguments[2])?;
        let recipe = options.with_operands(
            source.as_ref().unwrap_or(&arguments[0]),
            find.as_ref().unwrap_or(&arguments[1]),
            replacement.as_ref().unwrap_or(&arguments[2]),
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
