//! Replacement count normalization keeps original source/match/replacement borrows.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        text::{TextResult, replacement},
    },
};
use tablecraft_core::values::Value;

impl Evaluator<'_> {
    pub(super) fn contextual_replacement<'a>(
        &mut self,
        arguments: &'a [Value],
    ) -> Result<TextResult<'a>, EvalError> {
        self.check(0)?;
        let start = arguments
            .get(3)
            .map(|value| self.replacement_position(value))
            .transpose()?;
        let count = arguments
            .get(4)
            .map(|value| self.replacement_position(value))
            .transpose()?;
        let recipe = replacement::prepare_positions(arguments, start, count)?;
        Ok(TextResult::Replacement(recipe))
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
