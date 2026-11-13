//! Exact TCQC0001 wire framing; complete validation precedes owned allocation.
use super::{NativeQueryCreationIntent as Intent, QueryCreationIntentError as Error, binding};
use crate::{InspectError, header::check, identifier};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::LegacyDateTime,
};

const MAGIC: &[u8; 8] = b"TCQC0001";
const PREFIX: &[u8; 4] = b"TCQC";
const PROFILE: u16 = 1;
const TOTAL_AT: usize = MAGIC.len();
const PROFILE_AT: usize = TOTAL_AT + size_of::<u32>();
const FLAGS_AT: usize = PROFILE_AT + size_of::<u16>();
const BEFORE_AT: usize = FLAGS_AT + size_of::<u16>();
const BINDING_AT: usize = BEFORE_AT + size_of::<u64>();
const ID_AT: usize = BINDING_AT + binding::DIGEST_BYTES;
const VALUE_AT: usize = ID_AT + size_of::<i32>();
const DATE_AT: usize = VALUE_AT + size_of::<i32>();
const NAME_LENGTH_AT: usize = DATE_AT + size_of::<u64>();
const ALIAS_LENGTH_AT: usize = NAME_LENGTH_AT + size_of::<u16>();
const RESERVED_AT: usize = ALIAS_LENGTH_AT + size_of::<u16>();
const HEADER: usize = RESERVED_AT + size_of::<u32>();
const MAX_NAME_BYTES: usize = identifier::MAX_NAME_UNITS * char::MAX.len_utf8();
pub(super) const MAX_BYTES: usize = HEADER + 2 * MAX_NAME_BYTES;

struct Frame<'a> {
    name: &'a str,
    alias: &'a str,
    value: i32,
    captured_at: LegacyDateTime,
    native_id: i32,
    before_length: u64,
    before_binding: [u8; binding::DIGEST_BYTES],
}

pub(super) fn validate(
    names: [&str; 2],
    native_id: i32,
    before_length: u64,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    check(budget, cancelled)?;
    binding::retained_length(before_length)?;
    // The allocator reserves the signed-minimum seed and can allocate -1 once.
    if !(i32::MIN + 1..0).contains(&native_id) {
        return Err(Error::InvalidRecord);
    }
    for name in names {
        if name.is_empty()
            || name.len() > MAX_NAME_BYTES
            || !identifier::valid_name(name, budget, cancelled)?
        {
            return Err(Error::InvalidRecord);
        }
    }
    Ok(())
}

pub(super) fn encode(
    intent: &Intent,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    validate(
        [intent.name(), intent.alias()],
        intent.native_id,
        intent.before_length,
        budget,
        cancelled,
    )?;
    let length = HEADER + intent.name.len() + intent.alias.len();
    budget.charge_allocation(length as u64, 1)?;
    budget.charge(Resource::Steps, length as u64)?;
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
    bytes.extend_from_slice(&intent.native_id.to_le_bytes());
    bytes.extend_from_slice(&intent.value.to_le_bytes());
    bytes.extend_from_slice(&intent.captured_at.to_bits().to_le_bytes());
    bytes.extend_from_slice(&(intent.name.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&(intent.alias.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(intent.name.as_bytes());
    bytes.extend_from_slice(intent.alias.as_bytes());
    check(budget, cancelled)?;
    Ok(bytes)
}

pub(super) fn decode(
    input: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Intent, Error> {
    let frame = preflight(input, budget, cancelled)?;
    // NO_CLONE: The decoded semantic owner must outlive the caller's borrowed
    // record buffer. Names are copied only after the complete borrowed preflight.
    let name = copy_text(frame.name, budget)?;
    let alias = copy_text(frame.alias, budget)?;
    check(budget, cancelled)?;
    Ok(Intent {
        name,
        alias,
        value: frame.value,
        captured_at: frame.captured_at,
        native_id: frame.native_id,
        before_length: frame.before_length,
        before_binding: frame.before_binding,
    })
}

fn preflight<'a>(
    input: &'a [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Frame<'a>, Error> {
    check(budget, cancelled)?;
    if !(HEADER..=MAX_BYTES).contains(&input.len()) || input.get(..PREFIX.len()) != Some(PREFIX) {
        return Err(Error::InvalidRecord);
    }
    if input.get(..MAGIC.len()) != Some(MAGIC) {
        return Err(Error::UnsupportedVersion);
    }
    budget.charge(Resource::Steps, input.len() as u64)?;
    if u32::from_le_bytes(read(input, TOTAL_AT)?) as usize != input.len()
        || u16::from_le_bytes(read(input, PROFILE_AT)?) != PROFILE
        || u16::from_le_bytes(read(input, FLAGS_AT)?) != 0
        || u32::from_le_bytes(read(input, RESERVED_AT)?) != 0
    {
        return Err(Error::InvalidRecord);
    }
    let name_length = usize::from(u16::from_le_bytes(read(input, NAME_LENGTH_AT)?));
    let alias_length = usize::from(u16::from_le_bytes(read(input, ALIAS_LENGTH_AT)?));
    if name_length == 0
        || name_length > MAX_NAME_BYTES
        || alias_length == 0
        || alias_length > MAX_NAME_BYTES
    {
        return Err(Error::InvalidRecord);
    }
    let name_end = HEADER
        .checked_add(name_length)
        .ok_or(Error::InvalidRecord)?;
    let end = name_end
        .checked_add(alias_length)
        .ok_or(Error::InvalidRecord)?;
    if end != input.len() {
        return Err(Error::InvalidRecord);
    }
    let name = std::str::from_utf8(input.get(HEADER..name_end).ok_or(Error::InvalidRecord)?)
        .map_err(|_| Error::InvalidRecord)?;
    let alias = std::str::from_utf8(input.get(name_end..end).ok_or(Error::InvalidRecord)?)
        .map_err(|_| Error::InvalidRecord)?;
    let native_id = i32::from_le_bytes(read(input, ID_AT)?);
    let before_length = u64::from_le_bytes(read(input, BEFORE_AT)?);
    let captured_at = LegacyDateTime::from_bits(u64::from_le_bytes(read(input, DATE_AT)?))
        .map_err(|_| Error::InvalidRecord)?;
    validate([name, alias], native_id, before_length, budget, cancelled)?;
    Ok(Frame {
        name,
        alias,
        value: i32::from_le_bytes(read(input, VALUE_AT)?),
        captured_at,
        native_id,
        before_length,
        before_binding: read(input, BINDING_AT)?,
    })
}

fn read<const N: usize>(input: &[u8], at: usize) -> Result<[u8; N], Error> {
    input
        .get(at..at + N)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(Error::InvalidRecord)
}

fn copy_text(value: &str, budget: &mut ResourceBudget) -> Result<String, Error> {
    budget.charge_allocation(value.len() as u64, 1)?;
    budget.charge(Resource::Steps, value.len() as u64)?;
    let mut output = String::new();
    output
        .try_reserve_exact(value.len())
        .map_err(|_| InspectError::Allocation)?;
    output.push_str(value);
    Ok(output)
}
