//! Explicit final progressive projections borrow completed grouping key values.
use super::{Environment, Runtime, cell::CellValue};
use crate::{
    QueryError,
    plan::{
        Expr,
        identity::{Comparer, Identity},
    },
};

#[cfg(test)]
mod tests;

impl Runtime<'_, '_> {
    pub(super) fn retained_group_value(
        &mut self,
        expression: &Expr,
        environment: Environment<'_>,
    ) -> Result<Option<CellValue>, QueryError> {
        let Some(keys) = environment.group_keys.filter(|keys| keys.retained_values) else {
            return Ok(None);
        };
        if keys.expressions.len() != keys.values.len() {
            return Err(QueryError::Width);
        }
        for (key, value) in keys.expressions.iter().zip(keys.values) {
            let mut tick = |depth| {
                if depth > self.options.binding_depth.get() {
                    return Err(QueryError::StructureLimit);
                }
                self.tick()
            };
            if (Comparer { tick: &mut tick }).same_expression(
                expression,
                key,
                1,
                Identity {
                    shift: 0,
                    projection: false,
                },
            )? {
                return self.copy_expression(value).map(Some);
            }
        }
        Ok(None)
    }
}
