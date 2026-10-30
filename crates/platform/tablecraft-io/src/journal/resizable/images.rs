//! Fallible, chunked image ownership without changing the fixed V1 helpers.
use super::super::{
    JournalError,
    storage::{self, BLOCK_BYTES, DurableStorage},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

fn reserve(size: usize, budget: &mut ResourceBudget) -> Result<Vec<u8>, JournalError> {
    budget.charge_allocation(size as u64, 1)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size)
        .map_err(|_| JournalError::Allocation)?;
    Ok(bytes)
}

pub(super) fn read<D: DurableStorage>(
    device: &mut D,
    offset: u64,
    size: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, JournalError> {
    storage::check(budget, cancelled)?;
    let mut bytes = reserve(size, budget)?;
    let zeros = [0; BLOCK_BYTES];

    while bytes.len() < size {
        storage::check(budget, cancelled)?;
        let start = bytes.len();
        let count = (size - start).min(BLOCK_BYTES);
        budget.charge(Resource::Steps, count as u64)?;
        bytes.extend_from_slice(&zeros[..count]);
        storage::read_at(
            device,
            offset + start as u64,
            &mut bytes[start..],
            budget,
            cancelled,
        )?;
    }

    storage::check(budget, cancelled)?;
    Ok(bytes)
}

pub(super) fn copy(
    source: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, JournalError> {
    storage::check(budget, cancelled)?;
    let mut bytes = reserve(source.len(), budget)?;
    for chunk in source.chunks(BLOCK_BYTES) {
        storage::check(budget, cancelled)?;
        budget.charge(Resource::Steps, chunk.len() as u64)?;
        bytes.extend_from_slice(chunk);
    }

    storage::check(budget, cancelled)?;
    Ok(bytes)
}
