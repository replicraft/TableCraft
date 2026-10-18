use super::JournalError;
use crate::{ExclusiveFile, IoError, read_exact_at, write_exact_at};
use std::io::{Read, Seek, Write};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) const BLOCK_BYTES: usize = 64 * 1024;

/// Host-owned, exclusively accessed seekable storage with an honest sync barrier.
/// Successful sync must persist prior contents and length before later writes.
/// Target and journal must be distinct, correctly paired resources; hosts own
/// durable journal namespace creation, locks, identity, timeouts and later cleanup.
/// Normal target access must wait for recovery after any interrupted transaction.
pub trait DurableStorage: Read + Write + Seek {
    /// Current byte length, not inferred from the cursor position.
    fn storage_len(&mut self) -> Result<u64, IoError>;
    /// Persist preceding writes and length; flush alone is insufficient.
    fn sync_storage(&mut self) -> Result<(), IoError>;
}

impl DurableStorage for ExclusiveFile {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        self.length()
    }

    fn sync_storage(&mut self) -> Result<(), IoError> {
        self.sync_all().map_err(Into::into)
    }
}

pub(super) fn check(
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), JournalError> {
    if cancelled() {
        return Err(IoError::Cancelled.into());
    }
    budget.charge(Resource::Steps, 1)?;
    Ok(())
}

pub(super) fn read_at<D: DurableStorage>(
    device: &mut D,
    offset: u64,
    bytes: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), JournalError> {
    budget.charge(Resource::Bytes, bytes.len() as u64)?;
    read_exact_at(device, offset, bytes, budget, cancelled)?;
    Ok(())
}

pub(super) fn write_at<D: DurableStorage>(
    device: &mut D,
    offset: u64,
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), JournalError> {
    budget.charge(Resource::Bytes, bytes.len() as u64)?;
    write_exact_at(device, offset, bytes, budget, cancelled)?;
    Ok(())
}

pub(super) fn read_image<D: DurableStorage>(
    device: &mut D,
    offset: u64,
    size: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, JournalError> {
    check(budget, cancelled)?;
    budget.charge_allocation(size as u64, 1)?;

    let mut image = Vec::new();
    image
        .try_reserve_exact(size)
        .map_err(|_| JournalError::Allocation)?;
    image.resize(size, 0);

    for (i, chunk) in image.chunks_mut(BLOCK_BYTES).enumerate() {
        read_at(
            device,
            offset + (i * BLOCK_BYTES) as u64,
            chunk,
            budget,
            cancelled,
        )?;
    }

    Ok(image)
}

pub(super) fn write_image<D: DurableStorage>(
    device: &mut D,
    offset: u64,
    image: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), JournalError> {
    for (i, chunk) in image.chunks(BLOCK_BYTES).enumerate() {
        write_at(
            device,
            offset + (i * BLOCK_BYTES) as u64,
            chunk,
            budget,
            cancelled,
        )?;
    }
    Ok(())
}

pub(super) fn sync<D: DurableStorage>(
    device: &mut D,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), JournalError> {
    check(budget, cancelled)?;
    device.sync_storage()?;
    check(budget, cancelled)
}
