//! Retain metadata positions independently of pruned scalar join row positions.
use super::super::Binder;
use crate::{
    QueryError,
    plan::{Expr, ParentFilter, ParentLocation, Relation, RelationNode},
};
use tablecraft_schema::Column;

struct LocatedColumn {
    field: usize,
    location: ParentLocation,
    column: Column,
}
impl Binder<'_> {
    pub(super) fn joined_parent(
        &mut self,
        source: &Relation,
        filter: Expr,
        used: &[bool],
        depth: usize,
    ) -> Result<ParentFilter, QueryError> {
        let mut selected = Vec::new();
        self.parent_locations(source, 0, &mut 0, used, &mut selected, depth + 1)?;
        self.charge(
            selected.len()
                * (size_of::<usize>()
                    + size_of::<Column>()
                    + size_of::<ParentLocation>()
                    + size_of::<(usize, usize)>()),
        )?;
        let mut collections = Vec::new();
        let mut columns = Vec::new();
        let mut locations = Vec::new();
        let mut fields = Vec::new();
        collections
            .try_reserve_exact(selected.len())
            .map_err(|_| QueryError::Allocation)?;
        columns
            .try_reserve_exact(selected.len())
            .map_err(|_| QueryError::Allocation)?;
        locations
            .try_reserve_exact(selected.len())
            .map_err(|_| QueryError::Allocation)?;
        fields
            .try_reserve_exact(selected.len())
            .map_err(|_| QueryError::Allocation)?;
        for value in selected {
            collections.push(value.field);
            fields.push((value.field, value.field));
            columns.push(value.column);
            locations.push(value.location);
        }
        let predicate = self.parent_predicate(filter, source, &fields, depth + 1)?;
        Ok(ParentFilter {
            collections,
            columns,
            predicate,
            locations: Some(locations),
        })
    }
    fn parent_locations(
        &mut self,
        source: &Relation,
        offset: usize,
        leaf: &mut usize,
        used: &[bool],
        selected: &mut Vec<LocatedColumn>,
        depth: usize,
    ) -> Result<(), QueryError> {
        self.tick(depth)?;
        match &source.node {
            RelationNode::Cross(left, right) | RelationNode::Join { left, right, .. } => {
                self.parent_locations(left, offset, leaf, used, selected, depth + 1)?;
                self.parent_locations(
                    right,
                    offset + left.row_width(),
                    leaf,
                    used,
                    selected,
                    depth + 1,
                )?;
            }
            RelationNode::Table {
                columns,
                multivalues,
                members,
                ..
            } => {
                for (index, column) in multivalues
                    .iter()
                    .chain(members.iter().map(|member| member.column()))
                    .enumerate()
                {
                    let field = offset + columns.len() + index;
                    if used[field] {
                        self.charge(size_of::<LocatedColumn>() + column.name().len())?;
                        selected
                            .try_reserve_exact(1)
                            .map_err(|_| QueryError::Allocation)?;
                        selected.push(LocatedColumn {
                            field,
                            location: ParentLocation {
                                leaf: *leaf,
                                member: index,
                            },
                            column: column.clone(),
                        });
                    }
                }
                *leaf += 1;
            }
            RelationNode::Unit | RelationNode::Derived(_) => *leaf += 1,
        }
        Ok(())
    }
}
