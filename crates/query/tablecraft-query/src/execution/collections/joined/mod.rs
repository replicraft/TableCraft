//! Typed relation sidecars retain whole lists beside joined or expanded scalar rows.
mod derived;
mod missing;
mod relation;
mod table;

use super::{CollectionFrame, Runtime};
use crate::{
    MultivalueValue, QueryError,
    execution::multivalue::ParentCollections,
    plan::{Expr, ParentFilter, Relation, SelectPlan},
};
use tablecraft_core::values::Value;
use tablecraft_db::MultivalueList;

struct JoinedFrame {
    frame: CollectionFrame,
    // Positions in ParentFilter.collections, not scalar fields or output slots.
    criteria: Vec<(usize, Option<MultivalueValue>)>,
}

// A zero-length borrowed view accompanies an explicit absence bit; it never
// enters result payloads and never manufactures a stored NULL child.
struct MissingList;

impl MultivalueList for MissingList {
    fn len(&self) -> usize {
        0
    }

    fn get(&self, _: usize) -> Option<&Value> {
        None
    }
}

const MISSING_LIST: MissingList = MissingList;

struct Requests<'a> {
    selected: &'a [usize],
    parent: Option<&'a ParentFilter>,
}

impl Runtime<'_, '_> {
    pub(super) fn joined_collection_input(
        &mut self,
        select: &SelectPlan,
    ) -> Result<Vec<CollectionFrame>, QueryError> {
        let mut selected = Vec::new();
        for expression in &select.projection {
            self.tick()?;
            if let Expr::Multivalue { index, .. } = expression {
                self.push(&mut selected, *index)?;
            }
        }
        let requests = Requests {
            selected: &selected,
            parent: select.parent.as_ref(),
        };
        let input = self.joined_collection_relation(&select.source, &requests, 0, &mut 0)?;
        let mut rows = Vec::new();
        for row in input {
            self.tick()?;
            if let Some(parent) = &select.parent {
                self.budget.charge_allocation(
                    parent.collections.len() as u64,
                    size_of::<&dyn MultivalueList>() as u64,
                )?;
                self.budget
                    .charge_allocation(parent.collections.len() as u64, size_of::<bool>() as u64)?;
                let mut absent = Vec::new();
                absent
                    .try_reserve_exact(parent.collections.len())
                    .map_err(|_| QueryError::Allocation)?;
                let mut views = Vec::new();
                views
                    .try_reserve_exact(parent.collections.len())
                    .map_err(|_| QueryError::Allocation)?;
                for index in 0..parent.collections.len() {
                    let mut found = None;
                    for (position, value) in &row.criteria {
                        self.tick()?;
                        if *position == index {
                            found = Some(value.as_ref());
                            break;
                        }
                    }
                    let value = found.ok_or(QueryError::Source)?;
                    absent.push(value.is_none());
                    views.push(match value {
                        Some(value) => &value.values as &dyn MultivalueList,
                        None => &MISSING_LIST,
                    });
                }
                let collections = ParentCollections::borrowed_with_absence(
                    &parent.collections,
                    &parent.columns,
                    &views,
                    &absent,
                );
                if !self
                    .parent_truth(&parent.predicate, &collections, &row.frame.scalars, &[])?
                    .is_true()
                {
                    continue;
                }
            }
            self.push(&mut rows, row.frame)?;
        }
        Ok(rows)
    }

    fn joined_collection_width(&mut self, relation: &Relation) -> Result<usize, QueryError> {
        let mut width = 0_usize;
        for slot in &relation.slots {
            self.tick()?;
            width += usize::from(slot.collection_index.is_some());
        }
        Ok(width)
    }
}
