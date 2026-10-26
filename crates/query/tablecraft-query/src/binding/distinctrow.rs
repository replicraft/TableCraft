//! DISTINCTROW compares complete selected source records, including hidden fields.
use super::{Binder, projection::referenced_fields};
use crate::{
    QueryError,
    plan::{Expr, Relation, RelationNode},
};

impl Binder<'_> {
    pub(super) fn record_keys(
        &mut self,
        source: &Relation,
        projection: &[Expr],
        aggregate: bool,
        depth: usize,
    ) -> Result<Vec<Expr>, QueryError> {
        let used = referenced_fields(projection, source.row_width(), self)?;
        let mut leaves = Vec::new();
        self.record_sources(source, 0, &mut leaves, depth + 1)?;
        let selected = leaves
            .iter()
            .filter(|(_, offset, width)| used[*offset..offset + width].iter().any(|v| *v))
            .count();
        // Record deduplication needs a nonempty proper subset of source owners.
        if leaves.len() <= 1 || selected == 0 || selected == leaves.len() {
            return Ok(Vec::new());
        }
        if aggregate {
            return Err(QueryError::Unsupported);
        }
        // Already-bound projections evaluate before record deduplication. They
        // need no structural identity or purity guarantee: only source captures
        // determine which complete records participate in the key.
        self.charge(source.row_width() * size_of::<Expr>())?;
        let mut keys = Vec::new();
        keys.try_reserve_exact(source.row_width())
            .map_err(|_| QueryError::Allocation)?;
        for (relation, offset, width) in leaves {
            if !used[offset..offset + width].iter().any(|v| *v) {
                continue;
            }
            // A derived/named leaf's complete logical record is its output row,
            // including columns hidden by this enclosing SELECT. Native records
            // with unavailable scalar/collection payloads remain unqualified.
            if relation.slots.iter().any(|slot| !slot.available) {
                return Err(QueryError::Unsupported);
            }
            keys.extend((offset..offset + width).map(|index| Expr::Field {
                depth: 0,
                index,
                text_policy: relation.slots[index - offset].text_policy,
            }));
        }
        Ok(keys)
    }

    fn record_sources<'a>(
        &mut self,
        relation: &'a Relation,
        offset: usize,
        leaves: &mut Vec<(&'a Relation, usize, usize)>,
        depth: usize,
    ) -> Result<(), QueryError> {
        self.tick(depth)?;
        match &relation.node {
            RelationNode::Unit => {}
            RelationNode::Table { .. } | RelationNode::Derived { .. } => {
                self.charge(size_of::<(&Relation, usize, usize)>())?;
                leaves.try_reserve(1).map_err(|_| QueryError::Allocation)?;
                leaves.push((relation, offset, relation.row_width()));
            }
            RelationNode::Cross(left, right) | RelationNode::Join { left, right, .. } => {
                self.record_sources(left, offset, leaves, depth + 1)?;
                self.record_sources(right, offset + left.row_width(), leaves, depth + 1)?;
            }
        }
        Ok(())
    }
}
