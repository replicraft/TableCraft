//! Explicit cashflow inputs stay separate from stored scalar values.
use super::{Evaluator, NoBindings};
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        financial::cashflows::{CashFlowFunction, sql_arguments},
    },
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
        if !function.accepts_arity(arguments.len() + 1) {
            return Err(EvalError::UnsupportedExpression);
        }
        self.check(0)?;
        let number_format = self.number_format;
        let limits = self.limits;
        let cancelled = self.cancelled;
        function.apply_with_converter(
            arguments,
            values,
            self.budget,
            cancelled,
            &mut |value, budget| {
                let converted = if matches!(value, Value::Text(_)) {
                    // Scalar conversion reads only these policies and the shared quota;
                    // no bindings or other context providers are consulted.
                    let mut converter = Evaluator::new(&NoBindings, budget, limits);
                    converter.number_format = number_format;
                    converter.cancelled = cancelled;
                    converter.apply_conversion(Conversion::Double, value)?
                } else {
                    Conversion::Double.apply(value)?
                };
                let Value::Double(value) = converted else {
                    return Err(EvalError::TypeMismatch);
                };
                Ok(value.to_float())
            },
        )
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
