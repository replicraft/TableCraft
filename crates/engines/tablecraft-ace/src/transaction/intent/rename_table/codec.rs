//! Closed TCRN0001 framing; complete validation precedes decoded name allocation.
use super::{
    Identity, MAX_NAME_BYTES, NativeTableRenameIntent, TableRenameIntentError, binding, copy_name,
    validate_name,
};
use crate::{InspectError, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAGIC: &[u8; 8] = b"TCRN0001";
const PREFIX: &[u8; 4] = b"TCRN";
const PROFILE: u16 = 1;
const TOTAL_AT: usize = MAGIC.len();
const PROFILE_AT: usize = TOTAL_AT + size_of::<u32>();
const FLAGS_AT: usize = PROFILE_AT + size_of::<u16>();
const BEFORE_LENGTH_AT: usize = FLAGS_AT + size_of::<u16>();
const BINDING_AT: usize = BEFORE_LENGTH_AT + size_of::<u64>();
const BINDING_BYTES: usize = 32;
const NATIVE_ID_AT: usize = BINDING_AT + BINDING_BYTES;
const PARENT_ID_AT: usize = NATIVE_ID_AT + size_of::<i32>();
const DEFINITION_AT: usize = PARENT_ID_AT + size_of::<i32>();
const CATALOG_AT: usize = DEFINITION_AT + size_of::<u32>();
const SLOT_AT: usize = CATALOG_AT + size_of::<u32>();
const SLOT_RESERVED_AT: usize = SLOT_AT + size_of::<u8>();
const SLOT_RESERVED_BYTES: usize = 3;
const OLD_LENGTH_AT: usize = SLOT_RESERVED_AT + SLOT_RESERVED_BYTES;
const NEW_LENGTH_AT: usize = OLD_LENGTH_AT + size_of::<u16>();
const RESERVED_AT: usize = NEW_LENGTH_AT + size_of::<u16>();
const HEADER_BYTES: usize = RESERVED_AT + size_of::<u32>();
pub(super) const MAX_RECORD_BYTES: usize = HEADER_BYTES + 2 * MAX_NAME_BYTES;

fn bytes<const N: usize>(input: &[u8], at: usize) -> Result<[u8; N], TableRenameIntentError> {
    input
        .get(at..at + N)
        .and_then(|raw| raw.try_into().ok())
        .ok_or(TableRenameIntentError::InvalidRecord)
}

pub(super) fn encode(
    intent: &NativeTableRenameIntent,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, TableRenameIntentError> {
    check(budget, cancelled)?;
    validate_name(intent.table(), budget, cancelled)?;
    validate_name(intent.new_name(), budget, cancelled)?;
    binding::length(intent.before_length)?;
    let length = HEADER_BYTES + intent.table().len() + intent.new_name().len();
    budget.charge_allocation(length as u64, 1)?;
    budget.charge(Resource::Steps, length as u64)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&(length as u32).to_le_bytes());
    output.extend_from_slice(&PROFILE.to_le_bytes());
    output.extend_from_slice(&0_u16.to_le_bytes());
    output.extend_from_slice(&intent.before_length.to_le_bytes());
    output.extend_from_slice(&intent.before_binding);
    output.extend_from_slice(&intent.identity.native_id.to_le_bytes());
    output.extend_from_slice(&intent.identity.parent_id.to_le_bytes());
    output.extend_from_slice(&intent.identity.definition_page.to_le_bytes());
    output.extend_from_slice(&intent.identity.catalog_page.to_le_bytes());
    output.push(intent.identity.catalog_slot);
    output.extend_from_slice(&[0; SLOT_RESERVED_BYTES]);
    output.extend_from_slice(&(intent.table().len() as u16).to_le_bytes());
    output.extend_from_slice(&(intent.new_name().len() as u16).to_le_bytes());
    output.extend_from_slice(&0_u32.to_le_bytes());
    output.extend_from_slice(intent.table().as_bytes());
    output.extend_from_slice(intent.new_name().as_bytes());
    check(budget, cancelled)?;
    Ok(output)
}

pub(super) fn decode(
    input: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeTableRenameIntent, TableRenameIntentError> {
    check(budget, cancelled)?;
    if !(HEADER_BYTES..=MAX_RECORD_BYTES).contains(&input.len())
        || input.get(..PREFIX.len()) != Some(PREFIX)
    {
        return Err(TableRenameIntentError::InvalidRecord);
    }
    if input.get(..MAGIC.len()) != Some(MAGIC) {
        return Err(TableRenameIntentError::UnsupportedVersion);
    }
    budget.charge(Resource::Steps, input.len() as u64)?;
    if u32::from_le_bytes(bytes(input, TOTAL_AT)?) as usize != input.len()
        || u16::from_le_bytes(bytes(input, FLAGS_AT)?) != 0
        || input[SLOT_RESERVED_AT..OLD_LENGTH_AT] != [0; SLOT_RESERVED_BYTES]
        || u32::from_le_bytes(bytes(input, RESERVED_AT)?) != 0
    {
        return Err(TableRenameIntentError::InvalidRecord);
    }
    if u16::from_le_bytes(bytes(input, PROFILE_AT)?) != PROFILE {
        return Err(TableRenameIntentError::UnsupportedProfile);
    }
    let before_length = u64::from_le_bytes(bytes(input, BEFORE_LENGTH_AT)?);
    binding::length(before_length)?;
    let old_length = usize::from(u16::from_le_bytes(bytes(input, OLD_LENGTH_AT)?));
    let new_length = usize::from(u16::from_le_bytes(bytes(input, NEW_LENGTH_AT)?));
    if old_length > MAX_NAME_BYTES
        || new_length > MAX_NAME_BYTES
        || HEADER_BYTES + old_length + new_length != input.len()
    {
        return Err(TableRenameIntentError::InvalidRecord);
    }
    let table = std::str::from_utf8(&input[HEADER_BYTES..HEADER_BYTES + old_length])
        .map_err(|_| TableRenameIntentError::InvalidRecord)?;
    let new_name = std::str::from_utf8(&input[HEADER_BYTES + old_length..])
        .map_err(|_| TableRenameIntentError::InvalidRecord)?;
    validate_name(table, budget, cancelled)?;
    validate_name(new_name, budget, cancelled)?;
    let identity = Identity {
        native_id: i32::from_le_bytes(bytes(input, NATIVE_ID_AT)?),
        parent_id: i32::from_le_bytes(bytes(input, PARENT_ID_AT)?),
        definition_page: u32::from_le_bytes(bytes(input, DEFINITION_AT)?),
        catalog_page: u32::from_le_bytes(bytes(input, CATALOG_AT)?),
        catalog_slot: input[SLOT_AT],
    };
    if identity.native_id <= 0 || identity.definition_page == 0 || identity.catalog_page == 0 {
        return Err(TableRenameIntentError::InvalidRecord);
    }
    let before_binding = bytes(input, BINDING_AT)?;
    let table = copy_name(table, budget, cancelled)?;
    let new_name = copy_name(new_name, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(NativeTableRenameIntent {
        table,
        new_name,
        identity,
        before_length,
        before_binding,
    })
}
