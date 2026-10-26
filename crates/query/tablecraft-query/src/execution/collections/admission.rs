//! Prove the complete typed shape before requesting any parent/child records.
use super::Runtime;
use crate::{
    QueryError,
    plan::{ReadNode, ReadPlan, Relation, RelationNode},
    profile::MAX_RUNTIME_READ_DEPTH,
};

impl Runtime<'_, '_> {
    pub(in crate::execution) fn check_collection_read(
        &mut self,
        plan: &ReadPlan,
        depth: usize,
    ) -> Result<(), QueryError> {
        self.check_collection_depth(depth)?;
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
        if let Some(parent) = &select.parent {
            match &select.source.node {
                RelationNode::Derived(_) => return Err(QueryError::Unsupported),
                RelationNode::Cross(..) | RelationNode::Join { .. }
                    if parent
                        .locations
                        .as_ref()
                        .is_none_or(|locations| locations.len() != parent.collections.len()) =>
                {
                    return Err(QueryError::Source);
                }
                _ => {}
            }
        }
        self.check_collection_relation(&select.source, depth + 1)
    }

    fn check_collection_depth(&mut self, depth: usize) -> Result<(), QueryError> {
        self.tick()?;
        if depth > MAX_RUNTIME_READ_DEPTH {
            return Err(QueryError::StructureLimit);
        }
        Ok(())
    }

    fn check_collection_relation(
        &mut self,
        relation: &Relation,
        depth: usize,
    ) -> Result<(), QueryError> {
        self.check_collection_depth(depth)?;
        match &relation.node {
            RelationNode::Table { expansion, .. } => {
                if !expansion.is_empty() {
                    return Err(QueryError::Unsupported);
                }
                self.check_table_schema(&relation.node)
            }
            RelationNode::Derived(read) if read.has_collections() => {
                self.check_collection_read(read, depth + 1)
            }
            RelationNode::Derived(read) => self.check_collection_scalar_read(read, depth + 1),
            RelationNode::Cross(left, right) | RelationNode::Join { left, right, .. } => {
                self.check_collection_relation(left, depth + 1)?;
                self.check_collection_relation(right, depth + 1)
            }
            RelationNode::Unit => Err(QueryError::Unsupported),
        }
    }

    // A scalar-only derived leaf executes the ordinary buffered read service.
    // Inspect its input schemas without imposing list-output restrictions on it.
    fn check_collection_scalar_read(
        &mut self,
        read: &ReadPlan,
        depth: usize,
    ) -> Result<(), QueryError> {
        self.check_collection_depth(depth)?;
        match &read.node {
            ReadNode::Select(select) => {
                self.check_collection_scalar_relation(&select.source, depth + 1)
            }
            ReadNode::Union { first, rest } => {
                self.check_collection_scalar_read(first, depth + 1)?;
                for (_, read) in rest {
                    self.check_collection_scalar_read(read, depth + 1)?;
                }
                Ok(())
            }
            ReadNode::Crosstab(cross) => self.check_collection_scalar_read(&cross.read, depth + 1),
        }
    }

    fn check_collection_scalar_relation(
        &mut self,
        relation: &Relation,
        depth: usize,
    ) -> Result<(), QueryError> {
        self.check_collection_depth(depth)?;
        match &relation.node {
            RelationNode::Table { .. } => self.check_table_schema(&relation.node),
            RelationNode::Derived(read) => self.check_collection_scalar_read(read, depth + 1),
            RelationNode::Cross(left, right) | RelationNode::Join { left, right, .. } => {
                self.check_collection_scalar_relation(left, depth + 1)?;
                self.check_collection_scalar_relation(right, depth + 1)
            }
            RelationNode::Unit => Ok(()),
        }
    }
}
