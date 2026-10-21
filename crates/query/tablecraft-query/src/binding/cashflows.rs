//! Bind explicit array elements without adding arrays to scalar storage contracts.
use super::Binder;
use crate::{
    QueryError,
    plan::{Expr, Slot},
};
use tablecraft_expressions::functions::financial::cashflows::{CashFlowFunction, sql_arguments};
use tablecraft_sql::select::FunctionArgument;

impl Binder<'_> {
    pub(super) fn cashflows(
        &mut self,
        function: CashFlowFunction,
        arguments: &[FunctionArgument],
        scopes: &[&[Slot]],
        aggregates: bool,
        depth: usize,
    ) -> Result<Expr, QueryError> {
        let elements =
            sql_arguments::elements(function, arguments).ok_or(QueryError::Unsupported)?;
        let supplied = arguments
            .iter()
            .filter(|argument| matches!(argument, FunctionArgument::Expression(_)))
            .count();
        let count = elements.len() + supplied - 1;
        self.charge(count * size_of::<Expr>())?;
        let mut operands = Vec::new();
        operands
            .try_reserve_exact(count)
            .map_err(|_| QueryError::Allocation)?;
        let mut array = 0..0;

        for (index, argument) in arguments.iter().enumerate() {
            if index == function.array_index() {
                self.tick(depth + 1)?;
                let start = operands.len();
                for element in elements {
                    let FunctionArgument::Expression(expression) = element else {
                        return Err(QueryError::Unsupported);
                    };
                    operands.push(self.expression(expression, scopes, aggregates, depth + 2)?);
                }
                array = start..operands.len();
            } else if let FunctionArgument::Expression(expression) = argument {
                operands.push(self.expression(expression, scopes, aggregates, depth + 1)?);
            }
        }

        Ok(Expr::CashFlowCall(function, operands, array))
    }
}
