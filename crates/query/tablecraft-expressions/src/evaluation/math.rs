//! Meter math calls and supply explicit numeric Text conversions and random state.
use super::Evaluator;
use crate::{EvalError, functions::math::MathFunction};
use tablecraft_core::values::{ExpressionValue, Value};

impl Evaluator<'_> {
    /// Apply evaluated math operands; only Rnd consults the host random sequence.
    /// Earlier successful calls remain consumed if later query work fails.
    pub fn apply_math(
        &mut self,
        function: MathFunction,
        values: &[Value],
    ) -> Result<Value, EvalError> {
        self.apply_math_arguments(function, values.len(), |index| Ok(&values[index]))
    }

    /// Apply evaluated expression data at each kernel's validation position.
    /// Callers evaluate all arguments eagerly before entering this method.
    pub fn apply_expression_math(
        &mut self,
        function: MathFunction,
        values: &[ExpressionValue],
    ) -> Result<Value, EvalError> {
        let empty = Value::Int16(0);
        self.apply_math_arguments(function, values.len(), |index| match &values[index] {
            ExpressionValue::Value(value) => Ok(value),
            ExpressionValue::Error(_) => Err(EvalError::TypeMismatch),
            ExpressionValue::StringBytes(_) => Err(EvalError::NonScalarExpression),
            ExpressionValue::Missing => Err(EvalError::NonScalarExpression),
            ExpressionValue::Empty if function == MathFunction::Round && index == 0 => {
                Err(EvalError::NonScalarExpression)
            }
            ExpressionValue::Empty => Ok(&empty),
        })
    }

    fn apply_math_arguments<'a>(
        &mut self,
        function: MathFunction,
        count: usize,
        mut argument: impl FnMut(usize) -> Result<&'a Value, EvalError>,
    ) -> Result<Value, EvalError> {
        if !function.accepts_arity(count) {
            return Err(EvalError::UnsupportedExpression);
        }
        self.check(0)?;
        if function == MathFunction::Rnd && self.repeatable_dynamic {
            return Err(EvalError::DynamicNotRepeatable);
        }
        let random = self.random;
        function.apply_with_input(count, &mut argument, random, &mut |target, value| {
            if matches!(value, Value::Text(_)) {
                self.apply_conversion(target, value)
            } else {
                target.apply(value)
            }
        })
    }
}

#[cfg(test)]
mod random_tests;

#[cfg(test)]
mod expression_tests;
