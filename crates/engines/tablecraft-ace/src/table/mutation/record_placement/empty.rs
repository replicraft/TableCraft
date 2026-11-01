//! Empty DATA page bytes shared by actual engineering and normal allocators.
use crate::{
    EditError, InspectError,
    format::{
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        row::ROW_COUNT_OFFSET,
    },
};
use tablecraft_core::limits::ResourceBudget;

const FREE_OFFSET: usize = DATA_SIGNATURE.len();
const EMPTY_FREE: usize = PAGE_BYTES - ROW_COUNT_OFFSET - size_of::<u16>();

pub(in crate::table::mutation) fn data(
    owner: u32,
    budget: &mut ResourceBudget,
) -> Result<Vec<u8>, EditError> {
    budget
        .charge_allocation(PAGE_BYTES as u64, 1)
        .map_err(InspectError::from)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(PAGE_BYTES)
        .map_err(|_| InspectError::Allocation)?;
    bytes.resize(PAGE_BYTES, 0);
    bytes[..DATA_SIGNATURE.len()].copy_from_slice(&DATA_SIGNATURE);
    bytes[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()]
        .copy_from_slice(&(EMPTY_FREE as u16).to_le_bytes());
    bytes[OWNER_OFFSET..OWNER_OFFSET + size_of::<u32>()].copy_from_slice(&owner.to_le_bytes());
    Ok(bytes)
}
