//! Prepared date controls preserve validation order before bounded source conversion.
use super::{Evaluator, date_input::DateInput};
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        dates::{DateAdd, DateDiff, DatePart},
    },
};
use tablecraft_core::values::{ExpressionValue, Value};

impl Evaluator<'_> {
    pub(super) fn date_control_long(&mut self, value: &Value) -> Result<i32, EvalError> {
        if matches!(value, Value::Null) {
            return Err(EvalError::TypeMismatch);
        }
        match self.apply_conversion(Conversion::Long, value)? {
            Value::Int32(value) => Ok(value),
            _ => Err(EvalError::TypeMismatch),
        }
    }

    /// Add an interval after evaluating all supplied operands.
    pub fn apply_date_add(
        &mut self,
        interval: &Value,
        number: &Value,
        date: &Value,
    ) -> Result<Value, EvalError> {
        self.date_add_inputs(
            DateInput::Scalar(interval),
            DateInput::Scalar(number),
            DateInput::Scalar(date),
        )
    }

    /// Admit expression states and retained Strings at their original interval,
    /// count and date validation positions. Callers evaluate operands eagerly.
    pub fn apply_expression_date_add(
        &mut self,
        interval: &ExpressionValue,
        number: &ExpressionValue,
        date: &ExpressionValue,
    ) -> Result<Value, EvalError> {
        self.date_add_inputs(
            DateInput::Expression(interval),
            DateInput::Expression(number),
            DateInput::Expression(date),
        )
    }

    fn date_add_inputs(
        &mut self,
        interval: DateInput<'_>,
        number: DateInput<'_>,
        date: DateInput<'_>,
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        let interval = interval.materialize(self)?;
        let operation = DateAdd::prepare_with_count(&interval, || {
            let number = number.materialize(self)?;
            self.date_control_long(&number)
        })?;
        let date = date.materialize(self)?;
        let converted = self.converted_date_source(&date)?;
        operation.apply(converted.as_ref().unwrap_or(&date))
    }

    /// Extract a date part after interval and progressive week-option validation.
    pub fn apply_date_part(
        &mut self,
        interval: &Value,
        date: &Value,
        first_day: Option<&Value>,
        first_week: Option<&Value>,
    ) -> Result<Value, EvalError> {
        self.date_part_inputs(
            DateInput::Scalar(interval),
            DateInput::Scalar(date),
            [
                first_day.map(DateInput::Scalar),
                first_week.map(DateInput::Scalar),
            ],
        )
    }

    /// Publish retained inputs only as their week/source admission point is reached.
    pub fn apply_expression_date_part(
        &mut self,
        interval: &ExpressionValue,
        date: &ExpressionValue,
        first_day: Option<&ExpressionValue>,
        first_week: Option<&ExpressionValue>,
    ) -> Result<Value, EvalError> {
        self.date_part_inputs(
            DateInput::Expression(interval),
            DateInput::Expression(date),
            [
                first_day.map(DateInput::Expression),
                first_week.map(DateInput::Expression),
            ],
        )
    }

    fn date_part_inputs(
        &mut self,
        interval: DateInput<'_>,
        date: DateInput<'_>,
        options: [Option<DateInput<'_>>; 2],
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        let interval = interval.materialize(self)?;
        let context = self.week_context();
        let operation = DatePart::prepare_with_controls(
            &interval,
            |index| {
                let option = options[index]
                    .map(|value| value.materialize(self))
                    .transpose()?;
                option
                    .as_deref()
                    .map(|value| self.date_control_long(value))
                    .transpose()
            },
            context,
        )?;
        let date = date.materialize(self)?;
        let converted = self.converted_date_source(&date)?;
        operation.apply(converted.as_ref().unwrap_or(&date))
    }

    /// Count boundaries after interval/week validation and ordered source conversion.
    pub fn apply_date_diff(
        &mut self,
        interval: &Value,
        first: &Value,
        second: &Value,
        first_day: Option<&Value>,
        first_week: Option<&Value>,
    ) -> Result<Value, EvalError> {
        self.date_diff_inputs(
            DateInput::Scalar(interval),
            DateInput::Scalar(first),
            DateInput::Scalar(second),
            [
                first_day.map(DateInput::Scalar),
                first_week.map(DateInput::Scalar),
            ],
        )
    }

    /// Keep retained data until its original control/source admission position.
    pub fn apply_expression_date_diff(
        &mut self,
        interval: &ExpressionValue,
        first: &ExpressionValue,
        second: &ExpressionValue,
        first_day: Option<&ExpressionValue>,
        first_week: Option<&ExpressionValue>,
    ) -> Result<Value, EvalError> {
        self.date_diff_inputs(
            DateInput::Expression(interval),
            DateInput::Expression(first),
            DateInput::Expression(second),
            [
                first_day.map(DateInput::Expression),
                first_week.map(DateInput::Expression),
            ],
        )
    }

    fn date_diff_inputs(
        &mut self,
        interval: DateInput<'_>,
        first: DateInput<'_>,
        second: DateInput<'_>,
        options: [Option<DateInput<'_>>; 2],
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        let interval = interval.materialize(self)?;
        let context = self.week_context();
        let operation = DateDiff::prepare_with_controls(
            &interval,
            |index| {
                let option = options[index]
                    .map(|value| value.materialize(self))
                    .transpose()?;
                option
                    .as_deref()
                    .map(|value| self.date_control_long(value))
                    .transpose()
            },
            context,
        )?;
        let first = first.materialize(self)?;
        let converted_first = self.converted_date_source(&first)?;
        let second = second.materialize(self)?;
        let converted_second = self.converted_date_source(&second)?;
        operation.apply(
            converted_first.as_ref().unwrap_or(&first),
            converted_second.as_ref().unwrap_or(&second),
        )
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod expression_tests;
