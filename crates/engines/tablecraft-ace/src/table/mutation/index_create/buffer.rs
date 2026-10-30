use crate::{InspectError, header::check, preservation::COPY_CHUNK_BYTES};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::table) fn vector<T>(
    count: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<T>, InspectError> {
    check(budget, cancelled)?;
    budget.charge_allocation(count as u64, size_of::<T>() as u64)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(output)
}

pub(in crate::table) fn copy(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, InspectError> {
    let mut output = vector(bytes.len(), budget, cancelled)?;
    for chunk in bytes.chunks(COPY_CHUNK_BYTES) {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, chunk.len() as u64)?;
        output.extend_from_slice(chunk);
    }
    Ok(output)
}
