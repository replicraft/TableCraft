//! Closed TCTD framing carries original identity and source purpose, never AFTER bytes.
use super::{
    Identity, MAX_NAME_BYTES, NativeTableDropIntent as Intent, SourceProfile,
    TableDropIntentError as Error, binding, copy_name, validate_name,
};
use crate::{format::page::PAGE_BYTES, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAGIC: &[u8; 8] = b"TCTD0001";
const CURRENT_MAGIC: &[u8; 8] = b"TCTD0002";
const CURRENT_PROFILE: u16 = 1;
const INLINE_PROFILE: u16 = 1;
const CREATOR_EXTERNAL_PROFILE: u16 = 2;
const TOTAL_AT: usize = MAGIC.len();
const PROFILE_AT: usize = TOTAL_AT + size_of::<u32>();
const FLAGS_AT: usize = PROFILE_AT + size_of::<u16>();
const BEFORE_LENGTH_AT: usize = FLAGS_AT + size_of::<u16>();
const BINDING_AT: usize = BEFORE_LENGTH_AT + size_of::<u64>();
const NATIVE_ID_AT: usize = BINDING_AT + binding::DIGEST_BYTES;
const PARENT_ID_AT: usize = NATIVE_ID_AT + size_of::<i32>();
const DEFINITION_AT: usize = PARENT_ID_AT + size_of::<i32>();
const CATALOG_AT: usize = DEFINITION_AT + size_of::<u32>();
const SLOT_AT: usize = CATALOG_AT + size_of::<u32>();
const SLOT_RESERVED_AT: usize = SLOT_AT + size_of::<u8>();
const SLOT_RESERVED_BYTES: usize = size_of::<u32>() - size_of::<u8>();
const NAME_LENGTH_AT: usize = SLOT_RESERVED_AT + SLOT_RESERVED_BYTES;
const RESERVED_AT: usize = NAME_LENGTH_AT + size_of::<u16>();
const HEADER_BYTES: usize = RESERVED_AT + size_of::<u16>();
pub(super) const MAX_RECORD_BYTES: usize = HEADER_BYTES + MAX_NAME_BYTES;

fn bytes<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N], Error> {
    input
        .get(offset..offset + N)
        .and_then(|raw| raw.try_into().ok())
        .ok_or(Error::InvalidRecord)
}

pub(super) fn validate_identity(identity: Identity, length: u64) -> Result<(), Error> {
    binding::length(length)?;
    let pages = length / PAGE_BYTES as u64;
    if !length.is_multiple_of(PAGE_BYTES as u64)
        || identity.native_id <= 0
        || identity.definition == 0
        || u64::from(identity.definition) >= pages
        || identity.catalog_page == 0
        || u64::from(identity.catalog_page) >= pages
    {
        return Err(Error::InvalidRecord);
    }
    Ok(())
}

pub(super) fn encode(
    intent: &Intent,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    check(budget, cancelled)?;
    validate_name(intent.table(), budget, cancelled)?;
    validate_identity(intent.identity, intent.before_length)?;
    let length = HEADER_BYTES + intent.table.len();
    budget.charge_allocation(length as u64, size_of::<u8>() as u64)?;
    budget.charge(Resource::Steps, length as u64)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| crate::InspectError::Allocation)?;
    let (magic, profile) = match intent.profile {
        SourceProfile::Inline => (MAGIC, INLINE_PROFILE),
        SourceProfile::CreatorExternal => (MAGIC, CREATOR_EXTERNAL_PROFILE),
        SourceProfile::Current => (CURRENT_MAGIC, CURRENT_PROFILE),
    };
    output.extend_from_slice(magic);
    output.extend_from_slice(&(length as u32).to_le_bytes());
    output.extend_from_slice(&profile.to_le_bytes());
    output.extend_from_slice(&0_u16.to_le_bytes());
    output.extend_from_slice(&intent.before_length.to_le_bytes());
    output.extend_from_slice(&intent.before_binding);
    output.extend_from_slice(&intent.identity.native_id.to_le_bytes());
    output.extend_from_slice(&intent.identity.parent_id.to_le_bytes());
    output.extend_from_slice(&intent.identity.definition.to_le_bytes());
    output.extend_from_slice(&intent.identity.catalog_page.to_le_bytes());
    output.push(intent.identity.catalog_slot);
    output.extend_from_slice(&[0; SLOT_RESERVED_BYTES]);
    output.extend_from_slice(&(intent.table.len() as u16).to_le_bytes());
    output.extend_from_slice(&0_u16.to_le_bytes());
    output.extend_from_slice(intent.table.as_bytes());
    check(budget, cancelled)?;
    Ok(output)
}

