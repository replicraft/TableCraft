use super::Binder;
use crate::{
    QueryError,
    plan::{Expr, Slot},
    profile::MAX_BIND_NODES,
    source::{TextCollationId, TextPolicy},
};

impl Binder<'_> {
    pub(super) fn text_policy(&mut self, expr: &Expr) -> Result<TextPolicy, QueryError> {
        let binding_depth = self.binding_depth;
        crate::plan::text_policy::expression(
            expr,
            &mut || {
                self.work()?;
                self.policy_visits += 1;
                if self.policy_visits > MAX_BIND_NODES * (binding_depth.get() + 1) {
                    return Err(QueryError::StructureLimit);
                }
                Ok(())
            },
            1,
            binding_depth,
        )
    }

    pub(super) fn copy_text_collations(
        &mut self,
        policies: &[Option<TextCollationId>],
    ) -> Result<Vec<Option<TextCollationId>>, QueryError> {
        self.charge(size_of_val(policies))?;
        let mut copied = Vec::new();
        copied
            .try_reserve_exact(policies.len())
            .map_err(|_| QueryError::Allocation)?;
        copied.extend_from_slice(policies);
        Ok(copied)
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
