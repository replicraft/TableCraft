//! Explicit conversion calls share execution-local numeric Text policy and budget.
use super::{Evaluator, text_output::Output};
use crate::{EvalError, functions::conversions::Conversion};
use tablecraft_core::values::Value;
impl Evaluator<'_> {
    /// Convert a materialized value under this evaluator's policy and resource limits.
    pub fn apply_conversion(
        &mut self,
        conversion: Conversion,
        value: &Value,
    ) -> Result<Value, EvalError> {
        self.check(0)?;

        let Value::Text(text) = value else {
            return conversion.apply(value);
        };
        if text.len() > self.limits.max_text_bytes {
            return Err(EvalError::TextLimit);
        }
        let format = self.number_format.ok_or(EvalError::NumberFormatRequired)?;
        conversion.apply_text(
            text,
            format,
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
    fn conversion_text_charges_normalization_and_checks_cancellation_and_input_size() {
        let text = Value::Text("1234".into());
        let mut budget = ResourceBudget::new(3, 0, 100);
        assert!(
            matches!(Evaluator::new(&NoBindings,&mut budget,EvalLimits::default()).with_number_format(NumberFormat::Invariant)
            .apply_conversion(Conversion::Long,&text),Err(EvalError::Limit(error)) if error.resource==Resource::Bytes)
        );
        let mut budget = ResourceBudget::new(100, 0, 100);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(NumberFormat::Invariant)
                .with_cancellation(&|| true)
                .apply_conversion(Conversion::Long, &text),
            Err(EvalError::Cancelled)
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
            .apply_conversion(Conversion::Long, &text),
            Err(EvalError::TextLimit)
        );
    }
}
