//! Contextual financial conversion without changing kernel validation order.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        financial::{FinancialContext, FinancialFunction},
    },
};
use tablecraft_core::values::Value;

impl Evaluator<'_> {
    /// Apply evaluated financial operands using explicit numeric Text policies.
    /// Kernel conversion order, optional defaults and iterative checks are shared.
    pub fn apply_financial(
        &mut self,
        function: FinancialFunction,
        arguments: &[Value],
    ) -> Result<Value, EvalError> {
        if !function.accepts_arity(arguments.len()) {
            return Err(EvalError::UnsupportedExpression);
        }
        self.check(0)?;
        function.apply_with_context(arguments, self)
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
