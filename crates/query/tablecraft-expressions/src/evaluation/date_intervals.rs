//! Prepared date controls preserve validation order before bounded source conversion.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        dates::{DateAdd, DateDiff, DatePart},
    },
};
use tablecraft_core::values::Value;

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

    /// Add an interval to evaluated date/numeric/Text operands using explicit policies.
    /// Interval then count validation precede date conversion and NULL propagation.
    pub fn apply_date_add(
        &mut self,
        interval: &Value,
        number: &Value,
        date: &Value,
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        let operation = DateAdd::prepare(interval, number, |value| self.date_control_long(value))?;
        let converted = self.converted_date_source(date)?;
        operation.apply(converted.as_ref().unwrap_or(date))
    }

    /// Extract a date part after validating the interval and progressive week options.
    /// Non-date sources share the bounded CDate service; existing carriers stay intact.
    pub fn apply_date_part(
        &mut self,
        interval: &Value,
        date: &Value,
        first_day: Option<&Value>,
        first_week: Option<&Value>,
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        let operation = DatePart::prepare(
            interval,
            first_day,
            first_week,
            self.week_context(),
            |value| self.date_control_long(value),
        )?;
        let converted = self.converted_date_source(date)?;
        operation.apply(converted.as_ref().unwrap_or(date))
    }

    /// Count boundaries after interval/week validation and source conversion in order.
    /// Both sources validate before NULL propagation; no source payload is cloned.
    pub fn apply_date_diff(
        &mut self,
        interval: &Value,
        first: &Value,
        second: &Value,
        first_day: Option<&Value>,
        first_week: Option<&Value>,
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        let operation = DateDiff::prepare(
            interval,
            first_day,
            first_week,
            self.week_context(),
            |value| self.date_control_long(value),
        )?;
        let converted_first = self.converted_date_source(first)?;
        let converted_second = self.converted_date_source(second)?;
        operation.apply(
            converted_first.as_ref().unwrap_or(first),
            converted_second.as_ref().unwrap_or(second),
        )
    }
}

#[cfg(test)]
mod tests;
