//! Calendar names retain ordinal, abbreviation and first-day admission order.
use super::{Evaluator, text_arguments};
use crate::{
    EvalError,
    functions::text::{
        TextContext, TextFunction, TextResult,
        calendar_names::{self, CalendarName},
    },
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
        let recipe = self.calendar_name_inputs(function, arguments.len(), |index| {
            text_arguments::scalar(&arguments[index], &empty_number)
        })?;
        self.materialize_text_result(TextResult::CalendarName(recipe))
    }

    pub(super) fn contextual_calendar_name<'a>(
        &mut self,
        function: TextFunction,
        arguments: &'a [Value],
    ) -> Result<TextResult<'a>, EvalError> {
        self.check(0)?;
        self.calendar_name_inputs(function, arguments.len(), |index| Ok(&arguments[index]))
            .map(TextResult::CalendarName)
    }

    fn calendar_name_inputs<'a>(
        &mut self,
        function: TextFunction,
        count: usize,
        argument: impl FnMut(usize) -> Result<&'a Value, EvalError>,
    ) -> Result<CalendarName, EvalError> {
        let context = TextContext {
            date_format: self.date_format,
            first_day_of_week: self.first_day_of_week,
            ..Default::default()
        };
        calendar_names::prepare_with_inputs(function, count, context, argument, |target, value| {
            if matches!(value, Value::Text(_)) {
                self.apply_conversion(target, value)
            } else {
                target.apply(value)
            }
        })
    }
}

#[cfg(test)]
mod tests;
