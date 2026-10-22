//! Repetition count normalization retains String's literal character argument.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        text::{TextFunction, TextResult, construction},
    },
};
use tablecraft_core::values::Value;

impl Evaluator<'_> {
    pub(super) fn contextual_repeat(
        &mut self,
        function: TextFunction,
        arguments: &[Value],
    ) -> Result<TextResult<'static>, EvalError> {
        self.check(0)?;
        let Value::Int32(count) = self.apply_conversion(Conversion::Long, &arguments[0])? else {
            return Err(EvalError::TypeMismatch);
        };
        construction::repeat(function, count, arguments.get(1), self.character_code_page)
    }
}

#[cfg(test)]
mod tests;
