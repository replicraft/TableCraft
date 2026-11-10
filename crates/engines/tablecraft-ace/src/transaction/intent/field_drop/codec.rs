//! Explicit TCFD0001/2 source languages share framing, never decoder fallback.
use super::{
    FieldDropIntentError as Error, Identity, MAX_NAME_BYTES, NativeFieldDropIntent as Intent,
    binding, copy_text, validate_request,
};
use crate::{format::page::PAGE_BYTES, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAGIC: &[u8; 8] = b"TCFD0001";
#[cfg(feature = "ace16-create")]
const PROFILE: u16 = 1;
#[cfg(feature = "ace16-create")]
const CURRENT_TAIL_MAGIC: &[u8; 8] = b"TCFD0002";
#[cfg(feature = "ace16-create")]
const CURRENT_TAIL_PROFILE: u16 = 2;
const RETAINED_MAGIC: &[u8; 8] = b"TCFD0003";
const RETAINED_PROFILE: u16 = 3;
const TOTAL_AT: usize = MAGIC.len();
const PROFILE_AT: usize = TOTAL_AT + size_of::<u32>();
const FLAGS_AT: usize = PROFILE_AT + size_of::<u16>();
const BEFORE_LENGTH_AT: usize = FLAGS_AT + size_of::<u16>();
const BINDING_AT: usize = BEFORE_LENGTH_AT + size_of::<u64>();
const NATIVE_TABLE_AT: usize = BINDING_AT + binding::DIGEST_BYTES;
const DEFINITION_AT: usize = NATIVE_TABLE_AT + size_of::<i32>();
const NUMBER_AT: usize = DEFINITION_AT + size_of::<u32>();
const COLUMN_ID_AT: usize = NUMBER_AT + size_of::<u16>();
const TABLE_LENGTH_AT: usize = COLUMN_ID_AT + size_of::<u16>();
const COLUMN_LENGTH_AT: usize = TABLE_LENGTH_AT + size_of::<u16>();
const RESERVED_AT: usize = COLUMN_LENGTH_AT + size_of::<u16>();
const HEADER_BYTES: usize = RESERVED_AT + size_of::<u32>();
pub(super) const MAX_RECORD_BYTES: usize = HEADER_BYTES + 2 * MAX_NAME_BYTES;

#[derive(Clone, Copy)]
pub(super) enum Profile {
    #[cfg(feature = "ace16-create")]
    Empty,
    #[cfg(feature = "ace16-create")]
    PopulatedTail,
    RetainedTail,
}

impl Profile {
    fn magic(self) -> &'static [u8; 8] {
        match self {
            #[cfg(feature = "ace16-create")]
            Self::Empty => MAGIC,
            #[cfg(feature = "ace16-create")]
            Self::PopulatedTail => CURRENT_TAIL_MAGIC,
            Self::RetainedTail => RETAINED_MAGIC,
        }
    }

    fn number(self) -> u16 {
        match self {
            #[cfg(feature = "ace16-create")]
            Self::Empty => PROFILE,
            #[cfg(feature = "ace16-create")]
            Self::PopulatedTail => CURRENT_TAIL_PROFILE,
            Self::RetainedTail => RETAINED_PROFILE,
        }
    }
}

fn bytes<const N: usize>(input: &[u8], at: usize) -> Result<[u8; N], Error> {
    input
        .get(at..at + N)
        .and_then(|raw| raw.try_into().ok())
        .ok_or(Error::InvalidRecord)
}

