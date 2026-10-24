//! Static and retained group-key policies enrich scalar and raw string results.
use crate::{
    QueryError,
    execution::{Environment, Runtime},
    plan::{
        Expr,
        identity::{Comparer, Identity},
    },
    profile::MAX_BIND_DEPTH,
    source::TextPolicy,
};

impl Runtime<'_, '_> {
    pub(in crate::execution) fn result_text_policy(
        &mut self,
        expression: &Expr,
        current: TextPolicy,
        env: Environment<'_>,
    ) -> Result<TextPolicy, QueryError> {
        let mut policy = current.merge(self.text_policy(expression)?);
        if self.options.collation_resolver.is_none() {
            return Ok(policy);
        }

        let scopes = std::iter::once((0, env.group_keys)).chain(
            env.outer
                .iter()
                .enumerate()
                .map(|(index, scope)| (index + 1, scope.group_keys)),
        );
        for (shift, keys) in scopes {
            let Some(keys) = keys else {
                continue;
            };
            for (key, cell) in keys.expressions.iter().zip(keys.values) {
                let mut tick = |depth| {
                    if depth > MAX_BIND_DEPTH {
                        return Err(QueryError::StructureLimit);
                    }
                    self.tick()
                };
                if (Comparer { tick: &mut tick }).same_expression(
                    expression,
                    key,
                    1,
                    Identity {
                        shift,
                        projection: false,
                    },
                )? {
                    policy = policy.merge(cell.policy);
                }
            }
        }
        Ok(policy)
    }
}
