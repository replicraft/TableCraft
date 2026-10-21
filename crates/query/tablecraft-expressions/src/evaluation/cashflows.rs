//! Explicit cashflow inputs stay separate from stored scalar values.
use super::Evaluator;
use crate::{
    EvalError,
    functions::financial::cashflows::{CashFlowFunction, sql_arguments},
};
use tablecraft_core::values::Value;
use tablecraft_sql::select::FunctionArgument;

impl Evaluator<'_> {
    /// Apply explicitly supplied cashflow elements and evaluated scalar operands.
    /// All allocations and iterative work use this evaluator's cumulative budget.
    pub fn apply_cashflows(
        &mut self,
        function: CashFlowFunction,
        arguments: &[Value],
        values: &[Value],
    ) -> Result<Value, EvalError> {
        function.apply(arguments, values, self.budget, self.cancelled)
    }

    pub(super) fn cashflows(
        &mut self,
        function: CashFlowFunction,
        arguments: &[FunctionArgument],
        depth: usize,
    ) -> Result<Value, EvalError> {
        let elements =
            sql_arguments::elements(function, arguments).ok_or(EvalError::UnsupportedExpression)?;
        self.budget
            .charge_allocation(elements.len() as u64, size_of::<Value>() as u64)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(elements.len())
            .map_err(|_| EvalError::Allocation)?;
        let mut scalars = [Value::Null, Value::Null];
        let mut scalar_count = 0;

        for (index, argument) in arguments.iter().enumerate() {
            if index == function.array_index() {
                // The constructor itself participates in depth/work checks.
                self.check(depth + 1)?;
                for element in elements {
                    let FunctionArgument::Expression(expression) = element else {
                        return Err(EvalError::UnsupportedExpression);
                    };
                    values.push(self.node(expression, depth + 2)?);
                }
            } else if let FunctionArgument::Expression(expression) = argument {
                scalars[scalar_count] = self.node(expression, depth + 1)?;
                scalar_count += 1;
            }
        }

        self.apply_cashflows(function, &scalars[..scalar_count], &values)
    }
}