pub(super) fn validate_identity(
    identity: Identity,
    before_length: u64,
    profile: Profile,
) -> Result<(), Error> {
    if matches!(profile, Profile::RetainedTail) {
        binding::retained_length(before_length)?;
    } else {
        binding::length(before_length)?;
    }
    if !before_length.is_multiple_of(PAGE_BYTES as u64)
        || identity.native_table <= 0
        || identity.definition == 0
        || u64::from(identity.definition) >= before_length / PAGE_BYTES as u64
        || identity.number == 0
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
    validate_request(intent.table(), intent.column(), budget, cancelled)?;
    validate_identity(intent.identity, intent.before_length, intent.profile)?;
    let length = HEADER_BYTES + intent.table.len() + intent.column.len();
    budget.charge_allocation(length as u64, 1)?;
    budget.charge(Resource::Steps, length as u64)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| crate::InspectError::Allocation)?;
    output.extend_from_slice(intent.profile.magic());
    output.extend_from_slice(&(length as u32).to_le_bytes());
    output.extend_from_slice(&intent.profile.number().to_le_bytes());
    output.extend_from_slice(&0_u16.to_le_bytes());
    output.extend_from_slice(&intent.before_length.to_le_bytes());
    output.extend_from_slice(&intent.before_binding);
    output.extend_from_slice(&intent.identity.native_table.to_le_bytes());
    output.extend_from_slice(&intent.identity.definition.to_le_bytes());
    output.extend_from_slice(&intent.identity.number.to_le_bytes());
    output.extend_from_slice(&intent.identity.column_id.to_le_bytes());
    output.extend_from_slice(&(intent.table.len() as u16).to_le_bytes());
    output.extend_from_slice(&(intent.column.len() as u16).to_le_bytes());
    output.extend_from_slice(&0_u32.to_le_bytes());
    output.extend_from_slice(intent.table.as_bytes());
    output.extend_from_slice(intent.column.as_bytes());
    check(budget, cancelled)?;
    Ok(output)
}

pub(super) fn decode(
    input: &[u8],
    profile: Profile,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Intent, Error> {
    check(budget, cancelled)?;
    if !(HEADER_BYTES..=MAX_RECORD_BYTES).contains(&input.len())
        || input.get(..MAGIC.len()) != Some(profile.magic())
    {
        return Err(Error::InvalidRecord);
    }
    budget.charge(Resource::Steps, input.len() as u64)?;
    if u32::from_le_bytes(bytes(input, TOTAL_AT)?) as usize != input.len()
        || u16::from_le_bytes(bytes(input, PROFILE_AT)?) != profile.number()
        || u16::from_le_bytes(bytes(input, FLAGS_AT)?) != 0
        || u32::from_le_bytes(bytes(input, RESERVED_AT)?) != 0
    {
        return Err(Error::InvalidRecord);
    }
    let before_length = u64::from_le_bytes(bytes(input, BEFORE_LENGTH_AT)?);
    let identity = Identity {
        native_table: i32::from_le_bytes(bytes(input, NATIVE_TABLE_AT)?),
        definition: u32::from_le_bytes(bytes(input, DEFINITION_AT)?),
        number: u16::from_le_bytes(bytes(input, NUMBER_AT)?),
        column_id: u16::from_le_bytes(bytes(input, COLUMN_ID_AT)?),
    };
    let table_length = usize::from(u16::from_le_bytes(bytes(input, TABLE_LENGTH_AT)?));
    let column_length = usize::from(u16::from_le_bytes(bytes(input, COLUMN_LENGTH_AT)?));
    if table_length > MAX_NAME_BYTES
        || column_length > MAX_NAME_BYTES
        || HEADER_BYTES + table_length + column_length != input.len()
    {
        return Err(Error::InvalidRecord);
    }
    validate_identity(identity, before_length, profile)?;
    let table_end = HEADER_BYTES + table_length;
    let table =
        std::str::from_utf8(&input[HEADER_BYTES..table_end]).map_err(|_| Error::InvalidRecord)?;
    let column = std::str::from_utf8(&input[table_end..]).map_err(|_| Error::InvalidRecord)?;
    validate_request(table, column, budget, cancelled)?;
    let intent = Intent {
        profile,
        identity,
        table: copy_text(table, budget)?,
        column: copy_text(column, budget)?,
        before_length,
        before_binding: bytes(input, BINDING_AT)?,
    };
    check(budget, cancelled)?;
    Ok(intent)
}
