use crate::execution::cell::{CellValue, into_operator_scalar, into_scalar};
pub(super) mod accumulator;
pub(super) mod average;
pub(super) mod binary;
mod count;
mod encounter;
mod extrema;
mod fixed_total;
mod statistics;
mod sum;
use super::{Environment, Runtime};
use crate::{
    QueryError,
    plan::{Aggregate, Expr},
};
use tablecraft_core::values::Value;

impl Runtime<'_, '_> {
    pub(super) fn precomputed_aggregate(
        &mut self,
        expression: &Expr,
        env: Environment<'_>,
    ) -> Result<Option<CellValue>, QueryError> {
        let Some(results) = env.aggregate_results else {
            return Ok(None);
        };
        for result in results {
            self.tick()?;
            // Full node identity retains separate syntactic occurrences even
            // when a final kernel requests aggregate operands out of order.
            if std::ptr::eq(result.expression, expression) {
                return self.copy_expression(&result.value).map(Some);
            }
        }
        Err(QueryError::Source)
    }

    pub(super) fn aggregate(
        &mut self,
        expression: &Expr,
        kind: Aggregate,
        argument: Option<&Expr>,
        env: Environment<'_>,
    ) -> Result<Value, QueryError> {
        if let Some(mut result) = self.precomputed_aggregate(expression, env)? {
            result.value = self.evaluator().publish_expression_value(result.value)?;
            return Ok(into_scalar(result)?);
        }
        if matches!(kind, Aggregate::Count) {
            return self.count(argument, env);
        }
        if let Aggregate::Statistic(kind) = kind {
            return self.statistic(kind, argument.ok_or(QueryError::Unsupported)?, env);
        }
        if matches!(kind, Aggregate::Minimum | Aggregate::Maximum) {
            let mut result = self.extreme_aggregate(
                argument.ok_or(QueryError::Unsupported)?,
                env,
                matches!(kind, Aggregate::Maximum),
            )?;
            result.value = self.evaluator().publish_expression_value(result.value)?;
            return Ok(into_scalar(result)?);
        }
        if matches!(kind, Aggregate::First | Aggregate::Last) {
            let mut result = self.encounter_aggregate(
                argument.ok_or(QueryError::Unsupported)?,
                env,
                matches!(kind, Aggregate::Last),
            )?;
            result.value = self.evaluator().publish_expression_value(result.value)?;
            return Ok(into_scalar(result)?);
        }
        let group = env.group.ok_or(QueryError::Grouping)?;
        let mut count = 0_i64;
        let mut sum = sum::Sum::default();
        let mut mean = average::Mean::default();
        for row in group {
            self.tick()?;
            let value = self.numeric_aggregate_argument(
                argument,
                Environment {
                    row,
                    group: None,
                    group_keys: None,
                    ..env
                },
            )?;
            if matches!(value, Value::Null) {
                continue;
            }
            count = count
                .checked_add(1)
                .ok_or(tablecraft_expressions::EvalError::Overflow)?;
            match kind {
                Aggregate::Count => unreachable!("handled before scalar conversion"),
                Aggregate::Statistic(_) => unreachable!("handled by bounded statistical passes"),
                Aggregate::First | Aggregate::Last => unreachable!("handled before NULL filtering"),
                Aggregate::Average => mean.push(
                    &value,
                    self.budget,
                    self.options.cancelled,
                    &mut self.numeric_scratch,
                )?,
                Aggregate::Sum => sum.push(
                    &value,
                    self.budget,
                    self.options.cancelled,
                    &mut self.numeric_scratch,
                )?,
                Aggregate::Minimum | Aggregate::Maximum => {
                    unreachable!("handled by expression extrema")
                }
            }
        }
        if matches!(kind, Aggregate::Average) {
            return mean.finish(
                count,
                self.budget,
                self.options.cancelled,
                &mut self.numeric_scratch,
            );
        }
        sum.finish(
            self.budget,
            self.options.cancelled,
            &mut self.numeric_scratch,
        )
    }

    fn numeric_aggregate_argument(
        &mut self,
        argument: Option<&Expr>,
        environment: Environment<'_>,
    ) -> Result<Value, QueryError> {
        match argument {
            Some(argument) => Ok(into_operator_scalar(
                self.expression_operand(argument, environment)?,
            )?),
            None => Ok(Value::Int64(1)),
        }
    }
}
