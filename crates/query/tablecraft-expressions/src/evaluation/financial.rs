//! Shared scalar/bound SQL financial metering under the existing evaluator context.
use super::Evaluator;
use crate::{EvalError, functions::financial::FinancialFunction};
use tablecraft_core::{limits::Resource, values::Value};
impl Evaluator<'_> {
    /// Apply already evaluated financial operands under this operation's budget.
    /// Iterative kernels check cancellation and charge every solver iteration.
    pub fn apply_financial(
        &mut self,
        function: FinancialFunction,
        arguments: &[Value],
    ) -> Result<Value, EvalError> {
        function.apply_with_check(arguments, &mut || {
            if self.cancelled.is_some_and(|cancel| cancel()) {
                return Err(EvalError::Cancelled);
            }
            self.budget.charge(Resource::Steps, 1)?;
            Ok(())
        })
    }
}
