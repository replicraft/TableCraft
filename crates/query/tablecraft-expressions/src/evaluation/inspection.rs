//! Materialized inspection under the evaluator's shared limits and cancellation.
use super::{Evaluator, text_output::Output};
use crate::{EvalError, functions::inspection::InspectionFunction};
use tablecraft_core::values::Value;
impl Evaluator<'_> {
    /// Inspect one value; failed scans never become successful FALSE results.
    pub fn apply_inspection(
        &mut self,
        function: InspectionFunction,
        value: &Value,
    ) -> Result<Value, EvalError> {
        if matches!(value,Value::Text(text) if text.len()>self.limits.max_text_bytes) {
            return Err(EvalError::TextLimit);
        }
        function.apply(
            value,
            self.number_format,
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
        functions::conversions::text::NumberFormat,
    };
    use tablecraft_core::limits::{Resource, ResourceBudget};
    #[test]
    fn inspection_preserves_allocation_work_input_and_cancellation_failures() {
        let call = InspectionFunction::IsNumeric;
        let value = Value::Text("1234".into());
        let mut budget = ResourceBudget::new(3, 0, 100);
        assert!(
            matches!(Evaluator::new(&NoBindings,&mut budget,EvalLimits::default()).with_number_format(NumberFormat::Invariant).apply_inspection(call,&value),Err(EvalError::Limit(error)) if error.resource==Resource::Bytes)
        );
        let mut budget = ResourceBudget::new(100, 0, 3);
        assert!(
            matches!(Evaluator::new(&NoBindings,&mut budget,EvalLimits::default()).with_number_format(NumberFormat::Invariant).apply_inspection(call,&value),Err(EvalError::Limit(error)) if error.resource==Resource::Steps)
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
            .with_number_format(NumberFormat::Invariant)
            .apply_inspection(call, &value),
            Err(EvalError::TextLimit)
        );
        let mut budget = ResourceBudget::new(100, 0, 100);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_cancellation(&|| true)
                .apply_inspection(call, &Value::Int16(0)),
            Err(EvalError::Cancelled)
        );
    }
}
