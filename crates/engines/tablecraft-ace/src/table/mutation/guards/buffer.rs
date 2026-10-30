use crate::{InspectError, header::check};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn vector<T>(
    count: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<T>, InspectError> {
    check(budget, cancelled)?;
    budget
        .charge_allocation(count as u64, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(output)
}

pub(super) fn push<T>(
    output: &mut Vec<T>,
    value: T,
    maximum: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    check(budget, cancelled)?;
    if output.len() == maximum {
        return Err(InspectError::StructureLimit);
    }
    if output.len() == output.capacity() {
        budget
            .charge_allocation(1, size_of::<T>() as u64)
            .map_err(InspectError::from)?;
        output
            .try_reserve_exact(1)
            .map_err(|_| InspectError::Allocation)?;
    }
    output.push(value);
    Ok(())
}