pub(super) fn decode(
    input: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Intent, Error> {
    decode_family(input, WireFamily::Original, budget, cancelled)
}

pub(super) fn decode_current(
    input: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Intent, Error> {
    decode_family(input, WireFamily::Current, budget, cancelled)
}

#[derive(Clone, Copy)]
enum WireFamily {
    Original,
    Current,
}

fn decode_family(
    input: &[u8],
    family: WireFamily,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Intent, Error> {
    check(budget, cancelled)?;
    let magic = match family {
        WireFamily::Original => MAGIC,
        WireFamily::Current => CURRENT_MAGIC,
    };
    if !(HEADER_BYTES..=MAX_RECORD_BYTES).contains(&input.len())
        || input.get(..magic.len()) != Some(magic)
    {
        return Err(Error::InvalidRecord);
    }
    budget.charge(Resource::Steps, input.len() as u64)?;
    if u32::from_le_bytes(bytes(input, TOTAL_AT)?) as usize != input.len()
        || u16::from_le_bytes(bytes(input, FLAGS_AT)?) != 0
        || input[SLOT_RESERVED_AT..NAME_LENGTH_AT] != [0; SLOT_RESERVED_BYTES]
        || u16::from_le_bytes(bytes(input, RESERVED_AT)?) != 0
    {
        return Err(Error::InvalidRecord);
    }
    let profile = match (family, u16::from_le_bytes(bytes(input, PROFILE_AT)?)) {
        (WireFamily::Original, INLINE_PROFILE) => SourceProfile::Inline,
        (WireFamily::Original, CREATOR_EXTERNAL_PROFILE) => SourceProfile::CreatorExternal,
        (WireFamily::Current, CURRENT_PROFILE) => SourceProfile::Current,
        _ => return Err(Error::InvalidRecord),
    };
    let identity = Identity {
        native_id: i32::from_le_bytes(bytes(input, NATIVE_ID_AT)?),
        parent_id: i32::from_le_bytes(bytes(input, PARENT_ID_AT)?),
        definition: u32::from_le_bytes(bytes(input, DEFINITION_AT)?),
        catalog_page: u32::from_le_bytes(bytes(input, CATALOG_AT)?),
        catalog_slot: input[SLOT_AT],
    };
    let before_length = u64::from_le_bytes(bytes(input, BEFORE_LENGTH_AT)?);
    validate_identity(identity, before_length)?;
    let name_length = usize::from(u16::from_le_bytes(bytes(input, NAME_LENGTH_AT)?));
    if name_length > MAX_NAME_BYTES || HEADER_BYTES + name_length != input.len() {
        return Err(Error::InvalidRecord);
    }
    let name = std::str::from_utf8(&input[HEADER_BYTES..]).map_err(|_| Error::InvalidRecord)?;
    validate_name(name, budget, cancelled)?;
    let table = copy_name(name, budget, cancelled)?;
    let before_binding = bytes(input, BINDING_AT)?;
    check(budget, cancelled)?;

    Ok(Intent {
        profile,
        identity,
        table,
        before_length,
        before_binding,
    })
}
