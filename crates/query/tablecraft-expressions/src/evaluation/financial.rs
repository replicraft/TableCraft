//! Contextual financial conversion without changing kernel validation order.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        financial::{FinancialContext, FinancialFunction},
    },
};
use std::borrow::Cow;
use tablecraft_core::values::{ExpressionValue, Value};

impl Evaluator<'_> {
    /// Apply evaluated financial operands using explicit numeric Text policies.
    /// Kernel conversion order, optional defaults and iterative checks are shared.
    pub fn apply_financial(
        &mut self,
        function: FinancialFunction,
        arguments: &[Value],
    ) -> Result<Value, EvalError> {
        self.apply_financial_arguments(function, FinancialValues::Scalar(arguments))
    }

    /// Admit evaluated expression data at the kernel's conversion position.
    /// Supplied SQL expressions must already have evaluated eagerly.
    pub fn apply_expression_financial(
        &mut self,
        function: FinancialFunction,
        arguments: &[ExpressionValue],
    ) -> Result<Value, EvalError> {
        self.apply_financial_arguments(function, FinancialValues::Expression(arguments))
    }

    fn apply_financial_arguments(
        &mut self,
        function: FinancialFunction,
        arguments: FinancialValues<'_>,
    ) -> Result<Value, EvalError> {
        let count = arguments.len();
        if !function.accepts_arity(count) {
            return Err(EvalError::UnsupportedExpression);
        }
        self.check(0)?;
        function.apply_with_context(
            count,
            &mut FinancialEvaluation {
                evaluator: self,
                function,
                arguments,
            },
        )
    }
}

enum FinancialValues<'a> {
    Scalar(&'a [Value]),
    Expression(&'a [ExpressionValue]),
}

impl FinancialValues<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Scalar(values) => values.len(),
            Self::Expression(values) => values.len(),
        }
    }
}

struct FinancialEvaluation<'a, 'evaluator, 'host> {
    evaluator: &'evaluator mut Evaluator<'host>,
    function: FinancialFunction,
    arguments: FinancialValues<'a>,
}

impl<'a> FinancialContext<'a> for FinancialEvaluation<'a, '_, '_> {
    fn argument(&mut self, index: usize) -> Result<Cow<'a, Value>, EvalError> {
        match self.arguments {
            FinancialValues::Scalar(values) => Ok(Cow::Borrowed(&values[index])),
            FinancialValues::Expression(values) => match &values[index] {
                ExpressionValue::Value(value) => Ok(Cow::Borrowed(value)),
                ExpressionValue::StringBytes(value) => self
                    .evaluator
                    .publish_expression_string(value)
                    .map(Cow::Owned),
                ExpressionValue::Error(_) => Err(EvalError::TypeMismatch),
                ExpressionValue::Missing => Err(EvalError::NonScalarExpression),
                ExpressionValue::Empty
                    if self.function == FinancialFunction::EuroConvert
                        && matches!(index, 1 | 2) =>
                {
                    Err(EvalError::TypeMismatch)
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

    fn step(&mut self) -> Result<(), EvalError> {
        self.evaluator.check(0)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod expression_tests;

#[cfg(test)]
mod retained_tests;
