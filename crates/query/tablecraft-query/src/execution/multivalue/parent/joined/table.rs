use super::{ParentRow, Runtime};
use crate::{
    QueryError,
    plan::{ParentFilter, Relation, RelationNode},
    profile::SOURCE_SCHEMA_POLICY,
    source::MultivalueList,
};
use tablecraft_core::values::Value;
use tablecraft_schema::validate_row;

impl Runtime<'_> {
    pub(super) fn parent_table(
        &mut self,
        relation: &Relation,
        parent: &ParentFilter,
        selected: &[usize],
        store: &mut Vec<Vec<Value>>,
    ) -> Result<Vec<ParentRow>, QueryError> {
        let RelationNode::Table {
            name,
            columns,
            unavailable,
            projection,
            multivalues,
            members,
            expansion,
        } = &relation.node
        else {
            return Err(QueryError::Source);
        };
        if !expansion.is_empty() {
            return Err(QueryError::Unsupported);
        }
        if self.source.columns(name)? != columns
            || self.source.unavailable_columns(name)? != unavailable
            || self.source.multivalue_columns(name)? != multivalues
            || self.source.collection_members(name)? != members
        {
            return Err(QueryError::SchemaChanged);
        }
        let locations = parent.locations.as_ref().ok_or(QueryError::Source)?;
        self.budget
            .charge_allocation(selected.len() as u64, size_of::<usize>() as u64)?;
        let mut requested = Vec::new();
        requested
            .try_reserve_exact(selected.len())
            .map_err(|_| QueryError::Allocation)?;
        requested.extend(selected.iter().map(|index| locations[*index].member));
        let (values, named) = self.collection_requests(&requested, multivalues.len())?;
        let source = self.source;
        let mut rows = Vec::new();
        let mut failure: Option<QueryError> = None;
        let mut visitor = |scalars: crate::source::ProjectedRow<'_>,
                           lists: &[&dyn MultivalueList]| {
            if let Some(error) = &failure {
                return Err(error.clone());
            }
            let result = (|| {
                self.tick()?;
                if lists.len() != selected.len() {
                    return Err(QueryError::Source);
                }
                for (position, index) in requested.iter().enumerate() {
                    if let Some(member) = index
                        .checked_sub(multivalues.len())
                        .and_then(|index| members.get(index))
                    {
                        for (other, index) in requested[..position].iter().enumerate() {
                            self.tick()?;
                            if let Some(other_member) = index
                                .checked_sub(multivalues.len())
                                .and_then(|index| members.get(index))
                                && SOURCE_SCHEMA_POLICY.names.equals(
                                    member.collection().as_str(),
                                    other_member.collection().as_str(),
                                )
                                && lists[position].len() != lists[other].len()
                            {
                                return Err(QueryError::Source);
                            }
                        }
                    }
                }
                let values = self.copy_projected_row(columns, projection, scalars)?;
                let mut handles = self.parent_handles(parent.collections.len())?;
                for (position, global) in selected.iter().enumerate() {
                    let list = lists[position];
                    let column = parent.columns.get(*global).ok_or(QueryError::Source)?;
                    self.budget
                        .charge_allocation(list.len() as u64, size_of::<Value>() as u64)?;
                    let mut owned = Vec::new();
                    owned
                        .try_reserve_exact(list.len())
                        .map_err(|_| QueryError::Allocation)?;
                    for index in 0..list.len() {
                        self.tick()?;
                        let value = list.get(index).ok_or(QueryError::Source)?;
                        validate_row(std::slice::from_ref(column), std::slice::from_ref(value))?;
                        owned.push(self.copy(value)?);
                    }
                    handles[*global] = Some(store.len());
                    self.push(store, owned)?;
                }
                self.push(
                    &mut rows,
                    ParentRow {
                        values,
                        lists: handles,
                    },
                )
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
