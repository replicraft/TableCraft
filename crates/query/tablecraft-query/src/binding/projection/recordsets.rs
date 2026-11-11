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
            RelationNode::Table {
                recordset_projection,
                ..
            } => {
                let mut error = None;
                recordset_projection.retain(|index| {
                    if error.is_some() {
                        return false;
                    }
                    let selected = (|| {
                        for slot in &relation.slots {
                            self.binder.work()?;
                            let Some(recordset) = &slot.recordset else { continue };
                            if recordset.index != *index {
                                continue;
                            }
                            for expression in projection {
                                self.binder.work()?;
                                if matches!(expression, Expr::Recordset { origin, .. } if *origin == recordset.origin) {
                                    return Ok(true);
                                }
                            }
                        }
                        Ok(false)
                    })();
                    match selected {
                        Ok(keep) => keep,
                        Err(failure) => {
                            error = Some(failure);
                            false
                        }
                    }
                });
                if let Some(error) = error {
                    return Err(error);
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
