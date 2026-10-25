//! Count consumes nullness without requiring numeric or storable operands.
use super::{Environment, Runtime};
use crate::{QueryError, plan::Expr};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::{EvalError, functions::text::byte_string::ExpressionByteOperand};

impl Runtime<'_, '_> {
    pub(super) fn count(
        &mut self,
        argument: Option<&Expr>,
        env: Environment<'_>,
    ) -> Result<Value, QueryError> {
        if argument.is_none()
            && let Some(count) = self.progressive_count
        {
            self.tick()?;
            return Ok(Value::Int64(count));
        }
        let group = env.group.ok_or(QueryError::Grouping)?;
        let mut count = 0_i64;

        for row in group {
            self.tick()?;
            if let Some(argument) = argument {
                let value = self.string_cell(
                    argument,
                    Environment {
                        row,
                        group: None,
                        group_keys: None,
                        ..env
                    },
                )?;

                match value.value {
                    ExpressionByteOperand::Scalar(ExpressionValue::Value(Value::Null)) => {
                        continue;
                    }
                    ExpressionByteOperand::Scalar(
                        ExpressionValue::Value(_)
                        | ExpressionValue::Error(_)
                        | ExpressionValue::StringBytes(_),
                    )
                    | ExpressionByteOperand::Bytes(_) => {}
                    ExpressionByteOperand::Scalar(
                        ExpressionValue::Empty | ExpressionValue::Missing,
                    ) => {
                        // Empty/Missing are not admitted as query cells here.
                        return Err(EvalError::NonScalarExpression.into());
                    }
                }
            }

            count = count.checked_add(1).ok_or(EvalError::Overflow)?;
        }

        Ok(Value::Int64(count))
    }
}
