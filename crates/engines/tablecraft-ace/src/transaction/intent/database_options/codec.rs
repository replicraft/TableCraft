//! Closed fixed OFF request; neither paths nor source data are serialized here.
use super::{DatabaseOptionsIntentError, NativeDatabaseOptionsIntent};
use crate::{InspectError, format::page::PAGE_BYTES, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAGIC: &[u8; 8] = b"TCDO0001";
const PREFIX: &[u8; 4] = b"TCDO";
const PROFILE: u16 = 1;
const DISABLE_TRACK_AND_PERFORM: u32 = 1;
const KEYLESS_SOURCE_PAGES: u64 = 9;
const PRIMARY_SOURCE_PAGES: u64 = 10;
const TOTAL_AT: usize = MAGIC.len();
const PROFILE_AT: usize = TOTAL_AT + size_of::<u32>();
const FLAGS_AT: usize = PROFILE_AT + size_of::<u16>();
const BEFORE_LENGTH_AT: usize = FLAGS_AT + size_of::<u16>();
const BINDING_AT: usize = BEFORE_LENGTH_AT + size_of::<u64>();
const BINDING_BYTES: usize = 32;
const ACTION_AT: usize = BINDING_AT + BINDING_BYTES;
const RESERVED_AT: usize = ACTION_AT + size_of::<u32>();
pub(super) const RECORD_BYTES: usize = RESERVED_AT + size_of::<u32>();

pub(super) fn validate_length(length: u64) -> Result<(), DatabaseOptionsIntentError> {
    if !length.is_multiple_of(PAGE_BYTES as u64)
        || !matches!(
            length / PAGE_BYTES as u64,
            KEYLESS_SOURCE_PAGES | PRIMARY_SOURCE_PAGES
        )
    {
        return Err(DatabaseOptionsIntentError::InvalidRecord);
    }
    Ok(())
}

fn field<const N: usize>(
    input: &[u8],
    offset: usize,
) -> Result<[u8; N], DatabaseOptionsIntentError> {
    input
        .get(offset..offset + N)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(DatabaseOptionsIntentError::InvalidRecord)
}

pub(super) fn encode(
    intent: &NativeDatabaseOptionsIntent,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, DatabaseOptionsIntentError> {
    check(budget, cancelled)?;
    validate_length(intent.before_length)?;
    budget.charge_allocation(RECORD_BYTES as u64, 1)?;
    budget.charge(Resource::Steps, RECORD_BYTES as u64)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(RECORD_BYTES)
        .map_err(|_| InspectError::Allocation)?;
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&(RECORD_BYTES as u32).to_le_bytes());
    bytes.extend_from_slice(&PROFILE.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&intent.before_length.to_le_bytes());
    bytes.extend_from_slice(&intent.before_binding);
    bytes.extend_from_slice(&DISABLE_TRACK_AND_PERFORM.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    check(budget, cancelled)?;
    Ok(bytes)
}

pub(super) fn decode(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeDatabaseOptionsIntent, DatabaseOptionsIntentError> {
    check(budget, cancelled)?;
    if bytes.len() != RECORD_BYTES || bytes.get(..PREFIX.len()) != Some(PREFIX) {
        return Err(DatabaseOptionsIntentError::InvalidRecord);
    }
    if bytes.get(..MAGIC.len()) != Some(MAGIC) {
        return Err(DatabaseOptionsIntentError::UnsupportedVersion);
    }
    budget.charge(Resource::Steps, RECORD_BYTES as u64)?;
    if u32::from_le_bytes(field(bytes, TOTAL_AT)?) as usize != RECORD_BYTES
        || u16::from_le_bytes(field(bytes, FLAGS_AT)?) != 0
        || u32::from_le_bytes(field(bytes, ACTION_AT)?) != DISABLE_TRACK_AND_PERFORM
        || u32::from_le_bytes(field(bytes, RESERVED_AT)?) != 0
    {
        return Err(DatabaseOptionsIntentError::InvalidRecord);
    }
    if u16::from_le_bytes(field(bytes, PROFILE_AT)?) != PROFILE {
        return Err(DatabaseOptionsIntentError::UnsupportedProfile);
    }
    let before_length = u64::from_le_bytes(field(bytes, BEFORE_LENGTH_AT)?);
    validate_length(before_length)?;
    let before_binding = field(bytes, BINDING_AT)?;
    check(budget, cancelled)?;
    Ok(NativeDatabaseOptionsIntent {
        before_length,
        before_binding,
    })
}
