//! Min/Max retain expression data under strict scalar/String key comparison.
use super::{Environment, Expr, QueryError, Runtime};
use crate::execution::cell::CellValue;
use std::cmp::Ordering;
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::{EvalError, comparison::Comparison, logic::Truth};

impl Runtime<'_, '_> {
    pub(in crate::execution) fn extreme_aggregate(
        &mut self,
        argument: &Expr,
        env: Environment<'_>,
        maximum: bool,
    ) -> Result<CellValue, QueryError> {
        let group = env.group.ok_or(QueryError::Grouping)?;
        let mut selected = CellValue::scalar(Value::Null);

        let mut policy = crate::source::TextPolicy::Default;
        for row in group {
            self.tick()?;
            let value = self.retained_expression_operand(
                argument,
                Environment {
                    row,
                    group: None,
                    group_keys: None,
                    ..env
                },
            )?;
            policy = policy.merge(value.policy);
            match &value.value {
                ExpressionValue::Value(Value::Null) => continue,
                ExpressionValue::Value(_) | ExpressionValue::StringBytes(_) => {}
                ExpressionValue::Error(_) => {
                    // Even a singleton must have a qualified standard code.
                    self.evaluator().compare_expression_values(
                        Comparison::Equal,
                        &value,
                        &value,
                    )?;
                }
                ExpressionValue::Empty | ExpressionValue::Missing => {
                    return Err(EvalError::NonScalarExpression.into());
                }
            }

            let replace = if matches!(selected.value, ExpressionValue::Value(Value::Null)) {
                true
            } else {
                match (&value.value, &selected.value) {
                    (
                        ExpressionValue::Value(_) | ExpressionValue::StringBytes(_),
                        ExpressionValue::Value(_) | ExpressionValue::StringBytes(_),
                    ) => {
                        let policy = if matches!(
                            (&value.value, &selected.value),
                            (
                                ExpressionValue::Value(Value::Text(_))
                                    | ExpressionValue::StringBytes(_),
                                ExpressionValue::Value(Value::Text(_))
                                    | ExpressionValue::StringBytes(_)
                            )
                        ) {
                            self.text_policy(argument)?.merge(policy)
                        } else {
                            crate::source::TextPolicy::Default
                        };
                        self.order_key(&value, &selected, policy)?
                            == if maximum {
                                Ordering::Greater
                            } else {
                                Ordering::Less
                            }
                    }
                    _ => {
                        let comparison = if maximum {
                            Comparison::Greater
                        } else {
                            Comparison::Less
                        };
                        self.evaluator()
                            .compare_expression_values(comparison, &value, &selected)?
                            == Truth::True
                    }
                }
            };
            if replace {
                selected = value;
            }
        }

        Ok(selected.with_policy(policy))
    }
}
