//! Formatting recipes borrow their value while options normalize in validation order.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        text::{TextContext, TextFunction, TextResult, date_format, styled_number},
    },
};
use tablecraft_core::values::Value;

impl Evaluator<'_> {
    pub(super) fn contextual_numeric_format<'a>(
        &mut self,
        function: TextFunction,
        arguments: &'a [Value],
    ) -> Result<TextResult<'a>, EvalError> {
        self.check(0)?;
        let context = TextContext {
            number_format: self.number_format,
            ..Default::default()
        };
        // The owning parser validates each option before requesting the next.
        // Converting an entire replacement argument list would change errors
        // and require copying the borrowed value being formatted.
        let recipe = styled_number::prepare_with(function, arguments, context, |value| match self
            .apply_conversion(Conversion::Long, value)?
        {
            Value::Int32(value) => Ok(value),
            _ => Err(EvalError::TypeMismatch),
        })?;
        Ok(TextResult::StyledNumber(recipe))
    }

    pub(super) fn contextual_date_format<'a>(
        &mut self,
        arguments: &'a [Value],
    ) -> Result<TextResult<'a>, EvalError> {
        self.check(0)?;
        let mode = self.text_long_override(arguments.get(1))?;
        let recipe = date_format::prepare_mode(
            arguments,
            TextContext {
                number_format: self.number_format,
                date_format: self.date_format,
                year_window: self.year_window,
                reference_year: self.reference_year,
                ..Default::default()
            },
            mode.as_ref(),
        )?;
        Ok(TextResult::DateTimeFormat(recipe))
    }
}

#[cfg(test)]
mod tests;
