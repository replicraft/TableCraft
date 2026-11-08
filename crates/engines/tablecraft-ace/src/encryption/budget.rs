//! Finite host allowance for existing metadata work plus streaming page work.
use crate::{
    InspectError,
    format::{page::PAGE_BYTES, row::MAX_SLOTS},
    limits::MAX_READ_FILE_BYTES,
};
use tablecraft_core::limits::ResourceBudget;

pub(super) const PAGE_TRANSFORM_BYTES: u64 = PAGE_BYTES as u64;
pub(super) const PAGE_TRANSFORM_STEPS: u64 =
    (PAGE_BYTES / super::agile::AES_BLOCK_BYTES) as u64 + 2;
pub(super) const COORDINATOR_STEPS_PER_PAGE: u64 = 4;
const METADATA_BYTES: u64 = 512 * 1024 * 1024;
const METADATA_ROWS: u64 = 100_000;
const METADATA_STEPS: u64 = 100_000_000;
// Two I/O pages, two codec scratch pages and one reusable input-page allocation
// plus its status. Reuse can cost less; actual charges remain operation-owned.
const DATA_PAGE_BYTES: u64 =
    3 * PAGE_BYTES as u64 + 2 * PAGE_TRANSFORM_BYTES + size_of::<Result<(), InspectError>>() as u64;
const SID_STEPS_PER_PAGE: u64 = 2 * MAX_SLOTS as u64 + 2;
const IO_PROGRESS_STEPS_PER_PAGE: u64 = 32;
const DATA_PAGE_STEPS: u64 = 2 * PAGE_TRANSFORM_STEPS
    + SID_STEPS_PER_PAGE
    + COORDINATOR_STEPS_PER_PAGE
    + IO_PROGRESS_STEPS_PER_PAGE;

/// Construct a checked finite host work allowance for a streaming rewrite.
///
/// The source geometry must fit the native read-file envelope. Established
/// metadata/header/password/SID allowances remain separate from page-derived
/// read/write, two codec phases, batch/status allocation and SID-application
/// work. I/O progress has conservative retry headroom; endless retries still
/// exhaust the cumulative step budget. This is an operation work policy, not an
/// estimate of actual resident memory or a guarantee that every exotic layout
/// fits. Native profile/metadata guards and caller live-memory policy still apply.
/// It neither captures the file nor authorizes an encryption operation.
pub fn encryption_rewrite_budget(source_bytes: u64) -> Result<ResourceBudget, InspectError> {
    if source_bytes > MAX_READ_FILE_BYTES {
        return Err(InspectError::StructureLimit);
    }
    if source_bytes < PAGE_BYTES as u64 || !source_bytes.is_multiple_of(PAGE_BYTES as u64) {
        return Err(InspectError::Malformed);
    }
    let pages = source_bytes / PAGE_BYTES as u64 - 1;
    let bytes = pages
        .checked_mul(DATA_PAGE_BYTES)
        .and_then(|bytes| bytes.checked_add(METADATA_BYTES))
        .ok_or(InspectError::StructureLimit)?;
    let steps = pages
        .checked_mul(DATA_PAGE_STEPS)
        .and_then(|steps| steps.checked_add(METADATA_STEPS))
        .ok_or(InspectError::StructureLimit)?;
    Ok(ResourceBudget::new(bytes, METADATA_ROWS, steps))
}
