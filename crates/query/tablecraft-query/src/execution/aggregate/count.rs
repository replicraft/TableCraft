//! Count consumes nullness without requiring numeric or storable operands.
use super::{Environment, Runtime};
use crate::{QueryError, plan::Expr};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::{EvalError, functions::conversions::errors::into_scalar};

impl Runtime<'_, '_> {
    pub(super) fn count(
        &mut self,
        argument: Option<&Expr>,
        env: Environment<'_>,
    ) -> Result<Value, QueryError> {
        let group = env.group.ok_or(QueryError::Grouping)?;
        let mut count = 0_i64;

        for row in group {
            self.tick()?;
            if let Some(argument) = argument {
                let value = self.expression_operand(
                    argument,
                    Environment {
                        row,
                        group: None,
                        ..env
                    },
                )?;
                match value {
                    ExpressionValue::Value(Value::Null) => continue,
                    ExpressionValue::Value(_) | ExpressionValue::Error(_) => {}
                    value => {
                        // Empty/Missing are not admitted as query cells here.
                        into_scalar(value)?;
                    }
                }
            }
            count = count.checked_add(1).ok_or(EvalError::Overflow)?;
        }

        Ok(Value::Int64(count))
    }
}
