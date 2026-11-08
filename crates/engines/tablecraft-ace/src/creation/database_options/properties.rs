//! The explicit two-LONG OFF map; no missing/default or enabled-value inference.
use super::DatabaseOptionsError;
use crate::{InspectError, format::data_type, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) const NAMES: [&str; 2] = ["Track Name AutoCorrect Info", "Perform Name AutoCorrect"];
pub(super) const DDL_FLAG: u8 = 1;
const SIGNATURE: &[u8; 4] = b"MR2\0";
const NAME_LIST: u16 = 0x80;
const DEFAULT_MAP: u16 = 0;
const BLOCK_HEADER_BYTES: usize = size_of::<u32>() + size_of::<u16>();
const EMPTY_NAME_BYTES: usize = size_of::<u32>() + size_of::<u16>();
const LONG_ENTRY_BYTES: usize = 3 * size_of::<u16>() + 2 * size_of::<u8>() + size_of::<i32>();
const DICTIONARY_BYTES: usize = BLOCK_HEADER_BYTES
    + NAMES.len() * size_of::<u16>()
    + (NAMES[0].len() + NAMES[1].len()) * size_of::<u16>();
const DEFAULT_GROUP_BYTES: usize =
    BLOCK_HEADER_BYTES + EMPTY_NAME_BYTES + NAMES.len() * LONG_ENTRY_BYTES;
pub(super) const ENCODED_BYTES: usize = SIGNATURE.len() + DICTIONARY_BYTES + DEFAULT_GROUP_BYTES;

pub(super) fn encode(
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, DatabaseOptionsError> {
    check(budget, cancelled)?;
    budget
        .charge_allocation(ENCODED_BYTES as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, ENCODED_BYTES as u64)
        .map_err(InspectError::from)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(ENCODED_BYTES)
        .map_err(|_| InspectError::Allocation)?;
    bytes.extend_from_slice(SIGNATURE);
    bytes.extend_from_slice(&(DICTIONARY_BYTES as u32).to_le_bytes());
    bytes.extend_from_slice(&NAME_LIST.to_le_bytes());
    for name in NAMES {
        check(budget, cancelled)?;
        bytes.extend_from_slice(&((name.len() * size_of::<u16>()) as u16).to_le_bytes());
        for unit in name.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
    }
    bytes.extend_from_slice(&(DEFAULT_GROUP_BYTES as u32).to_le_bytes());
    bytes.extend_from_slice(&DEFAULT_MAP.to_le_bytes());
    bytes.extend_from_slice(&(EMPTY_NAME_BYTES as u32).to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    for index in 0..NAMES.len() {
        check(budget, cancelled)?;
        bytes.extend_from_slice(&(LONG_ENTRY_BYTES as u16).to_le_bytes());
        bytes.extend_from_slice(&[DDL_FLAG, data_type::INT32]);
        bytes.extend_from_slice(&(index as u16).to_le_bytes());
        bytes.extend_from_slice(&(size_of::<i32>() as u16).to_le_bytes());
        bytes.extend_from_slice(&0_i32.to_le_bytes());
    }
    if bytes.len() != ENCODED_BYTES {
        return Err(DatabaseOptionsError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(bytes)
}
