//! Week options normalize progressively at their owning function boundary.
use super::{Evaluator, text_arguments};
use crate::{
    EvalError,
    functions::{
        dates::WeekContext,
        text::{TextContext, TextFunction, TextResult, format},
    },
};
use tablecraft_core::values::{ExpressionValue, Value};

impl Evaluator<'_> {
    /// Retain eager states through the existing NULL/week/mask/source stages.
    /// Masked host Empty sources stay unqualified rather than choosing a value.
    pub fn apply_expression_format(
        &mut self,
        arguments: &[ExpressionValue],
    ) -> Result<Value, EvalError> {
        if !TextFunction::Format.accepts_arity(arguments.len()) {
            return Err(EvalError::UnsupportedExpression);
        }
        self.text_expression_limits(arguments)?;
        self.check(0)?;

        let empty_text = Value::Text(String::new());
        let empty_number = Value::Int32(0);
        let blank_mask = match arguments.get(1) {
            None | Some(ExpressionValue::Empty) => true,
            Some(ExpressionValue::Value(Value::Text(mask))) => mask.is_empty(),
            _ => false,
        };
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
        let result = format::prepare_inputs(
            arguments.len(),
            context,
            arguments
                .iter()
                .skip(2)
                .any(|value| matches!(value, ExpressionValue::Value(Value::Null))),
            |index| {
                if index == 0 && matches!(arguments[0], ExpressionValue::Empty) && !blank_mask {
                    return Err(EvalError::UnsupportedExpression);
                }
                let empty = if index < 2 {
                    &empty_text
                } else {
                    &empty_number
                };
                text_arguments::scalar(&arguments[index], empty)
            },
            |value| self.date_control_long(value),
        )?;
        self.materialize_text_result(result)
    }

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

#[cfg(test)]
mod expression_tests;
