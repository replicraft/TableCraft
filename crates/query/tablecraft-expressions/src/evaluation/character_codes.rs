//! Character-code Text uses checked Long conversion, never a code-page guess.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        text::{TextContext, TextFunction, TextResult, characters},
    },
};
use tablecraft_core::values::Value;

impl Evaluator<'_> {
    pub(super) fn contextual_character(
        &mut self,
        function: TextFunction,
        value: &Value,
    ) -> Result<TextResult<'static>, EvalError> {
        self.check(0)?;
        let code = self.apply_conversion(Conversion::Long, value)?;
        characters::apply(
            function,
            &code,
            TextContext {
                character_code_page: self.character_code_page,
                ..Default::default()
            },
        )
    }
}

#[cfg(test)]
mod tests;
