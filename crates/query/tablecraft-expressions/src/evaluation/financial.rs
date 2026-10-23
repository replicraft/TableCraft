//! Contextual financial conversion without changing kernel validation order.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        financial::{FinancialContext, FinancialFunction},
    },
};
use tablecraft_core::values::{ExpressionValue, Value};

impl Evaluator<'_> {
    /// Apply evaluated financial operands using explicit numeric Text policies.
    /// Kernel conversion order, optional defaults and iterative checks are shared.
    pub fn apply_financial(
        &mut self,
        function: FinancialFunction,
        arguments: &[Value],
    ) -> Result<Value, EvalError> {
        self.apply_financial_arguments(function, arguments.len(), |index| Ok(&arguments[index]))
    }

    /// Apply evaluated expression data at the kernel's conversion position.
    /// Supplied SQL expressions must already have evaluated eagerly.
    pub fn apply_expression_financial(
        &mut self,
        function: FinancialFunction,
        arguments: &[ExpressionValue],
    ) -> Result<Value, EvalError> {
        let empty = Value::Int16(0);
        self.apply_financial_arguments(function, arguments.len(), |index| match &arguments[index] {
            ExpressionValue::Value(value) => Ok(value),
            ExpressionValue::Error(_) => Err(EvalError::TypeMismatch),
            ExpressionValue::Missing => Err(EvalError::NonScalarExpression),
            ExpressionValue::Empty
                if function == FinancialFunction::EuroConvert && matches!(index, 1 | 2) =>
            {
                Err(EvalError::TypeMismatch)
            }
            ExpressionValue::Empty => Ok(&empty),
        })
    }

    fn apply_financial_arguments<'a>(
        &mut self,
        function: FinancialFunction,
        count: usize,
        mut argument: impl FnMut(usize) -> Result<&'a Value, EvalError>,
    ) -> Result<Value, EvalError> {
        if !function.accepts_arity(count) {
            return Err(EvalError::UnsupportedExpression);
        }
        self.check(0)?;
        function.apply_with_input(count, &mut argument, self)
    }
}

impl FinancialContext for Evaluator<'_> {
    fn convert(&mut self, target: Conversion, value: &Value) -> Result<Value, EvalError> {
        if matches!(value, Value::Text(_)) {
            self.apply_conversion(target, value)
        } else {
            target.apply(value)
        }
    }
    fn step(&mut self) -> Result<(), EvalError> {
        self.check(0)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod expression_tests;
