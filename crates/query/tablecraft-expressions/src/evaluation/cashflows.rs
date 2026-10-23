//! Explicit cashflow inputs stay separate from stored scalar values.
use super::{Evaluator, NoBindings};
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        financial::cashflows::{CashFlowFunction, Work, sql_arguments},
    },
};
use tablecraft_core::values::{ExpressionValue, Value};
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
        self.apply_cashflow_inputs(
            function,
            arguments.len(),
            |index| Ok(&arguments[index]),
            values.len(),
            |index| Ok(&values[index]),
        )
    }

    /// Apply explicit expression controls and elements after eager evaluation.
    /// Numeric Empty controls use zero; array Empty/Error remain unsupported data.
    pub fn apply_expression_cashflows(
        &mut self,
        function: CashFlowFunction,
        arguments: &[ExpressionValue],
        values: &[ExpressionValue],
    ) -> Result<Value, EvalError> {
        let empty = Value::Int16(0);
        self.apply_cashflow_inputs(
            function,
            arguments.len(),
            |index| match &arguments[index] {
                ExpressionValue::Value(value) => Ok(value),
                ExpressionValue::Empty => Ok(&empty),
                ExpressionValue::Error(_) => Err(EvalError::TypeMismatch),
                ExpressionValue::Missing => Err(EvalError::NonScalarExpression),
            },
            values.len(),
            |index| match &values[index] {
                ExpressionValue::Value(value) => Ok(value),
                ExpressionValue::Error(_) | ExpressionValue::Empty => Err(EvalError::TypeMismatch),
                ExpressionValue::Missing => Err(EvalError::NonScalarExpression),
            },
        )
    }

    fn apply_cashflow_inputs<'a>(
        &mut self,
        function: CashFlowFunction,
        scalar_count: usize,
        mut scalar: impl FnMut(usize) -> Result<&'a Value, EvalError>,
        element_count: usize,
        mut element: impl FnMut(usize) -> Result<&'a Value, EvalError>,
    ) -> Result<Value, EvalError> {
        if !function.accepts_arity(scalar_count + 1) {
            return Err(EvalError::UnsupportedExpression);
        }
        self.check(0)?;
        let number_format = self.number_format;
        let limits = self.limits;
        let cancelled = self.cancelled;
        function.apply_with_input(
            scalar_count,
            &mut scalar,
            element_count,
            &mut element,
            &mut Work {
                budget: self.budget,
                cancelled,
            },
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
            .charge_allocation(elements.len() as u64, size_of::<ExpressionValue>() as u64)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(elements.len())
            .map_err(|_| EvalError::Allocation)?;
        let mut scalars: [ExpressionValue; CashFlowFunction::MAX_SCALAR_ARGUMENTS] =
            std::array::from_fn(|_| ExpressionValue::Value(Value::Null));
        let mut scalar_count = 0;

        for (index, argument) in arguments.iter().enumerate() {
            if index == function.array_index() {
                // The constructor itself participates in depth/work checks.
                self.check(depth + 1)?;
                for element in elements {
                    let FunctionArgument::Expression(expression) = element else {
                        return Err(EvalError::UnsupportedExpression);
                    };
                    values.push(self.expression_operand(expression, depth + 2)?);
                }
            } else if let FunctionArgument::Expression(expression) = argument {
                scalars[scalar_count] = self.expression_operand(expression, depth + 1)?;
                scalar_count += 1;
            }
        }

        self.apply_expression_cashflows(function, &scalars[..scalar_count], &values)
    }
}

#[cfg(test)]
mod expression_tests;
