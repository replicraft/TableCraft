//! Distinct source-bound action over exactly pinned TCNT1–4 declaration templates.
use super::super::first_table::{
    MAX_DECLARATION_TEMPLATE_BYTES, decode_declaration_template, encode_declaration_template,
};
use super::{AddTableIntentError, BeforeBinding, NativeAddTableIntent};
use crate::{FirstTableDefinition, InspectError, format::page::PAGE_BYTES, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAGIC: &[u8; 8] = b"TCAT0001";
const PREFIX: &[u8; 4] = b"TCAT";
const PROFILE: u16 = 1;
const TOTAL_AT: usize = MAGIC.len();
const PROFILE_AT: usize = TOTAL_AT + size_of::<u32>();
const FLAGS_AT: usize = PROFILE_AT + size_of::<u16>();
const BEFORE_AT: usize = FLAGS_AT + size_of::<u16>();
const BINDING_AT: usize = BEFORE_AT + size_of::<u64>();
const BINDING_BYTES: usize = size_of::<BeforeBinding>();
const NESTED_LENGTH_AT: usize = BINDING_AT + BINDING_BYTES;
const RESERVED_AT: usize = NESTED_LENGTH_AT + size_of::<u32>();
const HEADER_BYTES: usize = RESERVED_AT + size_of::<u32>();
const NESTED_MAGIC: &[u8; 8] = b"TCNT0001";
const NESTED_TOTAL_AT: usize = NESTED_MAGIC.len();
const NESTED_PROFILE_AT: usize = NESTED_TOTAL_AT + size_of::<u32>();
const NESTED_FLAGS_AT: usize = NESTED_PROFILE_AT + size_of::<u16>();
const NESTED_BEFORE_AT: usize = NESTED_FLAGS_AT + size_of::<u16>();
const NESTED_BINDING_AT: usize = NESTED_BEFORE_AT + size_of::<u64>();
const NESTED_TABLE_LENGTH_AT: usize = NESTED_BINDING_AT + BINDING_BYTES;
const NESTED_PRIMARY_LENGTH_AT: usize = NESTED_TABLE_LENGTH_AT + size_of::<u16>();
const NESTED_COLUMNS_AT: usize = NESTED_PRIMARY_LENGTH_AT + size_of::<u16>();
const NESTED_RESERVED_AT: usize = NESTED_COLUMNS_AT + size_of::<u16>();
const NESTED_HEADER_BYTES: usize = NESTED_RESERVED_AT + size_of::<u16>();
const COPY_BYTES: usize = 4096;

// Profile 1 admits only the initial ten-page AddTable source. This length is
// framing, not source authority: the fresh native operation must qualify roles.
const PROFILE_SOURCE_PAGES: u64 = 10;
pub(super) const BEFORE_BYTES: u64 = PROFILE_SOURCE_PAGES * PAGE_BYTES as u64;
pub(super) const MAX_RECORD_BYTES: usize = HEADER_BYTES + MAX_DECLARATION_TEMPLATE_BYTES;

fn pin_template(raw: &[u8]) -> Result<(), AddTableIntentError> {
    if !(NESTED_HEADER_BYTES..=MAX_DECLARATION_TEMPLATE_BYTES).contains(&raw.len()) {
        return Err(AddTableIntentError::InvalidRecord);
    }
    let expected = match &raw[..NESTED_MAGIC.len()] {
        b"TCNT0001" => 1_u16,
        b"TCNT0002" => 2,
        b"TCNT0003" => 3,
        b"TCNT0004" => 4,
        _ => return Err(AddTableIntentError::UnsupportedVersion),
    };
    let profile = u16::from_le_bytes(raw[NESTED_PROFILE_AT..NESTED_FLAGS_AT].try_into().unwrap());
    if profile != expected {
        return Err(AddTableIntentError::UnsupportedProfile);
    }
    if raw[NESTED_FLAGS_AT..NESTED_BEFORE_AT] != [0; size_of::<u16>()]
        || raw[NESTED_RESERVED_AT..NESTED_HEADER_BYTES] != [0; size_of::<u16>()]
    {
        return Err(AddTableIntentError::InvalidRecord);
    }
    Ok(())
}

pub(super) fn encode(
    definition: &FirstTableDefinition,
    before_length: u64,
    before_binding: BeforeBinding,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, AddTableIntentError> {
    check(budget, cancelled)?;
    if before_length != BEFORE_BYTES {
        return Err(AddTableIntentError::BeforeMismatch);
    }
    let nested = encode_declaration_template(definition, budget, cancelled)?;
    pin_template(&nested)?;
    let total = HEADER_BYTES
        .checked_add(nested.len())
        .filter(|length| *length <= MAX_RECORD_BYTES)
        .ok_or(AddTableIntentError::InvalidRecord)?;
    budget
        .charge_allocation(total as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, HEADER_BYTES as u64)
        .map_err(InspectError::from)?;
    let mut raw = Vec::new();
    raw.try_reserve_exact(total)
        .map_err(|_| InspectError::Allocation)?;
    raw.resize(HEADER_BYTES, 0);
    raw[..MAGIC.len()].copy_from_slice(MAGIC);
    raw[TOTAL_AT..PROFILE_AT].copy_from_slice(&(total as u32).to_le_bytes());
    raw[PROFILE_AT..FLAGS_AT].copy_from_slice(&PROFILE.to_le_bytes());
    raw[BEFORE_AT..BINDING_AT].copy_from_slice(&before_length.to_le_bytes());
    raw[BINDING_AT..NESTED_LENGTH_AT].copy_from_slice(&before_binding);
    raw[NESTED_LENGTH_AT..RESERVED_AT].copy_from_slice(&(nested.len() as u32).to_le_bytes());
    for chunk in nested.chunks(COPY_BYTES) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(InspectError::from)?;
        raw.extend_from_slice(chunk);
    }
    check(budget, cancelled)?;
    Ok(raw)
}

pub(super) fn decode(
    raw: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeAddTableIntent, AddTableIntentError> {
    check(budget, cancelled)?;
    if !(HEADER_BYTES..=MAX_RECORD_BYTES).contains(&raw.len()) {
        return Err(AddTableIntentError::InvalidRecord);
    }
    budget
        .charge(Resource::Steps, HEADER_BYTES as u64)
        .map_err(InspectError::from)?;
    if &raw[..MAGIC.len()] != MAGIC {
        return Err(if raw.starts_with(PREFIX) {
            AddTableIntentError::UnsupportedVersion
        } else {
            AddTableIntentError::InvalidRecord
        });
    }
    if u16::from_le_bytes(raw[PROFILE_AT..FLAGS_AT].try_into().unwrap()) != PROFILE {
        return Err(AddTableIntentError::UnsupportedProfile);
    }
    let total = u32::from_le_bytes(raw[TOTAL_AT..PROFILE_AT].try_into().unwrap()) as usize;
    let before_length = u64::from_le_bytes(raw[BEFORE_AT..BINDING_AT].try_into().unwrap());
    let nested_length =
        u32::from_le_bytes(raw[NESTED_LENGTH_AT..RESERVED_AT].try_into().unwrap()) as usize;
    if total != raw.len()
        || raw[FLAGS_AT..BEFORE_AT] != [0; size_of::<u16>()]
        || raw[RESERVED_AT..HEADER_BYTES] != [0; size_of::<u32>()]
        || before_length != BEFORE_BYTES
        || nested_length != raw.len() - HEADER_BYTES
    {
        return Err(AddTableIntentError::InvalidRecord);
    }
    let nested = &raw[HEADER_BYTES..];
    budget
        .charge(Resource::Steps, NESTED_HEADER_BYTES as u64)
        .map_err(InspectError::from)?;
    pin_template(nested)?;
    let definition = decode_declaration_template(nested, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(NativeAddTableIntent {
        definition,
        before_length,
        before_binding: raw[BINDING_AT..NESTED_LENGTH_AT].try_into().unwrap(),
    })
}
