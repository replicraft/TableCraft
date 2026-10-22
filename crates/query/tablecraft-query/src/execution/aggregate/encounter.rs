//! Encounter-order selection preserves expression data and evaluates every member.
use super::{Environment, Expr, QueryError, Runtime};
use crate::execution::cell::CellValue;
use tablecraft_core::values::Value;

impl Runtime<'_, '_> {
    pub(in crate::execution) fn encounter_aggregate(
        &mut self,
        argument: &Expr,
        env: Environment<'_>,
        keep_last: bool,
    ) -> Result<CellValue, QueryError> {
        let group = env.group.ok_or(QueryError::Grouping)?;
        let mut selected = CellValue::scalar(Value::Null);
        let mut policy = crate::source::TextPolicy::Default;
        for (index, row) in group.iter().enumerate() {
            self.tick()?;
            let value = self.expression_operand(
                argument,
                Environment {
                    row,
                    group: None,
                    group_keys: None,
                    ..env
                },
            )?;
            policy = policy.merge(value.policy);
            if index == 0 || keep_last {
                selected = value;
            }
        }
        Ok(selected.with_policy(policy))
    }
}
