//! Budgeted materialization of borrowed text-function results.
use super::Evaluator;
use crate::{
    EvalError,
    functions::text::{self, TextFunction, TextResult},
};
use tablecraft_core::{limits::Resource, values::Value};

impl Evaluator<'_> {
    /// Apply a closed text built-in using this evaluator's shared scan/allocation
    /// limits. Operands must already be evaluated in written order.
    pub fn apply_text_function(
        &mut self,
        function: TextFunction,
        arguments: &[Value],
    ) -> Result<Value, EvalError> {
        for value in arguments {
            if matches!(value, Value::Text(text) if text.len() > self.limits.max_text_bytes) {
                return Err(EvalError::TextLimit);
            }
        }
        let result = text::apply_with_context(
            function,
            arguments,
            text::TextContext {
                collation: self.collation,
                character_code_page: self.character_code_page,
            },
            || {
                if self.cancelled.is_some_and(|cancelled| cancelled()) {
                    return Err(EvalError::Cancelled);
                }
                self.budget.charge(Resource::Steps, 1)?;
                Ok(())
            },
        )?;
        match result {
            TextResult::Null => Ok(Value::Null),
            TextResult::Comparison(value) | TextResult::CharacterCode(value) => {
                Ok(Value::Int16(value))
            }
            TextResult::Character(character) => {
                let mut buffer = [0; char::MAX.len_utf8()];
                self.text(character.encode_utf8(&mut buffer))
            }
            TextResult::Length(length) | TextResult::Position(length) => Ok(Value::Int32(length)),
            TextResult::Text(text) => self.text(text),
            TextResult::NumericFormat(format) => {
                let mut output = super::text_output::Output {
                    budget: self.budget,
                    cancelled: self.cancelled,
                    max_bytes: self.limits.max_text_bytes,
                    text: String::new(),
                };
                format.write(&mut output)?;
                Ok(Value::Text(output.text))
            }
            TextResult::NumericPrefix(prefix) => {
                let mut output = super::text_output::Output {
                    budget: self.budget,
                    cancelled: self.cancelled,
                    max_bytes: self.limits.max_text_bytes,
                    text: String::new(),
                };
                Ok(Value::Double(prefix.evaluate(&mut output)?))
            }
            TextResult::Construction(construction) => {
                let mut output = super::text_output::Output {
                    budget: self.budget,
                    cancelled: self.cancelled,
                    max_bytes: self.limits.max_text_bytes,
                    text: String::new(),
                };
                construction.write(&mut output)?;
                Ok(Value::Text(output.text))
            }
            TextResult::Replacement(replacement) => {
                let mut output = super::text_output::Output {
                    budget: self.budget,
                    cancelled: self.cancelled,
                    max_bytes: self.limits.max_text_bytes,
                    text: String::new(),
                };
                replacement.write(self.collation, &mut output)?;
                Ok(Value::Text(output.text))
            }
        }
    }
}
