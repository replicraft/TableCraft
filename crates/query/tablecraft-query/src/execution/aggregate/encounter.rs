//! Encounter-order selection preserves expression data and evaluates every member.
use super::{Environment, Expr, QueryError, Runtime};
use tablecraft_core::values::{ExpressionValue, Value};

impl Runtime<'_, '_> {
    pub(in crate::execution) fn encounter_aggregate(
        &mut self,
        argument: &Expr,
        env: Environment<'_>,
        keep_last: bool,
    ) -> Result<ExpressionValue, QueryError> {
        let group = env.group.ok_or(QueryError::Grouping)?;
        let mut selected = ExpressionValue::Value(Value::Null);
        for (index, row) in group.iter().enumerate() {
            self.tick()?;
            let value = self.expression_operand(
                argument,
                Environment {
                    row,
                    group: None,
                    ..env
                },
            )?;
            if index == 0 || keep_last {
                selected = value;
            }
        }
        Ok(selected)
    }
}
