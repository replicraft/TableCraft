//! Strict relational keys over retained scalar, String and standard Error data.
use super::{Environment, Runtime};
use crate::{QueryError, execution::cell::CellValue, plan::Expr, source::TextPolicy};
use std::cmp::Ordering;
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::{EvalError, comparison::Comparison, logic::Truth};

impl Runtime<'_, '_> {
    pub(super) fn comparison_key(
        &mut self,
        expression: &Expr,
        env: Environment<'_>,
    ) -> Result<CellValue, QueryError> {
        let cell = self.retained_expression_operand(expression, env)?;
        comparison_state(&cell)?;
        Ok(cell)
    }

    pub(super) fn repeatable_comparison_key(
        &mut self,
        expression: &Expr,
        env: Environment<'_>,
    ) -> Result<CellValue, QueryError> {
        let previous = self.repeatable_dynamic;
        self.repeatable_dynamic = true;
        let result = self.comparison_key(expression, env);
        self.repeatable_dynamic = previous;
        result
    }

    pub(super) fn validate_key_row(&mut self, row: &[CellValue]) -> Result<(), QueryError> {
        for cell in row {
            self.tick()?;
            comparison_state(cell)?;
        }
        Ok(())
    }

    pub(super) fn equal_key(
        &mut self,
        left: &CellValue,
        right: &CellValue,
        policy: TextPolicy,
    ) -> Result<bool, QueryError> {
        self.tick()?;
        comparison_pair(left, right)?;
        match (&left.value, &right.value) {
            (ExpressionValue::Value(Value::Null), ExpressionValue::Value(Value::Null)) => Ok(true),
            (ExpressionValue::Value(Value::Null), _) | (_, ExpressionValue::Value(Value::Null)) => {
                Ok(false)
            }
            _ => self
                .compare_key(Comparison::Equal, left, right, policy)
                .map(|truth| truth == Truth::True),
        }
    }

    pub(super) fn order_key(
        &mut self,
        left: &CellValue,
        right: &CellValue,
        policy: TextPolicy,
    ) -> Result<Ordering, QueryError> {
        self.tick()?;
        comparison_pair(left, right)?;
        match (&left.value, &right.value) {
            (ExpressionValue::Value(Value::Null), ExpressionValue::Value(Value::Null)) => {
                return Ok(Ordering::Equal);
            }
            (ExpressionValue::Value(Value::Null), _) => return Ok(Ordering::Less),
            (_, ExpressionValue::Value(Value::Null)) => return Ok(Ordering::Greater),
            _ => {}
        }
        if self.compare_key(Comparison::Equal, left, right, policy)? == Truth::True {
            return Ok(Ordering::Equal);
        }
        Ok(
            if self.compare_key(Comparison::Less, left, right, policy)? == Truth::True {
                Ordering::Less
            } else {
                Ordering::Greater
            },
        )
    }

    fn compare_key(
        &mut self,
        operator: Comparison,
        left: &CellValue,
        right: &CellValue,
        policy: TextPolicy,
    ) -> Result<Truth, QueryError> {
        if has_error_operand(left, right) {
            return self.compare_expression_predicate_policy(operator, left, right, policy);
        }
        match (&left.value, &right.value) {
            (ExpressionValue::Value(left), ExpressionValue::Value(right)) => {
                self.compare_policy(operator, left, right, policy)
            }
            (
                ExpressionValue::StringBytes(_),
                ExpressionValue::StringBytes(_) | ExpressionValue::Value(Value::Text(_)),
            )
            | (ExpressionValue::Value(Value::Text(_)), ExpressionValue::StringBytes(_)) => {
                self.compare_expression_predicate_policy(operator, left, right, policy)
            }
            (ExpressionValue::StringBytes(_), ExpressionValue::Value(_))
            | (ExpressionValue::Value(_), ExpressionValue::StringBytes(_)) => {
                Err(EvalError::TypeMismatch.into())
            }
            _ => Err(EvalError::NonScalarExpression.into()),
        }
    }
}

fn comparison_state(cell: &CellValue) -> Result<(), QueryError> {
    match cell.value {
        ExpressionValue::Value(_) | ExpressionValue::StringBytes(_) => Ok(()),
        ExpressionValue::Error(code) => u16::try_from(code)
            .map(|_| ())
            .map_err(|_| EvalError::UnsupportedExpression.into()),
        _ => Err(EvalError::NonScalarExpression.into()),
    }
}

fn has_error_operand(left: &CellValue, right: &CellValue) -> bool {
    matches!(left.value, ExpressionValue::Error(_))
        || matches!(right.value, ExpressionValue::Error(_))
}

fn comparison_pair(left: &CellValue, right: &CellValue) -> Result<(), QueryError> {
    comparison_state(left)?;
    comparison_state(right)?;
    if has_error_operand(left, right)
        && !matches!(
            (&left.value, &right.value),
            (ExpressionValue::Error(_), ExpressionValue::Error(_))
        )
    {
        return Err(EvalError::TypeMismatch.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn singleton_keys_validate_standard_codes_without_a_comparison() {
        for code in [0, 1, 65535] {
            assert_eq!(
                comparison_state(&ExpressionValue::Error(code).into()),
                Ok(())
            );
        }
        for code in [65536, u32::MAX] {
            assert_eq!(
                comparison_state(&ExpressionValue::Error(code).into()),
                Err(EvalError::UnsupportedExpression.into())
            );
        }
        for value in [ExpressionValue::Empty, ExpressionValue::Missing] {
            assert_eq!(
                comparison_state(&value.into()),
                Err(EvalError::NonScalarExpression.into())
            );
        }
    }

    #[test]
    fn error_keys_do_not_gain_null_first_or_numeric_scalar_coercion() {
        let error = ExpressionValue::Error(0).into();
        for value in [Value::Null, Value::Int32(0), Value::Text("Error 0".into())] {
            let scalar = CellValue::scalar(value);
            assert_eq!(
                comparison_pair(&error, &scalar),
                Err(EvalError::TypeMismatch.into())
            );
            assert_eq!(
                comparison_pair(&scalar, &error),
                Err(EvalError::TypeMismatch.into())
            );
        }
    }
}
