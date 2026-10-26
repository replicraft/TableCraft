//! Prove the complete typed shape before requesting any parent/child records.
use super::Runtime;
use crate::{
    QueryError,
    plan::{ReadNode, ReadPlan, RelationNode},
    profile::MAX_RUNTIME_READ_DEPTH,
};

impl Runtime<'_, '_> {
    pub(in crate::execution) fn check_collection_read(
        &mut self,
        plan: &ReadPlan,
        depth: usize,
    ) -> Result<(), QueryError> {
        self.tick()?;
        if depth > MAX_RUNTIME_READ_DEPTH {
            return Err(QueryError::StructureLimit);
        }
        let ReadNode::Select(select) = &plan.node else {
            return Err(QueryError::Unsupported);
        };
        if select.aggregate
            || select.having.is_some()
            || !select.group.is_empty()
            || !select.record_keys.is_empty()
            || plan.distinct
        {
            return Err(QueryError::Unsupported);
        }
        match &select.source.node {
            RelationNode::Table { expansion, .. } => {
                if !expansion.is_empty() {
                    return Err(QueryError::Unsupported);
                }
                self.check_table_schema(&select.source.node)?;
            }
            RelationNode::Derived(read) => {
                if select.parent.is_some() {
                    return Err(QueryError::Unsupported);
                }
                self.check_collection_read(read, depth + 1)?;
            }
            RelationNode::Unit | RelationNode::Cross(..) | RelationNode::Join { .. } => {
                return Err(QueryError::Unsupported);
            }
        }
        Ok(())
    }
}
