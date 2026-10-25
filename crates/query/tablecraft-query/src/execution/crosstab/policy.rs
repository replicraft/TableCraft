//! Complete runtime pivot cohorts never inherit discarded static contributors.
use super::Runtime;
use crate::{
    QueryError,
    execution::cell::CellValue,
    plan::{Expr, Relation, RelationNode},
    source::TextPolicy,
};

pub(super) struct PivotPolicy {
    fallback: TextPolicy,
    observed: TextPolicy,
    complete: bool,
}

impl PivotPolicy {
    pub(super) fn new(fallback: TextPolicy) -> Self {
        Self {
            fallback,
            observed: TextPolicy::Default,
            complete: false,
        }
    }

    pub(super) fn observe(&mut self, cell: &CellValue) -> TextPolicy {
        self.observed = self.observed.merge(cell.policy);
        self.complete |= cell.policy_complete;
        if self.complete {
            self.observed
        } else {
            self.fallback.merge(self.observed)
        }
    }
}

impl Runtime<'_, '_> {
    /// Native/base fields and fresh conversions cannot carry constructed lineage.
    /// Derived fields remain conservatively dynamic: their retained cells can
    /// come from a constructor even when their public metadata has one policy.
    pub(super) fn static_pivot_policy(
        &mut self,
        pivot: &Expr,
        source: &Relation,
    ) -> Result<bool, QueryError> {
        self.tick()?;
        match pivot {
            Expr::Field { depth: 0, .. } | Expr::CollectionItem { depth: 0, .. } => {
                self.base_policy_source(source)
            }
            Expr::CVar(value) => self.static_pivot_policy(value, source),
            Expr::Literal(_)
            | Expr::NumericLiteral(_)
            | Expr::DateLiteral(_, _)
            | Expr::TextCall(_, _)
            | Expr::Convert(_, _) => Ok(true),
            _ => Ok(false),
        }
    }

    fn base_policy_source(&mut self, source: &Relation) -> Result<bool, QueryError> {
        self.tick()?;
        match &source.node {
            RelationNode::Table { .. } => Ok(true),
            RelationNode::Cross(left, right) | RelationNode::Join { left, right, .. } => {
                let left = self.base_policy_source(left)?;
                let right = self.base_policy_source(right)?;
                Ok(left && right)
            }
            RelationNode::Derived(_) => Ok(false),
            RelationNode::Unit => Ok(true),
        }
    }
}
