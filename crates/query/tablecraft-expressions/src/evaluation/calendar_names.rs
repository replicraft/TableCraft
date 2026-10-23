//! Calendar names retain ordinal, abbreviation and first-day admission order.
use super::{Evaluator, text_arguments};
use crate::{
    EvalError,
    functions::text::{TextContext, TextFunction, TextResult, calendar_names},
};
use tablecraft_core::values::{ExpressionValue, Value};

impl Evaluator<'_> {
    /// Admit already evaluated states without inventing an ambient calendar.
    pub fn apply_expression_calendar_name(
        &mut self,
        function: TextFunction,
        arguments: &[ExpressionValue],
    ) -> Result<Value, EvalError> {
        if !matches!(
            function,
            TextFunction::MonthName | TextFunction::WeekdayName
        ) || !function.accepts_arity(arguments.len())
        {
            return Err(EvalError::UnsupportedExpression);
        }
        self.text_expression_limits(arguments)?;
        self.check(0)?;

        let empty_number = Value::Int32(0);
        let recipe = calendar_names::prepare_inputs(
            function,
            arguments.len(),
            TextContext {
                date_format: self.date_format,
                first_day_of_week: self.first_day_of_week,
                ..Default::default()
            },
            |index| text_arguments::scalar(&arguments[index], &empty_number),
        )?;
        self.materialize_text_result(TextResult::CalendarName(recipe))
    }
}

#[cfg(test)]
mod tests;
