//! Min/Max retain expression data without widening scalar key comparison.
use super::{Environment, Expr, QueryError, Runtime};
use std::cmp::Ordering;
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::{EvalError, comparison::Comparison, logic::Truth};

impl Runtime<'_, '_> {
    pub(in crate::execution) fn extreme_aggregate(
        &mut self,
        argument: &Expr,
        env: Environment<'_>,
        maximum: bool,
    ) -> Result<ExpressionValue, QueryError> {
        let group = env.group.ok_or(QueryError::Grouping)?;
        let mut selected = ExpressionValue::Value(Value::Null);

        for row in group {
            self.tick()?;
            let value = self.expression_operand(
                argument,
                Environment {
                    row,
                    group: None,
                    ..env
                },
            )?;
            match &value {
                ExpressionValue::Value(Value::Null) => continue,
                ExpressionValue::Value(_) => {}
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

            let replace = if matches!(selected, ExpressionValue::Value(Value::Null)) {
                true
            } else {
                match (&value, &selected) {
                    (ExpressionValue::Value(value), ExpressionValue::Value(selected)) => {
                        let policy =
                            if matches!((value, selected), (Value::Text(_), Value::Text(_))) {
                                self.text_policy(argument)?
                            } else {
                                crate::source::TextPolicy::Default
                            };
                        self.order(value, selected, policy)?
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

        Ok(selected)
    }
}
