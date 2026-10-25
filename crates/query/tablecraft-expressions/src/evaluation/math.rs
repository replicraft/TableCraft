//! Meter math calls and supply explicit numeric Text conversions and random state.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        math::{MathContext, MathFunction},
    },
};
use std::borrow::Cow;
use tablecraft_core::values::{ExpressionValue, Value};

impl Evaluator<'_> {
    /// Apply evaluated math operands; only Rnd consults the host random sequence.
    /// Earlier successful calls remain consumed if later query work fails.
    pub fn apply_math(
        &mut self,
        function: MathFunction,
        values: &[Value],
    ) -> Result<Value, EvalError> {
        self.apply_math_arguments(function, MathValues::Scalar(values))
    }

    /// Admit each evaluated argument at the kernel's validation position.
    /// Callers evaluate all arguments eagerly before entering this method.
    pub fn apply_expression_math(
        &mut self,
        function: MathFunction,
        values: &[ExpressionValue],
    ) -> Result<Value, EvalError> {
        self.apply_math_arguments(function, MathValues::Expression(values))
    }

    fn apply_math_arguments(
        &mut self,
        function: MathFunction,
        values: MathValues<'_>,
    ) -> Result<Value, EvalError> {
        let count = values.len();
        if !function.accepts_arity(count) {
            return Err(EvalError::UnsupportedExpression);
        }
        self.check(0)?;
        if function == MathFunction::Rnd && self.repeatable_dynamic {
            return Err(EvalError::DynamicNotRepeatable);
        }
        let random = self.random;
        function.apply_with_context(
            count,
            &mut MathEvaluation {
                evaluator: self,
                function,
                values,
            },
            random,
        )
    }
}

enum MathValues<'a> {
    Scalar(&'a [Value]),
    Expression(&'a [ExpressionValue]),
}

impl MathValues<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Scalar(values) => values.len(),
            Self::Expression(values) => values.len(),
        }
    }
}

struct MathEvaluation<'a, 'evaluator, 'host> {
    evaluator: &'evaluator mut Evaluator<'host>,
    function: MathFunction,
    values: MathValues<'a>,
}

impl<'a> MathContext<'a> for MathEvaluation<'a, '_, '_> {
    fn argument(&mut self, index: usize) -> Result<Cow<'a, Value>, EvalError> {
        match self.values {
            MathValues::Scalar(values) => Ok(Cow::Borrowed(&values[index])),
            MathValues::Expression(values) => match &values[index] {
                ExpressionValue::Value(value) => Ok(Cow::Borrowed(value)),
                ExpressionValue::StringBytes(value) => self
                    .evaluator
                    .publish_expression_string(value)
                    .map(Cow::Owned),
                ExpressionValue::Error(_) => Err(EvalError::TypeMismatch),
                ExpressionValue::Missing => Err(EvalError::NonScalarExpression),
                ExpressionValue::Empty if self.function == MathFunction::Round && index == 0 => {
                    Err(EvalError::NonScalarExpression)
                }
                ExpressionValue::Empty => Ok(Cow::Owned(Value::Int16(0))),
            },
        }
    }

    fn convert(&mut self, target: Conversion, value: &Value) -> Result<Value, EvalError> {
        if matches!(value, Value::Text(_)) {
            self.evaluator.apply_conversion(target, value)
        } else {
            target.apply(value)
        }
    }
}

#[cfg(test)]
mod random_tests;

#[cfg(test)]
mod expression_tests;

#[cfg(test)]
mod retained_tests;
