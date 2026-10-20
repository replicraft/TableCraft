use super::Runtime;
use crate::{
    QueryError,
    profile::SOURCE_SCHEMA_POLICY,
    source::{MultivalueList, ProjectedRow},
};
use tablecraft_core::values::Value;
use tablecraft_schema::{CollectionMember, Column, validate_row};
mod parent;

impl Runtime<'_> {
    pub(super) fn collection_requests<'a>(
        &mut self,
        selected: &'a [usize],
        values: usize,
    ) -> Result<(&'a [usize], Vec<usize>), QueryError> {
        let split = selected.partition_point(|index| *index < values);
        self.budget
            .charge_allocation((selected.len() - split) as u64, size_of::<usize>() as u64)?;
        let mut named = Vec::new();
        named
            .try_reserve_exact(selected.len() - split)
            .map_err(|_| QueryError::Allocation)?;
        for index in &selected[split..] {
            named.push(index - values);
        }
        Ok((&selected[..split], named))
    }
    pub(super) fn multivalue_rows(
        &mut self,
        name: &str,
        columns: &[Column],
        projection: &[usize],
        multivalues: &[Column],
        members: &[CollectionMember],
        expansion: &[usize],
    ) -> Result<Vec<Vec<Value>>, QueryError> {
        let (values, named) = self.collection_requests(expansion, multivalues.len())?;
        let mut groups: Vec<&str> = Vec::new();
        let mut group_for = Vec::new();
        self.budget.charge_allocation(
            expansion.len() as u64,
            (size_of::<&str>() + size_of::<usize>()) as u64,
        )?;
        groups
            .try_reserve_exact(expansion.len())
            .map_err(|_| QueryError::Allocation)?;
        group_for
            .try_reserve_exact(expansion.len())
            .map_err(|_| QueryError::Allocation)?;
        for index in expansion {
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
        let source = self.source;
        let mut rows = Vec::new();
        let mut failure: Option<QueryError> = None;
        let mut visitor = |scalars: ProjectedRow<'_>, lists: &[&dyn MultivalueList]| {
            if let Some(error) = &failure {
                return Err(error.clone());
            }
            let result = (|| {
                self.tick()?;
                if lists.len() != expansion.len() {
                    return Err(QueryError::Source);
                }
                self.budget.charge_allocation(
                    groups.len() as u64,
                    (size_of::<Option<usize>>() + size_of::<usize>()) as u64,
                )?;
                let mut lengths = Vec::new();
                lengths
                    .try_reserve_exact(groups.len())
                    .map_err(|_| QueryError::Allocation)?;
                lengths.resize(groups.len(), None);
                let mut positions = Vec::new();
                positions
                    .try_reserve_exact(groups.len())
                    .map_err(|_| QueryError::Allocation)?;
                positions.resize(groups.len(), 0_usize);
                for (list, group) in lists.iter().zip(&group_for) {
                    self.tick()?;
                    if lengths[*group].is_some_and(|length| length != list.len()) {
                        return Err(QueryError::Source);
                    }
                    lengths[*group] = Some(list.len());
                }
                let scalar = self.copy_projected_row(columns, projection, scalars)?;
                loop {
                    self.tick()?;
                    let mut row = self.allocate_row(scalar.len() + expansion.len())?;
                    for value in &scalar {
                        row.push(self.copy(value)?);
                    }
                    for ((list, index), group) in lists.iter().zip(expansion).zip(&group_for) {
                        let column = if *index < multivalues.len() {
                            &multivalues[*index]
                        } else {
                            members
                                .get(*index - multivalues.len())
                                .ok_or(QueryError::Source)?
                                .column()
                        };
                        let length = lengths[*group].ok_or(QueryError::Source)?;
                        let value = if length == 0 {
                            &Value::Null
                        } else {
                            let value = list.get(positions[*group]).ok_or(QueryError::Source)?;
                            validate_row(
                                std::slice::from_ref(column),
                                std::slice::from_ref(value),
                            )?;
                            value
                        };
                        row.push(self.copy(value)?);
                    }
                    self.push(&mut rows, row)?;
                    let mut carry = true;
                    for group in (0..groups.len()).rev() {
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
            })();
            if let Err(error) = &result {
                failure = Some(error.clone());
            }
            result
        };
        let result = source.scan_collection_members(name, projection, values, &named, &mut visitor);
        failure.map_or(result.map(|()| rows), Err)
    }
}
