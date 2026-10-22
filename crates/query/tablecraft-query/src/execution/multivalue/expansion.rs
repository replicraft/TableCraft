//! Shared child alignment and empty-parent expansion for collection scans.
use super::Runtime;
use crate::{QueryError, profile::SOURCE_SCHEMA_POLICY, source::MultivalueList};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_schema::{CollectionMember, Column, validate_row};

pub(super) struct Expansion<'a> {
    columns: Vec<&'a Column>,
    group_for: Vec<usize>,
    groups: usize,
}
impl Runtime<'_> {
    pub(super) fn collection_expansion<'a>(
        &mut self,
        multivalues: &'a [Column],
        members: &'a [CollectionMember],
        expansion: &[usize],
    ) -> Result<Expansion<'a>, QueryError> {
        let mut groups: Vec<&str> = Vec::new();
        let mut group_for = Vec::new();
        let mut columns = Vec::new();
        self.budget.charge_allocation(
            expansion.len() as u64,
            (size_of::<&str>() + size_of::<usize>() + size_of::<&Column>()) as u64,
        )?;
        groups
            .try_reserve_exact(expansion.len())
            .map_err(|_| QueryError::Allocation)?;
        group_for
            .try_reserve_exact(expansion.len())
            .map_err(|_| QueryError::Allocation)?;
        columns
            .try_reserve_exact(expansion.len())
            .map_err(|_| QueryError::Allocation)?;
        for index in expansion {
            self.tick()?;
            columns.push(if *index < multivalues.len() {
                &multivalues[*index]
            } else {
                members
                    .get(*index - multivalues.len())
                    .ok_or(QueryError::Source)?
                    .column()
            });
            let collection = if *index < multivalues.len() {
                multivalues[*index].name()
            } else {
                members
                    .get(*index - multivalues.len())
                    .ok_or(QueryError::Source)?
                    .collection()
                    .as_str()
            };
            let group = if let Some(group) = groups
                .iter()
                .position(|name| SOURCE_SCHEMA_POLICY.names.equals(name, collection))
            {
                group
            } else {
                groups.push(collection);
                groups.len() - 1
            };
            group_for.push(group);
        }
        Ok(Expansion {
            columns,
            group_for,
            groups: groups.len(),
        })
    }
    pub(super) fn expand_collection_row(
        &mut self,
        scalar: &[ExpressionValue],
        layout: &Expansion<'_>,
        lists: &[&dyn MultivalueList],
        rows: &mut Vec<Vec<ExpressionValue>>,
    ) -> Result<(), QueryError> {
        if lists.len() != layout.columns.len() {
            return Err(QueryError::Source);
        }
        self.budget.charge_allocation(
            layout.groups as u64,
            (size_of::<Option<usize>>() + size_of::<usize>()) as u64,
        )?;
        let mut lengths = Vec::new();
        lengths
            .try_reserve_exact(layout.groups)
            .map_err(|_| QueryError::Allocation)?;
        lengths.resize(layout.groups, None);
        let mut positions = Vec::new();
        positions
            .try_reserve_exact(layout.groups)
            .map_err(|_| QueryError::Allocation)?;
        positions.resize(layout.groups, 0_usize);
        for (list, group) in lists.iter().zip(&layout.group_for) {
            self.tick()?;
            if lengths[*group].is_some_and(|length| length != list.len()) {
                return Err(QueryError::Source);
            }
            lengths[*group] = Some(list.len());
        }
        loop {
            self.tick()?;
            let mut row = self.allocate_expression_row(scalar.len() + layout.columns.len())?;
            for value in scalar {
                row.push(self.copy_expression(value)?);
            }
            for ((list, column), group) in lists.iter().zip(&layout.columns).zip(&layout.group_for)
            {
                let length = lengths[*group].ok_or(QueryError::Source)?;
                let value = if length == 0 {
                    &Value::Null
                } else {
                    let value = list.get(positions[*group]).ok_or(QueryError::Source)?;
                    validate_row(std::slice::from_ref(*column), std::slice::from_ref(value))?;
                    value
                };
                row.push(ExpressionValue::Value(self.copy(value)?));
            }
            self.push(rows, row)?;
            let mut carry = true;
            for group in (0..layout.groups).rev() {
                self.tick()?;
                positions[group] += 1;
                if positions[group] < lengths[group].ok_or(QueryError::Source)?.max(1) {
                    carry = false;
                    break;
                }
                positions[group] = 0;
            }
            if carry {
                break;
            }
        }
        Ok(())
    }
}
