//! Prove stable retained field provenance without evaluating an operand twice.
use crate::{
    QueryError,
    execution::{Runtime, cell::CellValue},
    plan::Expr,
    profile::MAX_BIND_DEPTH,
};

impl Runtime<'_, '_> {
    pub(super) fn stable_extreme_policy(
        &mut self,
        argument: &Expr,
        group: &[Vec<CellValue>],
        depth: usize,
    ) -> Result<bool, QueryError> {
        self.tick()?;
        if depth > MAX_BIND_DEPTH {
            return Err(QueryError::StructureLimit);
        }

        let index = match argument {
            Expr::CVar(value) => {
                return self.stable_extreme_policy(value, group, depth + 1);
            }
            Expr::Field {
                depth: 0, index, ..
            } => *index,
            // Constructed, conditional and domain values can replace their
            // conservative static lineage with a completed runtime policy.
            _ => return Ok(false),
        };
        let mut observed = None;
        for row in group {
            self.tick()?;
            let cell = row.get(index).ok_or(QueryError::UnknownName)?;
            let provenance = (cell.policy, cell.policy_complete);
            if observed.is_some_and(|previous| previous != provenance) {
                return Ok(false);
            }
            observed = Some(provenance);
        }
        Ok(true)
    }
}
