//! Min/Max retain expression data under strict scalar/String key comparison.
mod policy;
mod state;
use super::{Environment, Expr, QueryError, Runtime};
use crate::{execution::cell::CellValue, source::TextPolicy};
pub(in crate::execution) use state::ExtremeState;
use std::cmp::Ordering;
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::{comparison::Comparison, logic::Truth};

impl Runtime<'_, '_> {
    pub(in crate::execution) fn extreme_aggregate(
        &mut self,
        argument: &Expr,
        env: Environment<'_>,
        maximum: bool,
    ) -> Result<CellValue, QueryError> {
        let group = env.group.ok_or(QueryError::Grouping)?;
        // Completed construction can discard provisional static contributors,
        // so even nondefault lineage cannot prove a stable runtime comparator.
        // Direct retained fields keep the one-cell lane when the whole group's
        // already available policy metadata proves that it cannot change.
        let deferred = self.options.collation_resolver.is_some()
            && (self.text_policy(argument)? == TextPolicy::Default
                || !self.stable_extreme_policy(argument, group, 1)?);
        let mut state = ExtremeState::new(maximum, deferred);

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
            self.push_extreme(&mut state, argument, value)?;
        }

        self.finish_extreme(state, argument)
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
