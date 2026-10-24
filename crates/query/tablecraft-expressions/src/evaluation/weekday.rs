//! Bounded materialized Weekday evaluation under request-local policies.
use super::{Evaluator, date_input::DateInput, text_output::Output};
use crate::{
    EvalError,
    functions::dates::{Weekday, parsing::DateParseContext},
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
        self.weekday_inputs(DateInput::Scalar(value), first_day.map(DateInput::Scalar))
    }

    /// Retain source Error data until after first-day conversion and validation.
    /// Host Empty dates use DATE zero; Empty first-day means explicit system zero.
    pub fn apply_expression_weekday(
        &mut self,
        value: &ExpressionValue,
        first_day: Option<&ExpressionValue>,
    ) -> Result<Value, EvalError> {
        self.weekday_inputs(
            DateInput::Expression(value),
            first_day.map(DateInput::Expression),
        )
    }

    fn weekday_inputs(
        &mut self,
        value: DateInput<'_>,
        first_day: Option<DateInput<'_>>,
    ) -> Result<Value, EvalError> {
        if matches!(value.scalar(), Some(Value::Text(text)) if text.len() > self.limits.max_text_bytes)
        {
            return Err(EvalError::TextLimit);
        }
        self.check(0)?;
        let first_day = first_day.map(|value| value.materialize(self)).transpose()?;
        let normalized = self.text_long_override(first_day.as_deref())?;
        let first_day = normalized.as_ref().or(first_day.as_deref());
        // Preserve the separate operation-entry checkpoint before validating
        // week controls, then admit the source only after those controls pass.
        self.check(0)?;
        let operation = Weekday::prepare(first_day, self.week_context())?;
        let value = value.materialize(self)?;
        operation.apply(
            &value,
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
