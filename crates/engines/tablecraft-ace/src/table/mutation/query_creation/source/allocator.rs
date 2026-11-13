//! Signed synthetic catalog identity, independent of table-definition pages.
use crate::{InspectError, NativeImage, QueryCreationError, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn greatest(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<i32, QueryCreationError> {
    let origins = image.catalog().origins();
    budget.charge_allocation(origins.len() as u64, size_of::<i32>() as u64)?;
    let mut ids = Vec::new();
    ids.try_reserve_exact(origins.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut greatest = i32::MIN;
    for origin in origins {
        check(budget, cancelled)?;
        let id = origin.native_id();
        if id < 0 {
            greatest = greatest.max(id);
        }
        ids.push(id);
    }
    let work = ids
        .len()
        .checked_mul(ids.len().max(1).ilog2() as usize + 1)
        .ok_or(QueryCreationError::Source)?;
    budget.charge(Resource::Steps, work as u64)?;
    ids.sort_unstable();
    for pair in ids.windows(2) {
        check(budget, cancelled)?;
        if pair[0] == pair[1] {
            return Err(QueryCreationError::Source);
        }
    }
    Ok(greatest)
}

pub(super) fn next(greatest: i32) -> Result<i32, QueryCreationError> {
    if greatest == -1 {
        return Err(QueryCreationError::Source);
    }
    greatest
        .checked_add(1)
        .filter(|id| *id < 0)
        .ok_or(QueryCreationError::Source)
}
