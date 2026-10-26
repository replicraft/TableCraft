//! Collection shape is explicit and cannot enter a scalar read accidentally.
use super::{Expr, ReadNode, ReadPlan, Relation, RelationNode};

impl ReadPlan {
    pub(crate) fn has_collections(&self) -> bool {
        match &self.node {
            ReadNode::Select(select) => {
                select
                    .projection
                    .iter()
                    .any(|value| matches!(value, Expr::Multivalue { .. }))
                    || relation_has_collections(&select.source)
            }
            ReadNode::Union { first, rest } => {
                first.has_collections() || rest.iter().any(|(_, read)| read.has_collections())
            }
            ReadNode::Crosstab(cross) => cross.read.has_collections(),
        }
    }
}

fn relation_has_collections(relation: &Relation) -> bool {
    match &relation.node {
        RelationNode::Derived(read) => read.has_collections(),
        RelationNode::Cross(left, right) | RelationNode::Join { left, right, .. } => {
            relation_has_collections(left) || relation_has_collections(right)
        }
        RelationNode::Unit | RelationNode::Table { .. } => false,
    }
}
