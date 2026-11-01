//! Shared fixed-record staging and independent complete-image authority.
pub(in crate::table::mutation) mod binding;
pub(in crate::table::mutation) mod indexed;
pub(in crate::table::mutation) mod proof;
pub(in crate::table::mutation) mod stage;

use crate::{EditError, InspectError, header::check, preservation::COPY_CHUNK_BYTES};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::table::mutation) fn copy(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    budget
        .charge_allocation(bytes.len() as u64, 1)
        .map_err(InspectError::from)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(bytes.len())
        .map_err(|_| InspectError::Allocation)?;
    for chunk in bytes.chunks(COPY_CHUNK_BYTES) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(InspectError::from)?;
        result.extend_from_slice(chunk);
    }
    Ok(result)
}

pub(in crate::table::mutation) fn name_work(
    left: &str,
    right: &str,
    budget: &mut ResourceBudget,
) -> Result<(), EditError> {
    budget
        .charge(Resource::Steps, left.len().max(right.len()) as u64)
        .map_err(InspectError::from)?;
    Ok(())
}

pub(in crate::table::mutation) fn unique(
    references: &mut Vec<u32>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    check(budget, cancelled)?;
    let levels = references.len().checked_ilog2().unwrap_or(0) as usize + 1;
    let work = references
        .len()
        .checked_mul(levels)
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    references.sort_unstable();
    references.dedup();
    check(budget, cancelled)?;
    Ok(references.len())
}
