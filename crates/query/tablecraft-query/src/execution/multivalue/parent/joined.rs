//! Scalar join rows carry private handles to complete owned collection lists.
use super::{ParentCollections, Runtime};
use crate::execution::cell::CellValue;
use crate::execution::context::OuterScope;
use crate::{
    QueryError,
    execution::Environment,
    plan::{Expr, ParentFilter, Relation, RelationNode},
};
use tablecraft_core::values::Value;
use tablecraft_db::MultivalueList;
use tablecraft_sql::select::JoinKind;
mod table;

struct ParentRow {
    values: Vec<CellValue>,
    // None denotes an absent relation after an outer join, not stored NULL or
    // an empty collection. Handles never enter scalar values or source APIs.
    lists: Vec<Option<usize>>,
}
struct Missing;
struct JoinInput<'a> {
    left: &'a Relation,
    right: &'a Relation,
    kind: Option<JoinKind>,
    condition: Option<&'a Expr>,
}
impl MultivalueList for Missing {
    fn len(&self) -> usize {
        0
    }
    fn get(&self, _: usize) -> Option<&Value> {
        None
    }
}

impl Runtime<'_, '_> {
    fn parent_handles(&mut self, width: usize) -> Result<Vec<Option<usize>>, QueryError> {
        self.budget
            .charge_allocation(width as u64, size_of::<Option<usize>>() as u64)?;
        let mut handles = Vec::new();
        handles
            .try_reserve_exact(width)
            .map_err(|_| QueryError::Allocation)?;
        handles.resize(width, None);
        Ok(handles)
    }
    pub(super) fn joined_parent_rows<F>(
        &mut self,
        relation: &Relation,
        parent: &ParentFilter,
        outer: &[OuterScope<'_>],
        before: &mut F,
    ) -> Result<Vec<Vec<CellValue>>, QueryError>
    where
        F: FnMut(&mut Self, &[CellValue]) -> Result<Option<CellValue>, QueryError>,
    {
        let _span = tracing::debug_span!("query.parent.join").entered();
        let mut store = Vec::new();
        let mut leaf = 0;
        let input = self.parent_relation(relation, parent, outer, &mut leaf, &mut store)?;
        let width = parent.collections.len();
        self.budget.charge_allocation(
            width as u64,
            (size_of::<&dyn MultivalueList>() + size_of::<bool>()) as u64,
        )?;
        let mut views: Vec<&dyn MultivalueList> = Vec::new();
        let mut absent = Vec::new();
        views
            .try_reserve_exact(width)
            .map_err(|_| QueryError::Allocation)?;
        absent
            .try_reserve_exact(width)
            .map_err(|_| QueryError::Allocation)?;
        let mut rows = Vec::new();
        for mut row in input {
            self.tick()?;
            views.clear();
            absent.clear();
            for handle in &row.lists {
                self.tick()?;
                absent.push(handle.is_none());
                views.push(match handle {
                    Some(index) => {
                        store.get(*index).ok_or(QueryError::Source)? as &dyn MultivalueList
                    }
                    None => &Missing,
                });
            }
            if let Some(value) = before(self, &row.values)? {
                self.push(&mut row.values, value)?;
            }
            let truth = self.parent_truth(
                &parent.predicate,
                &ParentCollections {
                    requested: &parent.collections,
                    columns: &parent.columns,
                    lists: &views,
                    absent: Some(&absent),
                },
                &row.values,
                outer,
            )?;
            if truth.is_true() {
                self.push(&mut rows, row.values)?;
            }
        }
        Ok(rows)
    }
    fn parent_relation(
        &mut self,
        relation: &Relation,
        parent: &ParentFilter,
        outer: &[OuterScope<'_>],
        leaf: &mut usize,
        store: &mut Vec<Vec<Value>>,
    ) -> Result<Vec<ParentRow>, QueryError> {
        self.tick()?;
        match &relation.node {
            RelationNode::Cross(left, right) => self.parent_join(
                JoinInput {
                    left,
                    right,
                    kind: None,
                    condition: None,
                },
                parent,
                outer,
                leaf,
                store,
            ),
            RelationNode::Join {
                left,
                right,
                kind,
                condition,
            } => self.parent_join(
                JoinInput {
                    left,
                    right,
                    kind: Some(*kind),
                    condition: Some(condition),
                },
                parent,
                outer,
                leaf,
                store,
            ),
            _ => {
                let current = *leaf;
                *leaf += 1;
                let locations = parent.locations.as_ref().ok_or(QueryError::Source)?;
                self.budget
                    .charge_allocation(locations.len() as u64, size_of::<usize>() as u64)?;
                let mut selected = Vec::new();
                selected
                    .try_reserve_exact(locations.len())
                    .map_err(|_| QueryError::Allocation)?;
                selected.extend(
                    locations
                        .iter()
                        .enumerate()
                        .filter_map(|(index, location)| {
                            (location.leaf == current).then_some(index)
                        }),
                );
                if !selected.is_empty() {
                    return self.parent_table(relation, parent, &selected, store);
                }
                let mut rows = Vec::new();
                for values in self.relation(relation, outer)? {
                    let lists = self.parent_handles(parent.collections.len())?;
                    self.push(&mut rows, ParentRow { values, lists })?;
                }
                Ok(rows)
            }
        }
    }
    fn parent_join(
        &mut self,
        input: JoinInput<'_>,
        parent: &ParentFilter,
        outer: &[OuterScope<'_>],
        leaf: &mut usize,
        store: &mut Vec<Vec<Value>>,
    ) -> Result<Vec<ParentRow>, QueryError> {
        let JoinInput {
            left,
            right,
            kind,
            condition,
        } = input;
        let lhs = self.parent_relation(left, parent, outer, leaf, store)?;
        let rhs = self.parent_relation(right, parent, outer, leaf, store)?;
        self.budget
            .charge_allocation(rhs.len() as u64, size_of::<bool>() as u64)?;
        let mut matched = Vec::new();
        matched
            .try_reserve_exact(rhs.len())
            .map_err(|_| QueryError::Allocation)?;
        matched.resize(rhs.len(), false);
        let mut rows = Vec::new();
        for l in &lhs {
            self.tick()?;
            let mut hit = false;
            for (index, r) in rhs.iter().enumerate() {
                let values = self.join_row(&l.values, &r.values)?;
                let accepted = if let Some(condition) = condition {
                    self.eval_predicate(
                        condition,
                        Environment {
                            aggregate_results: None,
                            collections: None,
                            row: &values,
                            outer,
                            output: &[],
                            group: None,
                            group_keys: None,
                        },
                    )?
                    .is_true()
                } else {
                    true
                };
                if accepted {
                    hit = true;
                    matched[index] = true;
                    let mut lists = self.parent_handles(parent.collections.len())?;
                    for (slot, (left, right)) in lists.iter_mut().zip(l.lists.iter().zip(&r.lists))
                    {
                        self.tick()?;
                        if left.is_some() && right.is_some() {
                            return Err(QueryError::Source);
                        }
                        *slot = left.or(*right);
                    }
                    self.push(&mut rows, ParentRow { values, lists })?;
                }
            }
            if !hit && kind == Some(JoinKind::Left) {
                let values = self.join_missing(&l.values, right.row_width(), false)?;
                let mut lists = self.parent_handles(parent.collections.len())?;
                lists.copy_from_slice(&l.lists);
                self.push(&mut rows, ParentRow { values, lists })?;
            }
        }
        if kind == Some(JoinKind::Right) {
            for (index, r) in rhs.iter().enumerate() {
                self.tick()?;
                if !matched[index] {
                    let values = self.join_missing(&r.values, left.row_width(), true)?;
                    let mut lists = self.parent_handles(parent.collections.len())?;
                    lists.copy_from_slice(&r.lists);
                    self.push(&mut rows, ParentRow { values, lists })?;
                }
            }
        }
        Ok(rows)
    }
}
