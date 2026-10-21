//! Bounded materialized Weekday evaluation under request-local policies.
use super::{Evaluator, text_output::Output};
use crate::{
    EvalError,
    functions::dates::{self, WeekContext, parsing::DateParseContext},
};
use tablecraft_core::values::Value;
impl Evaluator<'_> {
    /// Evaluate one date and optional already evaluated first-day setting.
    /// Text scans and private normalization share the evaluator's limits/budget.
    pub fn apply_weekday(
        &mut self,
        value: &Value,
        first_day: Option<&Value>,
    ) -> Result<Value, EvalError> {
        if matches!(value, Value::Text(text) if text.len() > self.limits.max_text_bytes) {
            return Err(EvalError::TextLimit);
        }
        dates::weekday(
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
