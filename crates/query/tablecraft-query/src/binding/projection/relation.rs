use super::{FieldMap, Pruner};
use crate::{
    QueryError,
    plan::{Relation, RelationNode},
};

impl Pruner<'_, '_> {
    pub(super) fn relation(
        &mut self,
        relation: &mut Relation,
        needed: &[bool],
        outer: &[&[Option<usize>]],
    ) -> Result<FieldMap, QueryError> {
        self.tick()?;
        if needed.len() != relation.row_width() {
            return Err(QueryError::Source);
        }
        let map = self.mapping(needed)?;
        match &mut relation.node {
            RelationNode::Table {
                projection,
                columns,
                expansion,
                ..
            } => {
                projection.retain(|index| needed[*index]);
                expansion.retain(|index| needed[columns.len() + index]);
            }
            RelationNode::Derived { read, expansion } => {
                let base = read
                    .output_types
                    .iter()
                    .filter(|kind| !kind.is_multivalue())
                    .count();
                if needed[..base].iter().any(|needed| !needed) {
                    return Err(QueryError::Source);
                }
                expansion.retain(|index| needed[base + index]);
                self.read(read, &[])?;
            }
            RelationNode::Cross(left, right) => {
                let split = left.row_width();
                self.relation(left, &needed[..split], outer)?;
                self.relation(right, &needed[split..], outer)?;
            }
            RelationNode::Join {
                left,
                right,
                condition,
                ..
            } => {
                let split = left.row_width();
                self.relation(left, &needed[..split], outer)?;
                self.relation(right, &needed[split..], outer)?;
                let mut scopes = self.vector(outer.len() + 1)?;
                scopes.push(map.as_slice());
                scopes.extend_from_slice(outer);
                self.expression(condition, &scopes)?;
            }
            RelationNode::Unit => {}
        }
        let mut index = 0;
        relation.slots.retain(|slot| {
            if !slot.available {
                true
            } else {
                let keep = needed[index];
                index += 1;
                keep
            }
        });
        Ok(map)
    }
}
