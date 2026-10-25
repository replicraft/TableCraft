//! Deferred group rows belong to explicit query operations, never a source cache.
use crate::{
    QueryError,
    execution::{
        Runtime, aggregate::accumulator::Accumulator, cell::CellValue, exact_keys::ExactKeys,
        progressive::groups::KeyPolicy,
    },
    plan::Expr,
    source::TextPolicy,
};

pub(in crate::execution::progressive) struct HavingInput<'p> {
    pub(super) expressions: Vec<&'p Expr>,
    pub(super) count_only: bool,
    pub(super) lookup: Option<ExactKeys>,
    pub(super) policies: Vec<TextPolicy>,
    pub(super) cohort: Vec<KeyPolicy>,
    pub(super) groups: Vec<DeferredGroup>,
}

pub(in crate::execution::progressive) struct HavingPopulation<'p> {
    pub(in crate::execution::progressive) expressions: Vec<&'p Expr>,
    pub(in crate::execution::progressive) groups: Vec<DeferredGroup>,
    pub(in crate::execution::progressive) count_only: bool,
    pub(in crate::execution::progressive) predicate_count_only: bool,
}

pub(in crate::execution::progressive) struct DeferredGroup {
    pub(in crate::execution::progressive) keys: Vec<CellValue>,
    pub(in crate::execution::progressive) accumulators: Vec<Accumulator>,
    pub(in crate::execution::progressive) rows: Vec<Vec<CellValue>>,
    pub(in crate::execution::progressive) count: i64,
}

impl Runtime<'_, '_> {
    pub(in crate::execution::progressive) fn retain_having_row(
        &mut self,
        rows: &mut Vec<Vec<CellValue>>,
        row: Vec<CellValue>,
    ) -> Result<(), QueryError> {
        self.tick()?;
        if rows.len() == rows.capacity() {
            let additional = rows.len().max(1);
            self.budget
                .charge_allocation(additional as u64, size_of::<Vec<CellValue>>() as u64)?;
            rows.try_reserve_exact(additional)
                .map_err(|_| QueryError::Allocation)?;
        }
        rows.push(row);
        Ok(())
    }

    pub(super) fn having_group(
        &mut self,
        keys: Vec<CellValue>,
        expressions: &[&Expr],
    ) -> Result<DeferredGroup, QueryError> {
        Ok(DeferredGroup {
            keys,
            accumulators: self.progressive_accumulators(expressions)?,
            rows: Vec::new(),
            count: 0,
        })
    }
}
