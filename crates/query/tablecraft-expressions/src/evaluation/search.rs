//! Contextual substring operands retain explicit formatting/search policies.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        text::{TextFunction, TextResult, search},
    },
};
use tablecraft_core::{limits::Resource, values::Value};

impl Evaluator<'_> {
    pub(super) fn contextual_search(
        &mut self,
        function: TextFunction,
        arguments: &[Value],
    ) -> Result<TextResult<'static>, EvalError> {
        self.check(0)?;
        // The comparison mode is validated before start conversion, retaining
        // existing mode/NULL/error precedence without duplicating that parser.
        let mode = self.text_long_override(arguments.get(3))?;
        let search = search::prepare_mode(function, arguments, mode.as_ref())?;
        let start = match search.start() {
            Some(value) => match self.apply_conversion(Conversion::Long, value)? {
                Value::Int32(value) => value,
                _ => return Err(EvalError::TypeMismatch),
            },
            None => search.default_start(),
        };
        search.validate_start(start)?;
        let (haystack, needle) = search.operands();
        if matches!(haystack, Value::Null) || matches!(needle, Value::Null) {
            return Ok(TextResult::Null);
        }
        let converted_haystack = self.string_operand(haystack)?;
        let converted_needle = self.string_operand(needle)?;
        search.find_operands(
            start,
            converted_haystack.as_ref().unwrap_or(haystack),
            converted_needle.as_ref().unwrap_or(needle),
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
mod scalar_tests;
