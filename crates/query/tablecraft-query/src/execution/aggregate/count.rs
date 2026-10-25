//! Count consumes nullness without requiring numeric or storable operands.
use super::{Environment, Runtime};
use crate::{QueryError, plan::Expr};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::{EvalError, functions::text::byte_string::ExpressionByteOperand};

impl Runtime<'_, '_> {
    pub(super) fn count(
        &mut self,
        expression: &Expr,
        argument: Option<&Expr>,
        env: Environment<'_>,
    ) -> Result<Value, QueryError> {
        if let Some(results) = env.counts {
            for result in results {
                self.tick()?;
                // Borrowed immutable plan nodes distinguish occurrences even
                // when their operand syntax is identical. No pointer is stored.
                if std::ptr::eq(result.expression, expression) {
                    return Ok(Value::Int64(result.count));
                }
            }
            return Err(QueryError::Source);
        }
        let group = env.group.ok_or(QueryError::Grouping)?;
        let mut count = 0_i64;

        for row in group {
            self.tick()?;
            if let Some(argument) = argument
                && !self.count_argument(
                    argument,
                    Environment {
                        row,
                        group: None,
                        group_keys: None,
                        ..env
                    },
                )?
            {
                continue;
            }
            count = count.checked_add(1).ok_or(EvalError::Overflow)?;
        }
        Ok(Value::Int64(count))
    }

    pub(in crate::execution) fn count_argument(
        &mut self,
        argument: &Expr,
        env: Environment<'_>,
    ) -> Result<bool, QueryError> {
        counted_value(self.string_cell(argument, env)?.value).map_err(QueryError::Evaluation)
    }
}

fn counted_value(value: ExpressionByteOperand) -> Result<bool, EvalError> {
    match value {
        ExpressionByteOperand::Scalar(ExpressionValue::Value(Value::Null)) => Ok(false),
        ExpressionByteOperand::Scalar(
            ExpressionValue::Value(_) | ExpressionValue::Error(_) | ExpressionValue::StringBytes(_),
        )
        | ExpressionByteOperand::Bytes(_) => Ok(true),
        ExpressionByteOperand::Scalar(ExpressionValue::Empty | ExpressionValue::Missing) => {
            Err(EvalError::NonScalarExpression)
        }
    }
}

#[cfg(test)]
mod tests;
