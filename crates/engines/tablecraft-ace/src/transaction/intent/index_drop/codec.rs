//! Exact TCID0001 data framing; no decoded physical effect is trusted.
use super::{Identity, IndexDropIntentError as Error, OriginalData as Intent, binding};
use crate::{format::page::PAGE_BYTES, header::check, identifier};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAGIC: &[u8; 8] = b"TCID0001";
const PROFILE: u16 = 1;
const UNIQUE_MAGIC: &[u8; 8] = b"TCID0002";
const UNIQUE_PROFILE: u16 = 2;

#[derive(Clone, Copy)]
enum Family {
    Original,
    UniqueLong,
}

impl Family {
    const fn framing(self) -> (&'static [u8; 8], u16) {
        match self {
            Self::Original => (MAGIC, PROFILE),
            Self::UniqueLong => (UNIQUE_MAGIC, UNIQUE_PROFILE),
        }
    }
}

const TOTAL_AT: usize = MAGIC.len();
const PROFILE_AT: usize = TOTAL_AT + size_of::<u32>();
const FLAGS_AT: usize = PROFILE_AT + size_of::<u16>();
const BEFORE_AT: usize = FLAGS_AT + size_of::<u16>();
const BINDING_AT: usize = BEFORE_AT + size_of::<u64>();
const TABLE_ID_AT: usize = BINDING_AT + binding::DIGEST_BYTES;
const DEFINITION_AT: usize = TABLE_ID_AT + size_of::<i32>();
const LOGICAL_AT: usize = DEFINITION_AT + size_of::<u32>();
const PHYSICAL_AT: usize = LOGICAL_AT + size_of::<u32>();
const TABLE_LENGTH_AT: usize = PHYSICAL_AT + size_of::<u32>();
const INDEX_LENGTH_AT: usize = TABLE_LENGTH_AT + size_of::<u16>();
const RESERVED_AT: usize = INDEX_LENGTH_AT + size_of::<u16>();
const HEADER: usize = RESERVED_AT + size_of::<u32>();
const MAX_NAME_BYTES: usize = 3 * identifier::MAX_NAME_UNITS;
pub(super) const MAX_BYTES: usize = HEADER + 2 * MAX_NAME_BYTES;

pub(super) fn validate(
    identity: Identity,
    before: u64,
    names: [&str; 2],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    binding::retained_length(before)?;
    if identity.definition == 0 || u64::from(identity.definition) >= before / PAGE_BYTES as u64 {
        return Err(Error::InvalidRecord);
    }
    for name in names {
        if name.len() > MAX_NAME_BYTES || !identifier::valid_name(name, budget, cancelled)? {
            return Err(Error::InvalidRecord);
        }
    }
    Ok(())
}

fn copy(text: &str, budget: &mut ResourceBudget) -> Result<String, Error> {
    budget.charge_allocation(text.len() as u64, 1)?;
    budget.charge(Resource::Steps, text.len() as u64)?;
    let mut result = String::new();
    result
        .try_reserve_exact(text.len())
        .map_err(|_| crate::InspectError::Allocation)?;
    result.push_str(text);
    Ok(result)
}

pub(super) fn encode(
    intent: &Intent,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    encode_family(intent, Family::Original, budget, cancelled)
}

pub(super) fn encode_unique(
    intent: &Intent,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    encode_family(intent, Family::UniqueLong, budget, cancelled)
}

