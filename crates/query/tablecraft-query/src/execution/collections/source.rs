//! One complete list callback per parent, with hidden criteria kept independent.
use super::{CollectionFrame, Runtime};
use crate::{
    MultivalueValue, QueryError,
    execution::multivalue::ParentCollections,
    plan::{Expr, RelationNode, SelectPlan},
};
use std::sync::Arc;
use tablecraft_core::{limits::Resource, values::Value};
use tablecraft_db::{MultivalueList, OwnedRecordset, ProjectedRow};
use tablecraft_schema::validate_row;

impl Runtime<'_, '_> {
    pub(super) fn collection_input(
        &mut self,
        select: &SelectPlan,
    ) -> Result<Vec<CollectionFrame>, QueryError> {
        if matches!(
            select.source.node,
            RelationNode::Cross(..) | RelationNode::Join { .. }
        ) {
            return self.joined_collection_input(select);
        }
        if let RelationNode::Table { expansion, .. } = &select.source.node
            && !expansion.is_empty()
        {
            // Displayed members need the current expanded scalar frame before
            // hidden predicates can run. The unexpanded path stays unchanged.
            return self.joined_collection_input(select);
        }
        if let RelationNode::Derived { read, expansion } = &select.source.node {
            if !expansion.is_empty() || select.parent.is_some() {
                return self.joined_collection_input(select);
            }
            return self.collection_read(read);
        }
        self.collection_table(select)
    }

    fn collection_table(
        &mut self,
        select: &SelectPlan,
    ) -> Result<Vec<CollectionFrame>, QueryError> {
        self.check_table_schema(&select.source.node)?;
        let RelationNode::Table {
            namespace,
            name,
            columns,
            projection,
            multivalues,
            members,
            recordsets,
            recordset_projection,
            ..
        } = &select.source.node
        else {
            return Err(QueryError::Unsupported);
        };
        let total = multivalues.len() + members.len();
        self.budget
            .charge_allocation(total as u64, size_of::<bool>() as u64)?;
        let mut selected = Vec::new();
        selected
            .try_reserve_exact(total)
            .map_err(|_| QueryError::Allocation)?;
        selected.resize(total, false);
        for expression in &select.projection {
            self.tick()?;
            if let Expr::Multivalue { index, .. } = expression {
                *selected.get_mut(*index).ok_or(QueryError::Source)? = true;
            }
        }
        self.budget
            .charge_allocation(total as u64, size_of::<usize>() as u64)?;
        let mut requested = Vec::new();
        requested
            .try_reserve_exact(total)
            .map_err(|_| QueryError::Allocation)?;
        for (index, shown) in selected.iter().enumerate() {
            self.tick()?;
            let needed = if let Some(parent) = &select.parent {
                let mut needed = false;
                for criterion in &parent.collections {
                    self.tick()?;
                    needed |= *criterion == index;
                }
                needed
            } else {
                false
            };
            if *shown || needed {
                requested.push(index);
            }
        }
        let (values, named) = self.collection_requests(&requested, multivalues.len())?;
        let source = self.namespace_source(*namespace)?;
        let mut rows = Vec::new();
        let mut failure: Option<QueryError> = None;
        let mut visitor = |scalars: ProjectedRow<'_>,
                           lists: &[&dyn MultivalueList],
                           children: &[Arc<OwnedRecordset>]| {
            if let Some(error) = &failure {
                return Err(*error);
            }
            let result = (|| {
                self.tick()?;
                if lists.len() != requested.len() || children.len() != recordset_projection.len() {
                    return Err(QueryError::Source);
                }
                let mut owned_recordsets = Vec::new();
                for (index, value) in recordset_projection.iter().zip(children) {
                    self.tick()?;
                    let expected = recordsets.get(*index).ok_or(QueryError::Source)?;
                    let mut origin = None;
                    for slot in &select.source.slots {
                        self.tick()?;
                        if let Some(recordset) = &slot.recordset
                            && recordset.index == *index
                        {
                            origin = Some(recordset.origin);
                            break;
                        }
                    }
                    let owned = self.retain_recordset(value, expected.schema())?;
                    self.push(
                        &mut owned_recordsets,
                        (origin.ok_or(QueryError::Source)?, Some(owned)),
                    )?;
                }
                // Validate every requested stored child, including a malformed
                // suffix after a matching criterion, before admitting its parent.
                for (index, list) in requested.iter().zip(lists) {
                    let column = if *index < multivalues.len() {
                        &multivalues[*index]
                    } else {
                        members
                            .get(*index - multivalues.len())
                            .ok_or(QueryError::Source)?
                            .column()
                    };
                    for child in 0..list.len() {
                        self.tick()?;
                        let value = list.get(child).ok_or(QueryError::Source)?;
                        validate_row(std::slice::from_ref(column), std::slice::from_ref(value))
                            .map_err(|_| QueryError::Source)?;
                    }
                }
                let scalars = self.copy_projected_row(columns, projection, scalars)?;
                if let Some(parent) = &select.parent {
                    self.budget.charge_allocation(
                        parent.collections.len() as u64,
                        size_of::<&dyn MultivalueList>() as u64,
                    )?;
                    let mut criteria = Vec::new();
                    criteria
                        .try_reserve_exact(parent.collections.len())
                        .map_err(|_| QueryError::Allocation)?;
                    for wanted in &parent.collections {
                        self.tick()?;
                        let position = requested
                            .binary_search(wanted)
                            .map_err(|_| QueryError::Source)?;
                        criteria.push(lists[position]);
                    }
                    let collections = ParentCollections::borrowed(
                        &parent.collections,
                        &parent.columns,
                        &criteria,
                    );
                    if !self
                        .parent_truth(&parent.predicate, &collections, &scalars, &[])?
                        .is_true()
                    {
                        return Ok(());
                    }
                }
                let mut owned_lists = Vec::new();
                for (index, list) in requested.iter().zip(lists) {
                    self.tick()?;
                    if selected[*index] {
                        let owned = self.copy_borrowed_collection(*list)?;
                        self.push(&mut owned_lists, (*index, Some(owned)))?;
                    }
                }
                self.push(
                    &mut rows,
                    CollectionFrame {
                        scalars,
                        lists: owned_lists,
                        recordsets: owned_recordsets,
                        keys: Vec::new(),
                    },
                )
            })();
            if let Err(error) = &result {
                failure = Some(*error);
            }
            result
        };
        let result = source.scan_structured(
            name,
            projection,
            values,
            &named,
            recordset_projection,
            &mut visitor,
        );
        failure.map_or(result.map(|()| rows), Err)
    }

    pub(super) fn copy_borrowed_collection(
        &mut self,
        list: &dyn MultivalueList,
    ) -> Result<MultivalueValue, QueryError> {
        self.tick()?;
        self.budget.charge(Resource::Rows, list.len() as u64)?;
        self.budget
            .charge_allocation(list.len() as u64, size_of::<Value>() as u64)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(list.len())
            .map_err(|_| QueryError::Allocation)?;
        for index in 0..list.len() {
            let value = list.get(index).ok_or(QueryError::Source)?;
            values.push(self.copy_stored_list_value(value)?);
        }
        Ok(MultivalueValue { values })
    }
}
