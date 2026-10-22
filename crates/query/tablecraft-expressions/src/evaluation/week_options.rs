//! Week options normalize progressively at their owning function boundary.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        dates::{self, WeekContext},
        text::{TextContext, TextResult, format},
    },
};
use tablecraft_core::values::Value;

impl Evaluator<'_> {
    fn week_context(&self) -> WeekContext {
        WeekContext {
            first_day: self.first_day_of_week,
            first_week: self.first_week_of_year,
        }
    }

    fn week_long(&mut self, value: &Value) -> Result<i32, EvalError> {
        if matches!(value, Value::Null) {
            return Err(EvalError::TypeMismatch);
        }
        match self.apply_conversion(Conversion::Long, value)? {
            Value::Int32(value) => Ok(value),
            _ => Err(EvalError::TypeMismatch),
        }
    }

    /// Apply DatePart to evaluated operands with metered numeric Text options.
    /// Interval validation precedes progressive option conversion and date access.
    pub fn apply_date_part(
        &mut self,
        interval: &Value,
        date: &Value,
        first_day: Option<&Value>,
        first_week: Option<&Value>,
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        dates::date_part_with_converter(
            interval,
            date,
            first_day,
            first_week,
            self.week_context(),
            |value| self.week_long(value),
        )
    }

    /// Apply DateDiff to evaluated operands with the same explicit week policies.
    /// Written operand expressions must all have been evaluated before this call.
    pub fn apply_date_diff(
        &mut self,
        interval: &Value,
        first: &Value,
        second: &Value,
        first_day: Option<&Value>,
        first_week: Option<&Value>,
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        dates::date_diff_with_converter(
            interval,
            first,
            second,
            first_day,
            first_week,
            self.week_context(),
            |value| self.week_long(value),
        )
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
        format::prepare_with(arguments, context, |value| self.week_long(value))
    }
}

#[cfg(test)]
mod tests;
