//! Bounded materialized Weekday evaluation under request-local policies.
use super::{Evaluator, date_conversions::expression_date_source, text_output::Output};
use crate::{
    EvalError,
    functions::dates::{self, WeekContext, parsing::DateParseContext},
};
use tablecraft_core::values::{ExpressionValue, Value};
impl Evaluator<'_> {
    /// Evaluate one date and optional already evaluated first-day setting.
    /// Text scans and private normalization share the evaluator's limits/budget.
    pub fn apply_weekday(
        &mut self,
        value: &Value,
        first_day: Option<&Value>,
    ) -> Result<Value, EvalError> {
        self.weekday_inputs(Some(value), || Ok(value), || Ok(first_day))
    }

    /// Retain source Error data until after first-day conversion and validation.
    /// Host Empty dates use DATE zero; Empty first-day means explicit system zero.
    pub fn apply_expression_weekday(
        &mut self,
        value: &ExpressionValue,
        first_day: Option<&ExpressionValue>,
    ) -> Result<Value, EvalError> {
        let raw_value = match value {
            ExpressionValue::Value(value) => Some(value),
            _ => None,
        };
        let empty = Value::Int16(0);
        self.weekday_inputs(
            raw_value,
            || expression_date_source(value, &empty),
            || {
                first_day
                    .map(|value| expression_date_source(value, &empty))
                    .transpose()
            },
        )
    }

    fn weekday_inputs<'a>(
        &mut self,
        raw_value: Option<&Value>,
        value: impl FnOnce() -> Result<&'a Value, EvalError>,
        first_day: impl FnOnce() -> Result<Option<&'a Value>, EvalError>,
    ) -> Result<Value, EvalError> {
        if matches!(raw_value, Some(Value::Text(text)) if text.len() > self.limits.max_text_bytes) {
            return Err(EvalError::TextLimit);
        }
        self.check(0)?;
        let first_day = first_day()?;
        let normalized = self.text_long_override(first_day)?;
        let first_day = normalized.as_ref().or(first_day);
        dates::weekday_with_input(
            value,
            first_day,
            WeekContext {
                first_day: self.first_day_of_week,
                first_week: self.first_week_of_year,
            },
            DateParseContext {
                date_format: self.date_format,
                year_window: self.year_window,
                reference_year: self.reference_year,
                number_format: self.number_format,
            },
            &mut Output {
                budget: self.budget,
                cancelled: self.cancelled,
                max_bytes: self.limits.max_text_bytes,
                text: String::new(),
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        evaluation::{EvalLimits, NoBindings},
        functions::dates::parsing::DateTextFormat,
    };
    use tablecraft_core::limits::{Resource, ResourceBudget};
    #[test]
    fn weekday_text_conversion_obeys_work_input_size_and_cancellation_limits() {
        let value = Value::Text("2024-02-29".into());
        let mut budget = ResourceBudget::new(100, 0, 3);
        assert!(
            matches!(Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).with_date_format(DateTextFormat::Iso).apply_weekday(&value, None), Err(EvalError::Limit(error)) if error.resource == Resource::Steps)
        );
        let mut budget = ResourceBudget::new(100, 0, 100);
        assert_eq!(
            Evaluator::new(
                &NoBindings,
                &mut budget,
                EvalLimits {
                    max_text_bytes: 3,
                    ..Default::default()
                }
            )
            .with_date_format(DateTextFormat::Iso)
            .apply_weekday(&value, None),
            Err(EvalError::TextLimit)
        );
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_cancellation(&|| true)
                .apply_weekday(&Value::Null, None),
            Err(EvalError::Cancelled)
        );
    }
}
