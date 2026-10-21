use super::Binder;
use crate::{
    QueryError,
    plan::{Expr, Slot},
    profile::{MAX_BIND_DEPTH, MAX_BIND_NODES},
    source::TextPolicy,
};

const MAX_POLICY_VISITS: usize = MAX_BIND_NODES * (MAX_BIND_DEPTH + 1);

impl Binder<'_> {
    pub(super) fn text_policy(&mut self, expr: &Expr) -> Result<TextPolicy, QueryError> {
        crate::plan::text_policy::expression(
            expr,
            &mut || {
                self.policy_visits += 1;
                if self.policy_visits > MAX_POLICY_VISITS {
                    return Err(QueryError::StructureLimit);
                }
                Ok(())
            },
            1,
        )
    }
    pub(super) fn policy_vector(&mut self, width: usize) -> Result<Vec<TextPolicy>, QueryError> {
        self.charge(
            width
                .checked_mul(size_of::<TextPolicy>())
                .ok_or(QueryError::StructureLimit)?,
        )?;
        let mut policies = Vec::new();
        policies
            .try_reserve_exact(width)
            .map_err(|_| QueryError::Allocation)?;
        Ok(policies)
    }
    pub(super) fn field(
        &self,
        depth: usize,
        index: usize,
        scope: &[Slot],
    ) -> Result<Expr, QueryError> {
        let slot = scope
            .iter()
            .filter(|s| s.available)
            .nth(index)
            .ok_or(QueryError::UnknownName)?;
        Ok(Expr::Field {
            depth,
            index,
            text_policy: slot.text_policy,
        })
    }
}
