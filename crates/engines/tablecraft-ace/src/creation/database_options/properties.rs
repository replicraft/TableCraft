//! The explicit two-LONG OFF map; no missing/default or enabled-value inference.
use super::DatabaseOptionsError;
use crate::{InspectError, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

const NAMES: [&str; 2] = ["Track Name AutoCorrect Info", "Perform Name AutoCorrect"];
const PROPERTY_BYTES: usize = 152;
const DICTIONARY_BYTES: u32 = 112;
const DEFAULT_GROUP_BYTES: u32 = 36;
const LONG_ENTRY_BYTES: u16 = 12;

pub(super) fn encode(
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, DatabaseOptionsError> {
    check(budget, cancelled)?;
    budget
        .charge_allocation(PROPERTY_BYTES as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, PROPERTY_BYTES as u64)
        .map_err(InspectError::from)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(PROPERTY_BYTES)
        .map_err(|_| InspectError::Allocation)?;
    bytes.extend_from_slice(b"MR2\0");
    bytes.extend_from_slice(&DICTIONARY_BYTES.to_le_bytes());
    bytes.extend_from_slice(&0x80_u16.to_le_bytes());
    for name in NAMES {
        check(budget, cancelled)?;
        bytes.extend_from_slice(&((name.len() * size_of::<u16>()) as u16).to_le_bytes());
        for unit in name.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
    }
    bytes.extend_from_slice(&DEFAULT_GROUP_BYTES.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&6_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    for index in 0_u16..2 {
        check(budget, cancelled)?;
        bytes.extend_from_slice(&LONG_ENTRY_BYTES.to_le_bytes());
        bytes.extend_from_slice(&[1, 4]);
        bytes.extend_from_slice(&index.to_le_bytes());
        bytes.extend_from_slice(&(size_of::<i32>() as u16).to_le_bytes());
        bytes.extend_from_slice(&0_i32.to_le_bytes());
    }
    if bytes.len() != PROPERTY_BYTES {
        return Err(DatabaseOptionsError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(bytes)
}
