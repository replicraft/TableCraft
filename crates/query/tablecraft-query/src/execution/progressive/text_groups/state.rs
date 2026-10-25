//! Paid operation-local keys and optional rows; no complete source cache.
use crate::{
    QueryError,
    execution::{
        Runtime,
        cell::CellValue,
        progressive::{groups::KeyPolicy, having::DeferredGroup},
    },
    plan::Expr,
    source::TextPolicy,
};

pub(super) struct Candidate {
    pub(super) keys: Vec<CellValue>,
    pub(super) row: Option<Vec<CellValue>>,
}

pub(super) struct AssignedRow {
    pub(super) group: usize,
    pub(super) row: Vec<CellValue>,
}

pub(super) struct Operations<'p> {
    pub(super) having: Vec<&'p Expr>,
    pub(super) selected_counts_only: bool,
    pub(super) active_counts_only: bool,
}

pub(super) struct Captured<'p> {
    pub(super) candidates: Vec<Candidate>,
    pub(super) cohort: Vec<KeyPolicy>,
    pub(super) policies: Vec<TextPolicy>,
    pub(super) operations: Operations<'p>,
}

pub(super) struct Partition {
    pub(super) groups: Vec<DeferredGroup>,
    pub(super) rows: Vec<AssignedRow>,
}

impl Runtime<'_, '_> {
    pub(super) fn push_text_group_item<T>(
        &mut self,
        items: &mut Vec<T>,
        value: T,
    ) -> Result<(), QueryError> {
        self.tick()?;
        if items.len() == items.capacity() {
            let additional = items.len().max(1);
            self.budget
                .charge_allocation(additional as u64, size_of::<T>() as u64)?;
            items
                .try_reserve_exact(additional)
                .map_err(|_| QueryError::Allocation)?;
        }
        items.push(value);
        Ok(())
    }
}
