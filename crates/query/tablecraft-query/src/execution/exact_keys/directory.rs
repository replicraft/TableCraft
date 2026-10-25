//! A fallible fingerprint directory whose probes and rebuilds are explicit work.
use super::check;
use crate::QueryError;
use tablecraft_core::limits::ResourceBudget;

const INITIAL_BUCKETS: usize = 8;
const BUCKETS_PER_ENTRY: usize = 2;

#[derive(Default)]
pub(super) struct Directory {
    heads: Vec<Option<usize>>,
    entries: Vec<Entry>,
}

struct Entry {
    fingerprint: u64,
    representative: usize,
    next: Option<usize>,
}

impl Directory {
    pub(super) fn find(
        &self,
        fingerprint: u64,
        mut visit: impl FnMut(usize, bool) -> Result<bool, QueryError>,
    ) -> Result<Option<usize>, QueryError> {
        if self.heads.is_empty() {
            return Ok(None);
        }
        let mut entry = self.heads[bucket(fingerprint, self.heads.len())];
        while let Some(index) = entry {
            let candidate = &self.entries[index];
            let same_fingerprint = candidate.fingerprint == fingerprint;
            if visit(candidate.representative, same_fingerprint)? && same_fingerprint {
                return Ok(Some(candidate.representative));
            }
            entry = candidate.next;
        }
        Ok(None)
    }

    pub(super) fn insert(
        &mut self,
        fingerprint: u64,
        representative: usize,
        budget: &mut ResourceBudget,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<(), QueryError> {
        check(budget, cancelled)?;
        let count = self
            .entries
            .len()
            .checked_add(1)
            .ok_or(QueryError::StructureLimit)?;
        let buckets = count
            .checked_mul(BUCKETS_PER_ENTRY)
            .and_then(usize::checked_next_power_of_two)
            .ok_or(QueryError::StructureLimit)?
            .max(INITIAL_BUCKETS);
        if buckets > self.heads.len() {
            self.rebuild(buckets, budget, cancelled)?;
        }
        if self.entries.len() == self.entries.capacity() {
            let additional = self.entries.len().max(1);
            budget.charge_allocation(additional as u64, size_of::<Entry>() as u64)?;
            self.entries
                .try_reserve_exact(additional)
                .map_err(|_| QueryError::Allocation)?;
        }
        let bucket = bucket(fingerprint, self.heads.len());
        let index = self.entries.len();
        self.entries.push(Entry {
            fingerprint,
            representative,
            next: self.heads[bucket],
        });
        self.heads[bucket] = Some(index);
        Ok(())
    }

    fn rebuild(
        &mut self,
        count: usize,
        budget: &mut ResourceBudget,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<(), QueryError> {
        budget.charge_allocation(count as u64, size_of::<Option<usize>>() as u64)?;
        let mut heads = Vec::new();
        heads
            .try_reserve_exact(count)
            .map_err(|_| QueryError::Allocation)?;
        for _ in 0..count {
            check(budget, cancelled)?;
            heads.push(None);
        }
        for (index, entry) in self.entries.iter_mut().enumerate() {
            check(budget, cancelled)?;
            let bucket = bucket(entry.fingerprint, count);
            entry.next = heads[bucket];
            heads[bucket] = Some(index);
        }
        self.heads = heads;
        Ok(())
    }
}

fn bucket(fingerprint: u64, count: usize) -> usize {
    // Counts are checked powers of two. Mask before conversion on narrow hosts.
    (fingerprint & (count - 1) as u64) as usize
}
