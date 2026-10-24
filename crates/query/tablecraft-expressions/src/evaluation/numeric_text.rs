//! Explicit numeric Text conversion before the existing bounded numeric renderer.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{conversions::Conversion, text::TextFunction},
};
use tablecraft_core::values::{Double, ExpressionValue, Value};

impl Evaluator<'_> {
    /// Str consumes standard Error data deliberately; ordinary errors still propagate.
    /// Empty follows its numeric-zero coercion; Missing remains non-scalar.
    /// Scalars use the same bounded renderer and explicit number/date policies.
    pub fn apply_expression_str(&mut self, value: &ExpressionValue) -> Result<Value, EvalError> {
        self.check(0)?;
        match value {
            ExpressionValue::Error(_) => self.apply_expression_cstr(value),
            ExpressionValue::Value(value) => {
                self.apply_text_function(TextFunction::Str, std::slice::from_ref(value))
            }
            ExpressionValue::Empty => self
                .apply_text_function(TextFunction::Str, &[Value::Double(Double::from_float(0.0))]),
            ExpressionValue::StringBytes(_) => Err(EvalError::NonScalarExpression),
            ExpressionValue::Missing => Err(EvalError::NonScalarExpression),
        }
    }

    /// Hex/Oct consume Empty as zero and reject implicit Error-code conversion.
    /// Other scalar inputs retain the shared Long/LongLong and Text policies.
    pub fn apply_expression_radix(
        &mut self,
        function: TextFunction,
        value: &ExpressionValue,
    ) -> Result<Value, EvalError> {
        if !matches!(function, TextFunction::Hex | TextFunction::Oct) {
            return Err(EvalError::UnsupportedExpression);
        }
        self.check(0)?;
        match value {
            ExpressionValue::Value(value) => {
                self.apply_text_function(function, std::slice::from_ref(value))
            }
            ExpressionValue::Empty => self.apply_text_function(function, &[Value::Int32(0)]),
            ExpressionValue::Error(_) => Err(EvalError::TypeMismatch),
            ExpressionValue::StringBytes(_) => Err(EvalError::NonScalarExpression),
            ExpressionValue::Missing => Err(EvalError::NonScalarExpression),
        }
    }

    pub(super) fn contextual_numeric_text(
        &mut self,
        function: TextFunction,
        value: &Value,
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        let target = match function {
            TextFunction::Hex | TextFunction::Oct => Conversion::Long,
            TextFunction::Str => Conversion::Double,
            _ => return Err(EvalError::UnsupportedExpression),
        };
        let value = self.apply_conversion(target, value)?;
        self.apply_text_function(function, &[value])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        evaluation::{EvalLimits, NoBindings},
        functions::conversions::text::NumberFormat,
    };
    use std::cell::Cell;
    use tablecraft_core::limits::ResourceBudget;

    #[test]
    fn numeric_text_parses_under_explicit_policy_and_retains_target_boundaries() {
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
        assert_eq!(
            evaluator.apply_text_function(TextFunction::Hex, &[Value::Text("2.5".into())]),
            Err(EvalError::NumberFormatRequired)
        );
        let mut evaluator = evaluator.with_number_format(NumberFormat::DeDe);
        for (function, input, expected) in [
            (TextFunction::Hex, "2,5", "2"),
            (TextFunction::Hex, "3,5", "4"),
            (TextFunction::Oct, "-32768", "37777700000"),
            (TextFunction::Hex, "2147483647", "7FFFFFFF"),
            (TextFunction::Str, "2,5", " 2.5"),
            (TextFunction::Str, "-2,5", "-2.5"),
        ] {
            assert_eq!(
                evaluator.apply_text_function(function, &[Value::Text(input.into())]),
                Ok(Value::Text(expected.into()))
            );
        }
        for function in [TextFunction::Hex, TextFunction::Oct] {
            assert_eq!(
                evaluator.apply_text_function(function, &[Value::Text("2147483648".into())]),
                Err(EvalError::Overflow)
            );
            // Only an actual LongLong bypasses Long conversion.
            assert!(
                evaluator
                    .apply_text_function(function, &[Value::Int64(2147483648)])
                    .is_ok()
            );
        }
        assert_eq!(
            evaluator.apply_text_function(TextFunction::Str, &[Value::Text("bad".into())]),
            Err(EvalError::TypeMismatch)
        );
    }

    #[test]
    fn numeric_text_conversion_and_rendering_share_cancellation_and_text_limits() {
        for function in [TextFunction::Hex, TextFunction::Oct, TextFunction::Str] {
            let checks = Cell::new(0);
            let value = Value::Text("123.5".into());
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(NumberFormat::EnUs)
                .with_cancellation(&|| {
                    checks.set(checks.get() + 1);
                    false
                })
                .apply_text_function(function, std::slice::from_ref(&value))
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
                        .apply_text_function(function, std::slice::from_ref(&value)),
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
                .apply_text_function(function, &[Value::Text("2.5".into())]),
                Err(EvalError::TextLimit)
            );
        }
    }
}

#[cfg(test)]
mod str_tests;

#[cfg(test)]
mod radix_tests;
