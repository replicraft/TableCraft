//! Coordinated existing-DATA and two wider physical root promotions.
mod admission;
mod authority;
mod proof;
mod selection;
mod stage;

pub(super) use selection::selected;
pub(super) use stage::plan;

use crate::{EditError, InspectError, header::check};
use tablecraft_core::limits::ResourceBudget;

fn vector<T>(
    capacity: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<T>, EditError> {
    check(budget, cancelled)?;
    budget
        .charge_allocation(capacity as u64, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(capacity)
        .map_err(|_| InspectError::Allocation)?;
    Ok(result)
}

/// Pay the complete new allocation and relocation when extending an owned image.
fn reserve_pages(
    bytes: &mut Vec<u8>,
    pages: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let extra = pages
        .checked_mul(crate::format::page::PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    let needed = bytes
        .len()
        .checked_add(extra)
        .ok_or(InspectError::StructureLimit)?;
    if needed > bytes.capacity() {
        budget
            .charge_allocation(needed as u64, 1)
            .map_err(InspectError::from)?;
        budget
            .charge(tablecraft_core::limits::Resource::Steps, bytes.len() as u64)
            .map_err(InspectError::from)?;
        bytes
            .try_reserve_exact(extra)
            .map_err(|_| InspectError::Allocation)?;
    }
    check(budget, cancelled)?;
    Ok(())
}

#[cfg(test)]
mod tests;
