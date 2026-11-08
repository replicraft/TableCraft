//! Closed TCAC0001 framing, independent of every existing native action language.
use super::{AddColumnIntentError, NativeAddColumnIntent, binding, copy_name, validate_name};
use crate::{InspectError, header::check};
use tablecraft_core::limits::ResourceBudget;

const MAGIC: &[u8; 8] = b"TCAC0001";
const TOTAL_AT: usize = MAGIC.len();
const PROFILE_AT: usize = TOTAL_AT + size_of::<u32>();
const FLAGS_AT: usize = PROFILE_AT + size_of::<u16>();
const BEFORE_LENGTH_AT: usize = FLAGS_AT + size_of::<u16>();
const BINDING_AT: usize = BEFORE_LENGTH_AT + size_of::<u64>();
const TABLE_LENGTH_AT: usize = BINDING_AT + 32;
const COLUMN_LENGTH_AT: usize = TABLE_LENGTH_AT + size_of::<u16>();
pub(super) const HEADER_BYTES: usize = COLUMN_LENGTH_AT + size_of::<u16>();
const PROFILE: u16 = 1;

fn field<const N: usize>(input: &[u8], at: usize) -> Result<[u8; N], AddColumnIntentError> {
    input
        .get(at..at + N)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(AddColumnIntentError::InvalidRecord)
}

pub(super) fn decode(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeAddColumnIntent, AddColumnIntentError> {
    check(budget, cancelled)?;
    if !(HEADER_BYTES..=NativeAddColumnIntent::MAX_ENCODED_BYTES).contains(&bytes.len())
        || bytes.get(..MAGIC.len()) != Some(MAGIC.as_slice())
        || u32::from_le_bytes(field(bytes, TOTAL_AT)?) as usize != bytes.len()
        || u16::from_le_bytes(field(bytes, PROFILE_AT)?) != PROFILE
        || u16::from_le_bytes(field(bytes, FLAGS_AT)?) != 0
    {
        return Err(AddColumnIntentError::InvalidRecord);
    }
    let before_length = u64::from_le_bytes(field(bytes, BEFORE_LENGTH_AT)?);
    binding::length(before_length)?;
    let table_length = usize::from(u16::from_le_bytes(field(bytes, TABLE_LENGTH_AT)?));
    let column_length = usize::from(u16::from_le_bytes(field(bytes, COLUMN_LENGTH_AT)?));
    if HEADER_BYTES + table_length + column_length != bytes.len() {
        return Err(AddColumnIntentError::InvalidRecord);
    }
    let split = HEADER_BYTES + table_length;
    let table = std::str::from_utf8(&bytes[HEADER_BYTES..split])
        .map_err(|_| AddColumnIntentError::InvalidRecord)?;
    let column =
        std::str::from_utf8(&bytes[split..]).map_err(|_| AddColumnIntentError::InvalidRecord)?;
    validate_name(table, budget, cancelled)?;
    validate_name(column, budget, cancelled)?;

    let result = NativeAddColumnIntent {
        table: copy_name(table, budget, cancelled)?,
        column: copy_name(column, budget, cancelled)?,
        before_length,
        before_binding: field(bytes, BINDING_AT)?,
    };
    check(budget, cancelled)?;
    Ok(result)
}

pub(super) fn encode(
    intent: &NativeAddColumnIntent,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, AddColumnIntentError> {
    validate_name(intent.table(), budget, cancelled)?;
    validate_name(intent.column(), budget, cancelled)?;
    let length = HEADER_BYTES + intent.table().len() + intent.column().len();
    budget.charge_allocation(length as u64, 1)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&(length as u32).to_le_bytes());
    bytes.extend_from_slice(&PROFILE.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&intent.before_length.to_le_bytes());
    bytes.extend_from_slice(&intent.before_binding);
    bytes.extend_from_slice(&(intent.table().len() as u16).to_le_bytes());
    bytes.extend_from_slice(&(intent.column().len() as u16).to_le_bytes());
    bytes.extend_from_slice(intent.table().as_bytes());
    bytes.extend_from_slice(intent.column().as_bytes());
    check(budget, cancelled)?;
    Ok(bytes)
}
