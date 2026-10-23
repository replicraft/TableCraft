//! Materialize explicit array positions in written expression order.
use super::{Environment, Runtime};
use crate::{QueryError, plan::Expr};
use std::ops::Range;
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::functions::financial::cashflows::CashFlowFunction;

impl Runtime<'_, '_> {
    pub(super) fn cashflows(
        &mut self,
        function: CashFlowFunction,
        operands: &[Expr],
        array: &Range<usize>,
        environment: Environment<'_>,
    ) -> Result<Value, QueryError> {
        self.tick()?;
        self.budget
            .charge_allocation(array.len() as u64, size_of::<ExpressionValue>() as u64)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(array.len())
            .map_err(|_| QueryError::Allocation)?;
        let mut scalars: [ExpressionValue; CashFlowFunction::MAX_SCALAR_ARGUMENTS] =
            std::array::from_fn(|_| ExpressionValue::Value(Value::Null));
        let mut scalar_count = 0;

        for (index, operand) in operands.iter().enumerate() {
            let value = self.expression_operand(operand, environment)?.value;
            if array.contains(&index) {
                values.push(value);
            } else {
                scalars[scalar_count] = value;
                scalar_count += 1;
            }
        }

        Ok(self.evaluator().apply_expression_cashflows(
            function,
            &scalars[..scalar_count],
            &values,
        )?)
    }
}
