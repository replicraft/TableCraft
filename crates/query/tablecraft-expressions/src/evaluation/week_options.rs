//! Week options normalize progressively at their owning function boundary.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{
        dates::WeekContext,
        text::{TextContext, TextResult, format},
    },
};
use tablecraft_core::values::Value;

impl Evaluator<'_> {
    pub(super) fn week_context(&self) -> WeekContext {
        WeekContext {
            first_day: self.first_day_of_week,
            first_week: self.first_week_of_year,
        }
    }

    pub(super) fn contextual_week_format<'a>(
        &mut self,
        arguments: &'a [Value],
    ) -> Result<TextResult<'a>, EvalError> {
        self.check(0)?;
        let context = TextContext {
            collation: self.collation,
            character_code_page: self.character_code_page,
            number_format: self.number_format,
            date_format: self.date_format,
            year_window: self.year_window,
            reference_year: self.reference_year,
            first_day_of_week: self.first_day_of_week,
            first_week_of_year: self.first_week_of_year,
        };
        format::prepare_with(arguments, context, |value| self.date_control_long(value))
    }
}

#[cfg(test)]
mod tests;
