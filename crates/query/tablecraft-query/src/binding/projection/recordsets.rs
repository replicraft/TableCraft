//! Prune only source recordsets; written derived projections retain eager work.
use super::Pruner;
use crate::{
    QueryError,
    plan::{Expr, Relation, RelationNode},
};

impl Pruner<'_, '_> {
    pub(super) fn recordsets(
        &mut self,
        relation: &mut Relation,
        projection: &[Expr],
    ) -> Result<(), QueryError> {
        self.tick()?;
        match &mut relation.node {
            RelationNode::Table { recordsets, .. } => {
                for (index, entry) in recordsets.iter_mut().enumerate() {
                    let mut selected = false;
                    for slot in &relation.slots {
                        self.binder.work()?;
                        let Some(recordset) = &slot.recordset else {
                            continue;
                        };
                        if recordset.index != index {
                            continue;
                        }
                        for expression in projection {
                            self.binder.work()?;
                            selected |= matches!(expression, Expr::Recordset { origin, .. } if *origin == recordset.origin);
                        }
                    }
                    entry.projected = selected;
                }
            }
            RelationNode::Cross(left, right) | RelationNode::Join { left, right, .. } => {
                self.recordsets(left, projection)?;
                self.recordsets(right, projection)?;
            }
            // The derived read is pruned from its own written projection in
            // the scalar traversal. Removing it here would suppress eager work.
            RelationNode::Derived { .. } | RelationNode::Unit => {}
        }
        Ok(())
    }
}
