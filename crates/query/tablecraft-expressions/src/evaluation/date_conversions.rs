//! Date conversion calls share the evaluator's work, limits and per-execution policy.
use super::{Evaluator, text_output::Output};
use crate::{
    EvalError,
    functions::dates::parsing::{DateConversion, DateParseContext},
};
use tablecraft_core::values::Value;
impl Evaluator<'_> {
    /// Convert or extract a date/time with explicit syntax/reference-year controls.
    pub fn apply_date_conversion(
        &mut self,
        function: DateConversion,
        value: &Value,
    ) -> Result<Value, EvalError> {
        if matches!(value,Value::Text(text) if text.len()>self.limits.max_text_bytes) {
            return Err(EvalError::TextLimit);
        }
        function.apply(
            value,
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
    fn date_scans_obey_shared_work_input_and_cancellation_limits() {
        let text = Value::Text("2026-10-20".into());
        let mut budget = ResourceBudget::new(100, 0, 3);
        assert!(
            matches!(Evaluator::new(&NoBindings,&mut budget,EvalLimits::default()).with_date_format(DateTextFormat::Iso).apply_date_conversion(DateConversion::DateValue,&text),Err(EvalError::Limit(error)) if error.resource==Resource::Steps)
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
            .apply_date_conversion(DateConversion::DateValue, &text),
            Err(EvalError::TextLimit)
        );
        let mut budget = ResourceBudget::new(100, 0, 100);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_cancellation(&|| true)
                .apply_date_conversion(DateConversion::CDate, &Value::Int16(0)),
            Err(EvalError::Cancelled)
        );
    }
}
