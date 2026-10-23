//! Character-code Text uses checked Long conversion, never a code-page guess.
use super::{Evaluator, text_arguments};
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        text::{TextContext, TextFunction, TextResult, characters},
    },
};
use tablecraft_core::values::{ExpressionValue, Value};

impl Evaluator<'_> {
    /// Admit character codes without implicitly converting Error data to its code.
    pub fn apply_expression_character(
        &mut self,
        function: TextFunction,
        value: &ExpressionValue,
    ) -> Result<Value, EvalError> {
        if !matches!(
            function,
            TextFunction::Chr | TextFunction::ChrString | TextFunction::ChrW
        ) {
            return Err(EvalError::UnsupportedExpression);
        }
        self.text_expression_limits(std::slice::from_ref(value))?;
        self.check(0)?;

        let empty_number = Value::Int32(0);
        let code = text_arguments::scalar(value, &empty_number)?;
        self.apply_text_function(function, std::slice::from_ref(code))
    }

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

#[cfg(test)]
mod expression_tests;
