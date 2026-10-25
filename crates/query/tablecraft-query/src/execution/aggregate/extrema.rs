//! Min/Max retain expression data under strict scalar/String key comparison.
use super::{Environment, Expr, QueryError, Runtime};
use crate::{execution::cell::CellValue, source::TextPolicy};
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
        let mut policy = TextPolicy::Default;
        let mut complete = false;
        // A later runtime field policy can reverse an earlier default ordering.
        // Stable global/declared field policies keep the existing one-cell path.
        let deferred = self.options.collation_resolver.is_some()
            && self.text_policy(argument)? == TextPolicy::Default;
        let mut candidates = Vec::new();

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
            complete |= value.policy_complete;
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

            let string = matches!(
                value.value,
                ExpressionValue::Value(Value::Text(_)) | ExpressionValue::StringBytes(_)
            );
            if deferred && string {
                if !matches!(selected.value, ExpressionValue::Value(Value::Null)) {
                    return Err(EvalError::TypeMismatch.into());
                }
                self.push(&mut candidates, value)?;
            } else {
                // Strict extrema do not mix Strings with numeric/Error/other
                // scalar families. This refusal needs no comparer; non-String
                // populations keep one selected cell even with a resolver.
                if !candidates.is_empty() {
                    return Err(EvalError::TypeMismatch.into());
                }
                if self.replace_extreme(argument, &selected, &value, policy, maximum)? {
                    selected = value;
                }
            }
        }

        // Evaluate no expression twice: only retained cells are compared here.
        // NULL contributors still participate in the final policy above.
        for value in candidates {
            self.tick()?;
            if self.replace_extreme(argument, &selected, &value, policy, maximum)? {
                selected = value;
            }
        }

        Ok(selected.with_policy(policy).with_complete_policy(complete))
    }

    fn replace_extreme(
        &mut self,
        argument: &Expr,
        selected: &CellValue,
        value: &CellValue,
        policy: TextPolicy,
        maximum: bool,
    ) -> Result<bool, QueryError> {
        if matches!(selected.value, ExpressionValue::Value(Value::Null)) {
            return Ok(true);
        }

        match (&value.value, &selected.value) {
            (
                ExpressionValue::Value(_) | ExpressionValue::StringBytes(_),
                ExpressionValue::Value(_) | ExpressionValue::StringBytes(_),
            ) => {
                let policy = if matches!(
                    (&value.value, &selected.value),
                    (
                        ExpressionValue::Value(Value::Text(_)) | ExpressionValue::StringBytes(_),
                        ExpressionValue::Value(Value::Text(_)) | ExpressionValue::StringBytes(_)
                    )
                ) {
                    if value.policy_complete || selected.policy_complete {
                        policy
                    } else {
                        self.text_policy(argument)?.merge(policy)
                    }
                } else {
                    TextPolicy::Default
                };
                Ok(self.order_key(value, selected, policy)?
                    == if maximum {
                        Ordering::Greater
                    } else {
                        Ordering::Less
                    })
            }
            _ => {
                let comparison = if maximum {
                    Comparison::Greater
                } else {
                    Comparison::Less
                };
                Ok(self
                    .evaluator()
                    .compare_expression_values(comparison, value, selected)?
                    == Truth::True)
            }
        }
    }
}
