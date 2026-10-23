//! Partition converts controls in order after its explicit NULL precheck.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        text::{TextResult, partition},
    },
};
use tablecraft_core::values::Value;

impl Evaluator<'_> {
    pub(super) fn contextual_partition<'a>(
        &mut self,
        arguments: &'a [Value],
    ) -> Result<TextResult<'a>, EvalError> {
        self.check(0)?;
        partition::prepare_with(arguments, |value| {
            let converted = if matches!(value, Value::Text(_)) {
                self.apply_conversion(Conversion::Long, value)?
            } else {
                Conversion::Long.apply(value)?
            };
            match converted {
                Value::Int32(value) => Ok(value),
                _ => Err(EvalError::TypeMismatch),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        evaluation::{EvalLimits, NoBindings},
        functions::{conversions::text::NumberFormat, text::TextFunction},
    };
    use std::cell::Cell;
    use tablecraft_core::limits::ResourceBudget;

    #[test]
    fn partition_text_controls_retain_rounding_null_and_conversion_order() {
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
        let input = ["2,5", "0", "9", "2"].map(|text| Value::Text(text.into()));
        assert_eq!(
            evaluator.apply_text_function(TextFunction::Partition, &input),
            Err(EvalError::NumberFormatRequired)
        );
        assert_eq!(
            evaluator.apply_text_function(
                TextFunction::Partition,
                &[
                    Value::Text("bad".into()),
                    Value::Null,
                    Value::Int32(9),
                    Value::Int32(2)
                ]
            ),
            Ok(Value::Null)
        );
        let mut evaluator = evaluator.with_number_format(NumberFormat::DeDe);
        assert_eq!(
            evaluator.apply_text_function(TextFunction::Partition, &input),
            Ok(Value::Text(" 2: 3".into()))
        );
        // All conversions precede range validation, in written order.
        for (values, expected) in [
            (["1", "-1", "9", "bad"], EvalError::TypeMismatch),
            (["2147483648", "-1", "9", "bad"], EvalError::Overflow),
            (["1", "-1", "9", "2"], EvalError::Domain),
        ] {
            let values = values.map(|text| Value::Text(text.into()));
            assert_eq!(
                evaluator.apply_text_function(TextFunction::Partition, &values),
                Err(expected)
            );
        }
    }

    #[test]
    fn partition_text_conversion_and_output_share_limits_and_cancellation() {
        let values = ["2.5", "0", "9", "2"].map(|text| Value::Text(text.into()));
        let checks = Cell::new(0);
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .with_cancellation(&|| {
                checks.set(checks.get() + 1);
                false
            })
            .apply_text_function(TextFunction::Partition, &values)
            .unwrap();
        for stop in 1..=checks.get() {
            let calls = Cell::new(0);
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            assert_eq!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .with_number_format(NumberFormat::EnUs)
                    .with_cancellation(&|| {
                        calls.set(calls.get() + 1);
                        calls.get() >= stop
                    })
                    .apply_text_function(TextFunction::Partition, &values),
                Err(EvalError::Cancelled)
            );
        }
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert_eq!(
            Evaluator::new(
                &NoBindings,
                &mut budget,
                EvalLimits {
                    max_text_bytes: 2,
                    ..Default::default()
                }
            )
            .with_number_format(NumberFormat::EnUs)
            .apply_text_function(TextFunction::Partition, &values),
            Err(EvalError::TextLimit)
        );
    }
}
