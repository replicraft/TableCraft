use super::cell::CellValue;
use super::{Runtime, retrieval::OutputRow};
use crate::{QueryError, plan::ReadPlan};
use std::cmp::Ordering;

impl Runtime<'_, '_> {
    /// Fallible stable bottom-up merge ordering over indices. Row payloads never
    /// clone; all scratch/index/output containers consume the shared allowance.
    pub(super) fn sort_rows(
        &mut self,
        rows: Vec<OutputRow>,
        plan: &ReadPlan,
    ) -> Result<Vec<OutputRow>, QueryError> {
        self.sort_rows_by(rows, plan, |row| &row.keys, |row| &mut row.keys)
    }

    pub(super) fn sort_rows_by<T>(
        &mut self,
        mut rows: Vec<T>,
        plan: &ReadPlan,
        keys: fn(&T) -> &[CellValue],
        keys_mut: fn(&mut T) -> &mut [CellValue],
    ) -> Result<Vec<T>, QueryError> {
        if plan.sort.is_empty() || rows.len() < 2 {
            return Ok(rows);
        }
        self.unify_cell_policies(&mut rows, keys_mut)?;
        let n = rows.len();
        self.budget
            .charge_allocation(n as u64, (2 * std::mem::size_of::<usize>()) as u64)?;
        let mut indices = Vec::new();
        indices
            .try_reserve_exact(n)
            .map_err(|_| QueryError::Allocation)?;
        indices.extend(0..n);
        let mut scratch = Vec::new();
        scratch
            .try_reserve_exact(n)
            .map_err(|_| QueryError::Allocation)?;
        scratch.resize(n, 0);
        let mut width = 1;
        while width < n {
            let mut start = 0;
            while start < n {
                self.tick()?;
                let mid = (start + width).min(n);
                let end = (mid + width).min(n);
                let (mut l, mut r) = (start, mid);
                for dest in &mut scratch[start..end] {
                    let take_left = if l == mid {
                        false
                    } else if r == end {
                        true
                    } else {
                        self.compare_keys(keys(&rows[indices[l]]), keys(&rows[indices[r]]), plan)?
                            != Ordering::Greater
                    };
                    if take_left {
                        *dest = indices[l];
                        l += 1;
                    } else {
                        *dest = indices[r];
                        r += 1;
                    }
                }
                start = end;
            }
            std::mem::swap(&mut indices, &mut scratch);
            width *= 2;
        }
        self.budget
            .charge_allocation(n as u64, std::mem::size_of::<Option<T>>() as u64)?;
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(n)
            .map_err(|_| QueryError::Allocation)?;
        slots.extend(rows.into_iter().map(Some));
        let mut sorted = Vec::new();
        for index in indices {
            self.tick()?;
            let row = slots[index].take().ok_or(QueryError::Source)?;
            self.push(&mut sorted, row)?;
        }
        Ok(sorted)
    }
}
