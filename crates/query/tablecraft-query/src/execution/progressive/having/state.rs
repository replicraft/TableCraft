//! Deferred rows belong only to the explicit HAVING operation, never a cache.
use crate::{
    QueryError,
    execution::{Runtime, aggregate::accumulator::Accumulator, cell::CellValue},
    plan::Expr,
};

pub(super) struct HavingPopulation<'p> {
    pub(super) expressions: Vec<&'p Expr>,
    pub(super) groups: Vec<HavingGroup>,
    pub(super) count_only: bool,
}

pub(super) struct HavingGroup {
    pub(super) keys: Vec<CellValue>,
    pub(super) accumulators: Vec<Accumulator>,
    pub(super) rows: Vec<Vec<CellValue>>,
    pub(super) count: i64,
}

impl Runtime<'_, '_> {
    pub(super) fn retain_having_row(
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
    ) -> Result<HavingGroup, QueryError> {
        Ok(HavingGroup {
            keys,
            accumulators: self.progressive_accumulators(expressions)?,
            rows: Vec::new(),
            count: 0,
        })
    }
}
