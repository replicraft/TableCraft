//! Prepared date controls preserve validation order before bounded source conversion.
use super::Evaluator;
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
        self.date_add_inputs(|| Ok(interval), || Ok(number), || Ok(date))
    }

    /// Admit expression states at interval, count and date validation positions.
    pub fn apply_expression_date_add(
        &mut self,
        interval: &ExpressionValue,
        number: &ExpressionValue,
        date: &ExpressionValue,
    ) -> Result<Value, EvalError> {
        let empty = Value::Int16(0);
        self.date_add_inputs(
            || date_interval_scalar(interval, &empty),
            || date_interval_scalar(number, &empty),
            || date_interval_scalar(date, &empty),
        )
    }

    fn date_add_inputs<'a>(
        &mut self,
        interval: impl FnOnce() -> Result<&'a Value, EvalError>,
        number: impl FnOnce() -> Result<&'a Value, EvalError>,
        date: impl FnOnce() -> Result<&'a Value, EvalError>,
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        let operation = DateAdd::prepare_with_number(interval()?, number, |value| {
            self.date_control_long(value)
        })?;
        let date = date()?;
        let converted = self.converted_date_source(date)?;
        operation.apply(converted.as_ref().unwrap_or(date))
    }

    /// Extract a date part after interval and progressive week-option validation.
    pub fn apply_date_part(
        &mut self,
        interval: &Value,
        date: &Value,
        first_day: Option<&Value>,
        first_week: Option<&Value>,
    ) -> Result<Value, EvalError> {
        let options = [first_day, first_week];
        self.date_part_inputs(|| Ok(interval), || Ok(date), |index| Ok(options[index]))
    }

    /// Apply already evaluated states without changing week/source conversion order.
    pub fn apply_expression_date_part(
        &mut self,
        interval: &ExpressionValue,
        date: &ExpressionValue,
        first_day: Option<&ExpressionValue>,
        first_week: Option<&ExpressionValue>,
    ) -> Result<Value, EvalError> {
        let empty = Value::Int16(0);
        let options = [first_day, first_week];
        self.date_part_inputs(
            || date_interval_scalar(interval, &empty),
            || date_interval_scalar(date, &empty),
            |index| {
                options[index]
                    .map(|value| date_interval_scalar(value, &empty))
                    .transpose()
            },
        )
    }

    fn date_part_inputs<'a>(
        &mut self,
        interval: impl FnOnce() -> Result<&'a Value, EvalError>,
        date: impl FnOnce() -> Result<&'a Value, EvalError>,
        option: impl FnMut(usize) -> Result<Option<&'a Value>, EvalError>,
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        let operation =
            DatePart::prepare_with_options(interval()?, option, self.week_context(), |value| {
                self.date_control_long(value)
            })?;
        let date = date()?;
        let converted = self.converted_date_source(date)?;
        operation.apply(converted.as_ref().unwrap_or(date))
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
        let options = [first_day, first_week];
        self.date_diff_inputs(
            || Ok(interval),
            || Ok(first),
            || Ok(second),
            |index| Ok(options[index]),
        )
    }

    /// Keep Error/Empty data until its original control/source admission position.
    pub fn apply_expression_date_diff(
        &mut self,
        interval: &ExpressionValue,
        first: &ExpressionValue,
        second: &ExpressionValue,
        first_day: Option<&ExpressionValue>,
        first_week: Option<&ExpressionValue>,
    ) -> Result<Value, EvalError> {
        let empty = Value::Int16(0);
        let options = [first_day, first_week];
        self.date_diff_inputs(
            || date_interval_scalar(interval, &empty),
            || date_interval_scalar(first, &empty),
            || date_interval_scalar(second, &empty),
            |index| {
                options[index]
                    .map(|value| date_interval_scalar(value, &empty))
                    .transpose()
            },
        )
    }

    fn date_diff_inputs<'a>(
        &mut self,
        interval: impl FnOnce() -> Result<&'a Value, EvalError>,
        first: impl FnOnce() -> Result<&'a Value, EvalError>,
        second: impl FnOnce() -> Result<&'a Value, EvalError>,
        option: impl FnMut(usize) -> Result<Option<&'a Value>, EvalError>,
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        let operation =
            DateDiff::prepare_with_options(interval()?, option, self.week_context(), |value| {
                self.date_control_long(value)
            })?;
        let first = first()?;
        let converted_first = self.converted_date_source(first)?;
        let second = second()?;
        let converted_second = self.converted_date_source(second)?;
        operation.apply(
            converted_first.as_ref().unwrap_or(first),
            converted_second.as_ref().unwrap_or(second),
        )
    }
}

fn date_interval_scalar<'a>(
    value: &'a ExpressionValue,
    empty: &'a Value,
) -> Result<&'a Value, EvalError> {
    match value {
        ExpressionValue::Value(value) => Ok(value),
        ExpressionValue::Empty => Ok(empty),
        ExpressionValue::Error(_) => Err(EvalError::TypeMismatch),
        ExpressionValue::Missing => Err(EvalError::NonScalarExpression),
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod expression_tests;
