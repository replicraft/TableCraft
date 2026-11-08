//! Reusable bounded owned data and payload-free ordered status storage.
use super::{FIXED_BUFFER_BYTES, PAGE_BYTES, PageResult, WORKER_SCRATCH_BYTES};
use crate::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};
use zeroize::{Zeroize, Zeroizing};

pub(super) struct Buffer {
    pub data: Zeroizing<Vec<u8>>,
    pub outcomes: Vec<PageResult>,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            data: Zeroizing::new(Vec::new()),
            outcomes: Vec::new(),
        }
    }

    pub fn retained_bytes(&self) -> Result<usize, InspectError> {
        FIXED_BUFFER_BYTES
            .checked_add(self.data.capacity())
            .and_then(|bytes| {
                self.outcomes
                    .capacity()
                    .checked_mul(size_of::<PageResult>())
                    .and_then(|statuses| bytes.checked_add(statuses))
            })
            .ok_or(InspectError::StructureLimit)
    }

    pub fn prepare(
        &mut self,
        pages: usize,
        workers: usize,
        ceiling: usize,
        budget: &mut ResourceBudget,
    ) -> Result<(), InspectError> {
        let bytes = pages
            .checked_mul(PAGE_BYTES)
            .ok_or(InspectError::StructureLimit)?;
        let scratch = pages
            .min(workers)
            .checked_mul(WORKER_SCRATCH_BYTES)
            .ok_or(InspectError::StructureLimit)?;
        let retained = self
            .retained_bytes()?
            .checked_add(scratch)
            .ok_or(InspectError::StructureLimit)?;
        if self.data.capacity() < bytes || self.outcomes.capacity() < pages || retained > ceiling {
            // Drop the old allocation before acquiring its replacement. Keeping
            // it live while growing would consume unreported peak memory and
            // turn repeated adaptive decisions into geometric batch shrinkage.
            self.data = Zeroizing::new(Vec::new());
            self.outcomes = Vec::new();
            budget.charge_allocation(bytes as u64, 1)?;
            budget.charge_allocation(pages as u64, size_of::<PageResult>() as u64)?;
            self.data
                .try_reserve_exact(bytes)
                .map_err(|_| InspectError::Allocation)?;
            self.outcomes
                .try_reserve_exact(pages)
                .map_err(|_| InspectError::Allocation)?;
            let extra_data = self.data.capacity().saturating_sub(bytes);
            let extra_statuses = self.outcomes.capacity().saturating_sub(pages);
            budget.charge(Resource::Bytes, extra_data as u64)?;
            budget.charge_allocation(extra_statuses as u64, size_of::<PageResult>() as u64)?;
        }
        if self
            .retained_bytes()?
            .checked_add(scratch)
            .ok_or(InspectError::StructureLimit)?
            > ceiling
        {
            return Err(InspectError::Allocation);
        }
        if self.data.len() > bytes {
            self.data[bytes..].zeroize();
        }
        self.data.resize(bytes, 0);
        self.outcomes.clear();
        self.outcomes.resize_with(pages, || Ok(()));
        Ok(())
    }
}

pub(super) fn pages_for_memory(
    ceiling: usize,
    workers: usize,
    remaining: usize,
) -> Result<usize, InspectError> {
    let fits = |pages: usize| {
        pages
            .checked_mul(PAGE_BYTES + size_of::<PageResult>())
            .and_then(|bytes| {
                pages
                    .min(workers)
                    .checked_mul(WORKER_SCRATCH_BYTES)
                    .and_then(|scratch| bytes.checked_add(scratch))
            })
            .and_then(|bytes| bytes.checked_add(FIXED_BUFFER_BYTES))
            .is_some_and(|bytes| bytes <= ceiling)
    };
    if remaining == 0 || workers == 0 || !fits(1) {
        return Err(InspectError::Allocation);
    }
    let mut lower = 1;
    let mut upper = remaining;
    while lower < upper {
        let middle = lower + (upper - lower).div_ceil(2);
        if fits(middle) {
            lower = middle;
        } else {
            upper = middle - 1;
        }
    }
    Ok(lower)
}
