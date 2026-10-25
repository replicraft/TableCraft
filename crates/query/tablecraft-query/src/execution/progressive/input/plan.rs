//! Metadata-only linear continuation collection; no row/discovery fallback.
use super::{Frame, Input};
use crate::{
    QueryError,
    execution::Runtime,
    plan::{Relation, RelationNode},
    profile::MAX_BIND_NODES,
};

impl Runtime<'_, '_> {
    pub(in crate::execution::progressive) fn progressive_input<'p>(
        &mut self,
        source: &'p Relation,
    ) -> Result<Input<'p>, QueryError> {
        let mut leaf = source;
        let mut frames = Vec::new();
        loop {
            self.tick()?;
            match &leaf.node {
                RelationNode::Table { expansion, .. } if expansion.is_empty() => {
                    return Ok(Input { leaf, frames });
                }
                RelationNode::Derived(read) => {
                    let select = self.progressive_inner_select(read)?;
                    if frames.len() == MAX_BIND_NODES {
                        return Err(QueryError::StructureLimit);
                    }
                    if frames.len() == frames.capacity() {
                        let additional = frames.len().max(1).min(MAX_BIND_NODES - frames.len());
                        self.budget
                            .charge_allocation(additional as u64, size_of::<Frame<'_>>() as u64)?;
                        frames
                            .try_reserve_exact(additional)
                            .map_err(|_| QueryError::Allocation)?;
                    }
                    frames.push(Frame {
                        read,
                        select,
                        delivered: 0,
                    });
                    leaf = &select.source;
                }
                _ => return Err(QueryError::Unsupported),
            }
        }
    }
}
