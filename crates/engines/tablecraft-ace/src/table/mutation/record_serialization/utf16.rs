//! Paid uncompressed UTF16 representation shared by native query record writers.
use crate::{InspectError, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::table::mutation) fn encode(
    value: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, InspectError> {
    let mut length = 0_usize;
    for _ in value.encode_utf16() {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, 1)?;
        length = length
            .checked_add(size_of::<u16>())
            .ok_or(InspectError::StructureLimit)?;
    }
    budget.charge_allocation(length as u64, 1)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    for unit in value.encode_utf16() {
        check(budget, cancelled)?;
        result.extend_from_slice(&unit.to_le_bytes());
    }
    Ok(result)
}
