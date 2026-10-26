//! Explicit list results use sidecars, never scalar placeholders or child IDs.
mod admission;
mod execute;
mod joined;
mod projection;
mod source;

use super::{Runtime, cell::CellValue};
use crate::{
    MultivalueValue, QueryError,
    plan::{ReadNode, ReadPlan},
    profile::MAX_RUNTIME_READ_DEPTH,
};

pub(super) struct CollectionFrame {
    scalars: Vec<CellValue>,
    // Indices are local collection slots, never evaluator values or native IDs.
    lists: Vec<(usize, MultivalueValue)>,
    keys: Vec<CellValue>,
}

impl Runtime<'_, '_> {
    pub(super) fn collection_read(
        &mut self,
        plan: &ReadPlan,
    ) -> Result<Vec<CollectionFrame>, QueryError> {
        self.tick()?;
        let read_depth = self.discovery.active_read_depth.get();
        if read_depth >= MAX_RUNTIME_READ_DEPTH {
            return Err(QueryError::StructureLimit);
        }
        let previous = std::mem::replace(&mut self.owner, plan.owner);
        self.discovery.active_read_depth.set(read_depth + 1);
        let result = self.collection_read_owned(plan);
        self.discovery.active_read_depth.set(read_depth);
        self.owner = previous;
        result
    }

    fn collection_read_owned(
        &mut self,
        plan: &ReadPlan,
    ) -> Result<Vec<CollectionFrame>, QueryError> {
        let ReadNode::Select(select) = &plan.node else {
            return Err(QueryError::Unsupported);
        };
        let input = self.collection_input(select)?;
        let mut rows = Vec::new();
        for frame in input {
            self.tick()?;
            if let Some(row) = self.collection_project(select, plan, &frame)? {
                self.push(&mut rows, row)?;
            }
        }
        let mut rows = self.sort_rows_by(rows, plan, |row| &row.keys, |row| &mut row.keys)?;
        self.apply_top_by(&mut rows, plan, |row| &row.keys)?;
        for row in &mut rows {
            self.tick()?;
            row.keys.clear();
        }
        Ok(rows)
    }
}