fn encode_family(
    intent: &Intent,
    family: Family,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    let (magic, profile) = family.framing();
    check(budget, cancelled)?;
    let names = [intent.table(), intent.index_name()];
    validate(
        intent.identity,
        intent.before_length,
        names,
        budget,
        cancelled,
    )?;
    let length = names
        .iter()
        .try_fold(HEADER, |length, name| length.checked_add(name.len()))
        .ok_or(Error::InvalidRecord)?;
    budget.charge(Resource::Steps, length as u64)?;
    budget.charge_allocation(length as u64, 1)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| crate::InspectError::Allocation)?;
    bytes.extend(magic);
    bytes.extend(
        u32::try_from(length)
            .map_err(|_| Error::InvalidRecord)?
            .to_le_bytes(),
    );
    bytes.extend(profile.to_le_bytes());
    bytes.extend(0u16.to_le_bytes());
    bytes.extend(intent.before_length.to_le_bytes());
    bytes.extend(intent.before_binding);
    bytes.extend(intent.identity.native_table.to_le_bytes());
    bytes.extend(intent.identity.definition.to_le_bytes());
    bytes.extend(intent.identity.logical.to_le_bytes());
    bytes.extend(intent.identity.physical.to_le_bytes());
    for name in names {
        bytes.extend(
            u16::try_from(name.len())
                .map_err(|_| Error::InvalidRecord)?
                .to_le_bytes(),
        );
    }
    bytes.extend(0u32.to_le_bytes());
    for name in names {
        check(budget, cancelled)?;
        bytes.extend(name.as_bytes());
    }
    if bytes.len() != length {
        return Err(Error::InvalidRecord);
    }
    Ok(bytes)
}

pub(super) fn decode(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Intent, Error> {
    decode_family(bytes, Family::Original, budget, cancelled)
}

pub(super) fn decode_unique(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Intent, Error> {
    decode_family(bytes, Family::UniqueLong, budget, cancelled)
}

fn decode_family(
    bytes: &[u8],
    family: Family,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Intent, Error> {
    let (magic, profile) = family.framing();
    check(budget, cancelled)?;
    if !(HEADER..=MAX_BYTES).contains(&bytes.len())
        || bytes.get(..MAGIC.len()) != Some(magic)
        || u32::from_le_bytes(array(bytes, TOTAL_AT)?) as usize != bytes.len()
        || u16::from_le_bytes(array(bytes, PROFILE_AT)?) != profile
        || u16::from_le_bytes(array(bytes, FLAGS_AT)?) != 0
        || u32::from_le_bytes(array(bytes, RESERVED_AT)?) != 0
    {
        return Err(Error::InvalidRecord);
    }
    let before_length = u64::from_le_bytes(array(bytes, BEFORE_AT)?);
    let identity = Identity {
        native_table: i32::from_le_bytes(array(bytes, TABLE_ID_AT)?),
        definition: u32::from_le_bytes(array(bytes, DEFINITION_AT)?),
        logical: u32::from_le_bytes(array(bytes, LOGICAL_AT)?),
        physical: u32::from_le_bytes(array(bytes, PHYSICAL_AT)?),
    };
    let lengths = [
        usize::from(u16::from_le_bytes(array(bytes, TABLE_LENGTH_AT)?)),
        usize::from(u16::from_le_bytes(array(bytes, INDEX_LENGTH_AT)?)),
    ];
    if lengths
        .iter()
        .any(|length| *length == 0 || *length > MAX_NAME_BYTES)
        || lengths
            .iter()
            .try_fold(HEADER, |length, count| length.checked_add(*count))
            .ok_or(Error::InvalidRecord)?
            != bytes.len()
    {
        return Err(Error::InvalidRecord);
    }
    budget.charge(Resource::Steps, bytes.len() as u64)?;
    let mut cursor = HEADER;
    let mut names = [""; 2];
    for (name, length) in names.iter_mut().zip(lengths) {
        check(budget, cancelled)?;
        let end = cursor.checked_add(length).ok_or(Error::InvalidRecord)?;
        *name = std::str::from_utf8(bytes.get(cursor..end).ok_or(Error::InvalidRecord)?)
            .map_err(|_| Error::InvalidRecord)?;
        cursor = end;
    }
    validate(identity, before_length, names, budget, cancelled)?;
    Ok(Intent {
        table: copy(names[0], budget)?,
        index: copy(names[1], budget)?,
        identity,
        before_length,
        before_binding: array(bytes, BINDING_AT)?,
    })
}

fn array<const N: usize>(bytes: &[u8], at: usize) -> Result<[u8; N], Error> {
    bytes
        .get(at..at.checked_add(N).ok_or(Error::InvalidRecord)?)
        .ok_or(Error::InvalidRecord)?
        .try_into()
        .map_err(|_| Error::InvalidRecord)
}
