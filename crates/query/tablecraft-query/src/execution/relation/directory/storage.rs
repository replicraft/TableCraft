//! Fallible append-linked buckets retain original right-row encounter order.
use crate::QueryError;
use tablecraft_core::limits::{Resource, ResourceBudget};

#[cfg(test)]
mod tests;

const INITIAL_BUCKETS: usize = 8;
const BUCKETS_PER_ENTRY: usize = 2;

#[derive(Default)]
pub(super) struct Directory {
    buckets: Vec<Bucket>,
    entries: Vec<Entry>,
}

struct Bucket {
    head: Option<usize>,
    tail: Option<usize>,
}

pub(super) struct Entry {
    pub(super) fingerprint: u64,
    pub(super) row: usize,
    pub(super) next: Option<usize>,
}

impl Directory {
    pub(super) fn first(&self, fingerprint: u64) -> Option<usize> {
        if self.buckets.is_empty() {
            return None;
        }
        self.buckets[bucket(fingerprint, self.buckets.len())].head
    }

    pub(super) fn entry(&self, position: usize) -> &Entry {
        &self.entries[position]
    }

    pub(super) fn insert(
        &mut self,
        fingerprint: u64,
        row: usize,
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
        if buckets > self.buckets.len() {
            self.rebuild(buckets, budget, cancelled)?;
        }
        if self.entries.len() == self.entries.capacity() {
            let additional = self.entries.len().max(1);
            budget.charge_allocation(additional as u64, size_of::<Entry>() as u64)?;
            self.entries
                .try_reserve_exact(additional)
                .map_err(|_| QueryError::Allocation)?;
        }
        let index = self.entries.len();
        self.entries.push(Entry {
            fingerprint,
            row,
            next: None,
        });
        append(&mut self.buckets, &mut self.entries, index);
        Ok(())
    }

    fn rebuild(
        &mut self,
        count: usize,
        budget: &mut ResourceBudget,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<(), QueryError> {
        budget.charge_allocation(count as u64, size_of::<Bucket>() as u64)?;
        let mut buckets = Vec::new();
        buckets
            .try_reserve_exact(count)
            .map_err(|_| QueryError::Allocation)?;
        for _ in 0..count {
            check(budget, cancelled)?;
            buckets.push(Bucket {
                head: None,
                tail: None,
            });
        }
        for index in 0..self.entries.len() {
            check(budget, cancelled)?;
            self.entries[index].next = None;
            append(&mut buckets, &mut self.entries, index);
        }
        self.buckets = buckets;
        Ok(())
    }
}

fn append(buckets: &mut [Bucket], entries: &mut [Entry], index: usize) {
    let position = bucket(entries[index].fingerprint, buckets.len());
    let bucket = &mut buckets[position];
    if let Some(tail) = bucket.tail {
        entries[tail].next = Some(index);
    } else {
        bucket.head = Some(index);
    }
    bucket.tail = Some(index);
}

fn bucket(fingerprint: u64, count: usize) -> usize {
    (fingerprint & (count - 1) as u64) as usize
}

fn check(
    budget: &mut ResourceBudget,
    cancelled: Option<&dyn Fn() -> bool>,
) -> Result<(), QueryError> {
    if cancelled.is_some_and(|cancelled| cancelled()) {
        return Err(QueryError::Cancelled);
    }
    budget.charge(Resource::Steps, 1)?;
    Ok(())
}
