//! Leaf requests distinguish combined output slots from parent predicate requests.
use super::{CollectionFrame, JoinedFrame, Requests, Runtime};
use crate::{
    MultivalueValue, QueryError,
    plan::{Relation, RelationNode},
    profile::SOURCE_SCHEMA_POLICY,
    source::{MultivalueList, ProjectedRow},
};
use tablecraft_core::{limits::Resource, values::Value};
use tablecraft_schema::{CollectionMember, Column, validate_row};

struct Request {
    native: usize,
    output: Option<usize>,
    criterion: Option<usize>,
}

impl Runtime<'_, '_> {
    pub(super) fn joined_collection_table(
        &mut self,
        relation: &Relation,
        requests: &Requests<'_>,
        base: usize,
        leaf: usize,
    ) -> Result<Vec<JoinedFrame>, QueryError> {
        self.check_table_schema(&relation.node)?;
        let RelationNode::Table {
            namespace,
            name,
            columns,
            projection,
            multivalues,
            members,
            ..
        } = &relation.node
        else {
            return Err(QueryError::Unsupported);
        };
        let total = multivalues
            .len()
            .checked_add(members.len())
            .ok_or(QueryError::StructureLimit)?;
        self.budget
            .charge_allocation(total as u64, size_of::<Request>() as u64)?;
        let mut needed = Vec::new();
        needed
            .try_reserve_exact(total)
            .map_err(|_| QueryError::Allocation)?;
        for native in 0..total {
            self.tick()?;
            let output = if native < multivalues.len() {
                let index = base.checked_add(native).ok_or(QueryError::StructureLimit)?;
                let mut shown = false;
                for selected in requests.selected {
                    self.tick()?;
                    shown |= *selected == index;
                }
                shown.then_some(index)
            } else {
                None
            };
            let mut criterion = None;
            if let Some(parent) = requests.parent {
                let locations = parent.locations.as_ref().ok_or(QueryError::Source)?;
                for (position, location) in locations.iter().enumerate() {
                    self.tick()?;
                    if location.leaf == leaf
                        && location.member == native
                        && criterion.replace(position).is_some()
                    {
                        return Err(QueryError::Source);
                    }
                }
            }
            if output.is_some() || criterion.is_some() {
                needed.push(Request {
                    native,
                    output,
                    criterion,
                });
            }
        }
        if needed.is_empty() {
            let mut result = Vec::new();
            for scalars in self.relation(relation, &[])? {
                self.push(
                    &mut result,
                    JoinedFrame {
                        frame: CollectionFrame {
                            scalars,
                            lists: Vec::new(),
                            keys: Vec::new(),
                        },
                        criteria: Vec::new(),
                    },
                )?;
            }
            return Ok(result);
        }
        self.budget
            .charge_allocation(needed.len() as u64, size_of::<usize>() as u64)?;
        let mut indices = Vec::new();
        indices
            .try_reserve_exact(needed.len())
            .map_err(|_| QueryError::Allocation)?;
        indices.extend(needed.iter().map(|request| request.native));
        let (values, named) = self.collection_requests(&indices, multivalues.len())?;
        let source = self.namespace_source(*namespace)?;
        let mut rows = Vec::new();
        let mut failure: Option<QueryError> = None;
        let mut visitor = |scalars: ProjectedRow<'_>, lists: &[&dyn MultivalueList]| {
            if let Some(error) = &failure {
                return Err(error.clone());
            }
            let result = (|| {
                self.tick()?;
                if lists.len() != needed.len() {
                    return Err(QueryError::Source);
                }
                self.check_joined_list_views(&needed, multivalues, members, lists)?;
                let scalars = self.copy_projected_row(columns, projection, scalars)?;
                let mut owned = Vec::new();
                let mut criteria = Vec::new();
                for (request, values) in needed.iter().zip(lists) {
                    self.tick()?;
                    if let Some(position) = request.criterion {
                        let copied = self.copy_joined_criterion(*values)?;
                        self.push(&mut criteria, (position, Some(copied)))?;
                    }
                    if let Some(index) = request.output {
                        let copied = self.copy_borrowed_collection(*values)?;
                        self.push(&mut owned, (index, Some(copied)))?;
                    }
                }
                self.push(
                    &mut rows,
                    JoinedFrame {
                        frame: CollectionFrame {
                            scalars,
                            lists: owned,
                            keys: Vec::new(),
                        },
                        criteria,
                    },
                )
            })();
            if let Err(error) = &result {
                failure = Some(error.clone());
            }
            result
        };
        let scan = source.scan_collection_members(name, projection, values, &named, &mut visitor);
        failure.map_or(scan.map(|()| rows), Err)
    }

    fn check_joined_list_views(
        &mut self,
        requested: &[Request],
        columns: &[Column],
        members: &[CollectionMember],
        lists: &[&dyn MultivalueList],
    ) -> Result<(), QueryError> {
        for (position, (request, list)) in requested.iter().zip(lists).enumerate() {
            self.tick()?;
            let column = if request.native < columns.len() {
                &columns[request.native]
            } else {
                let member = members
                    .get(request.native - columns.len())
                    .ok_or(QueryError::Source)?;
                for (other, earlier) in requested[..position].iter().enumerate() {
                    self.tick()?;
                    if let Some(index) = earlier.native.checked_sub(columns.len()) {
                        let earlier = members.get(index).ok_or(QueryError::Source)?;
                        if SOURCE_SCHEMA_POLICY
                            .names
                            .equals(member.collection().as_str(), earlier.collection().as_str())
                            && list.len() != lists[other].len()
                        {
                            return Err(QueryError::Source);
                        }
                    }
                }
                member.column()
            };
            for child in 0..list.len() {
                self.tick()?;
                let value = list.get(child).ok_or(QueryError::Source)?;
                validate_row(std::slice::from_ref(column), std::slice::from_ref(value))
                    .map_err(|_| QueryError::Source)?;
            }
        }
        Ok(())
    }

    fn copy_joined_criterion(
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
            // Predicate operands keep the original scalar copy/admission rules;
            // only projected stored lists use the IEEE-preserving data helper.
            let value = list.get(index).ok_or(QueryError::Source)?;
            values.push(self.copy(value)?);
        }
        Ok(MultivalueValue { values })
    }
}
