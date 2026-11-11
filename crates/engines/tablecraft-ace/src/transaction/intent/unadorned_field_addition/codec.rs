//! Exact scalar originals carry declaration and source witnesses, never native pages.
mod declaration;
use super::{
    Identity, NativeUnadornedFieldAdditionIntent as Intent,
    UnadornedFieldAdditionIntentError as Error,
};
use crate::{
    InspectError, UnadornedFieldAdditionDefinition, header::check, identifier,
    transaction::intent::binding,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAGIC: &[u8; 8] = b"TCAC0014";
const PROFILE: u16 = 14;
const HEADER_BYTES: usize = 88;
const TOTAL_AT: usize = 8;
const PROFILE_AT: usize = 12;
const FLAGS_AT: usize = 14;
const BEFORE_AT: usize = 16;
const BINDING_AT: usize = 24;
const TABLE_ID_AT: usize = 56;
const DEFINITION_AT: usize = 60;
const NUMBER_AT: usize = 64;
const COLUMN_ID_AT: usize = 66;
const COLUMN_COUNT_AT: usize = 68;
const KIND_AT: usize = 70;
const POLICY_AT: usize = 71;
const TABLE_LENGTH_AT: usize = 72;
const COLUMN_LENGTH_AT: usize = 74;
const SOURCE_AT: usize = 76;
const COMPRESSION_AT: usize = 77;
const WIDTH_AT: usize = 78;
const PRECISION_AT: usize = 80;
const SCALE_AT: usize = 81;
const RESERVED_AT: usize = 82;
const PAGE_BYTES: u64 = 4096;
const MAX_ACTIVE_COLUMNS: u16 = crate::limits::MAX_USER_COLUMNS as u16;
// Three UTF8 bytes per native UTF16 unit is the largest possible valid envelope.
const TABLE_BYTES: usize = identifier::MAX_NAME_UNITS * 3;
const COLUMN_BYTES: usize = identifier::MAX_NAME_UNITS;
pub(super) const MAX_BYTES: usize = HEADER_BYTES + TABLE_BYTES + COLUMN_BYTES;

fn array<const N: usize>(bytes: &[u8], at: usize) -> Result<[u8; N], Error> {
    bytes
        .get(at..at.checked_add(N).ok_or(Error::InvalidRecord)?)
        .and_then(|value| value.try_into().ok())
        .ok_or(Error::InvalidRecord)
}

fn validate_table(
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    if name.len() > TABLE_BYTES || !identifier::valid_name(name, budget, cancelled)? {
        return Err(Error::InvalidRecord);
    }
    Ok(())
}

pub(super) fn copy_table(
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<String, Error> {
    validate_table(name, budget, cancelled)?;
    budget
        .charge_allocation(name.len() as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, name.len() as u64)
        .map_err(InspectError::from)?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(name.len())
        .map_err(|_| InspectError::Allocation)?;
    owned.push_str(name);
    Ok(owned)
}

pub(super) fn validate_identity(identity: Identity, before_length: u64) -> Result<(), Error> {
    binding::retained_length(before_length)?;
    if identity.definition == 0
        || u64::from(identity.definition) >= before_length / PAGE_BYTES
        || identity.column_count == 0
        || identity.column_count > MAX_ACTIVE_COLUMNS
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
    validate_identity(intent.identity, intent.before_length)?;
    validate_table(intent.table(), budget, cancelled)?;
    let column = intent.column();
    if !column.is_ascii()
        || column.len() > COLUMN_BYTES
        || !identifier::valid_name(column, budget, cancelled)?
    {
        return Err(Error::InvalidRecord);
    }
    let total = HEADER_BYTES + intent.table.len() + column.len();
    budget
        .charge_allocation(total as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, total as u64)
        .map_err(InspectError::from)?;
    let mut out = Vec::new();
    out.try_reserve_exact(total)
        .map_err(|_| InspectError::Allocation)?;
    out.resize(HEADER_BYTES, 0);
    out[..MAGIC.len()].copy_from_slice(MAGIC);
    out[TOTAL_AT..TOTAL_AT + 4].copy_from_slice(&(total as u32).to_le_bytes());
    out[PROFILE_AT..PROFILE_AT + 2].copy_from_slice(&PROFILE.to_le_bytes());
    out[BEFORE_AT..BEFORE_AT + 8].copy_from_slice(&intent.before_length.to_le_bytes());
    out[BINDING_AT..BINDING_AT + 32].copy_from_slice(&intent.before_binding);
    out[TABLE_ID_AT..TABLE_ID_AT + 4].copy_from_slice(&intent.identity.native_table.to_le_bytes());
    out[DEFINITION_AT..DEFINITION_AT + 4]
        .copy_from_slice(&intent.identity.definition.to_le_bytes());
    out[NUMBER_AT..NUMBER_AT + 2].copy_from_slice(&intent.identity.native_number.to_le_bytes());
    out[COLUMN_ID_AT..COLUMN_ID_AT + 2].copy_from_slice(&intent.identity.column_id.to_le_bytes());
    out[COLUMN_COUNT_AT..COLUMN_COUNT_AT + 2]
        .copy_from_slice(&intent.identity.column_count.to_le_bytes());
    let shape = declaration::Shape::capture(intent.definition.column())?;
    out[KIND_AT] = shape.kind;
    out[SOURCE_AT] = intent.identity.source.tag();
    out[WIDTH_AT..WIDTH_AT + 2].copy_from_slice(&shape.width.to_le_bytes());
    out[PRECISION_AT] = shape.precision;
    out[SCALE_AT] = shape.scale;
    out[TABLE_LENGTH_AT..TABLE_LENGTH_AT + 2]
        .copy_from_slice(&(intent.table.len() as u16).to_le_bytes());
    out[COLUMN_LENGTH_AT..COLUMN_LENGTH_AT + 2]
        .copy_from_slice(&(column.len() as u16).to_le_bytes());
    out.extend_from_slice(intent.table.as_bytes());
    out.extend_from_slice(column.as_bytes());
    check(budget, cancelled)?;
    Ok(out)
}

pub(super) fn decode(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Intent, Error> {
    check(budget, cancelled)?;
    if bytes.len() < HEADER_BYTES || bytes.len() > MAX_BYTES {
        return Err(Error::InvalidRecord);
    }
    budget
        .charge(Resource::Steps, bytes.len() as u64)
        .map_err(InspectError::from)?;
    if bytes.get(..MAGIC.len()) != Some(MAGIC.as_slice())
        || u32::from_le_bytes(array(bytes, TOTAL_AT)?) as usize != bytes.len()
        || u16::from_le_bytes(array(bytes, PROFILE_AT)?) != PROFILE
        || u16::from_le_bytes(array(bytes, FLAGS_AT)?) != 0
        || bytes[POLICY_AT] != 0
        || bytes[COMPRESSION_AT] != 0
        || bytes[RESERVED_AT..HEADER_BYTES]
            .iter()
            .any(|byte| *byte != 0)
    {
        return Err(Error::InvalidRecord);
    }
    let before_length = u64::from_le_bytes(array(bytes, BEFORE_AT)?);
    let identity = Identity {
        source: super::SourceClass::from_tag(bytes[SOURCE_AT])?,
        native_table: i32::from_le_bytes(array(bytes, TABLE_ID_AT)?),
        definition: u32::from_le_bytes(array(bytes, DEFINITION_AT)?),
        native_number: u16::from_le_bytes(array(bytes, NUMBER_AT)?),
        column_id: u16::from_le_bytes(array(bytes, COLUMN_ID_AT)?),
        column_count: u16::from_le_bytes(array(bytes, COLUMN_COUNT_AT)?),
    };
    validate_identity(identity, before_length)?;
    let table_length = usize::from(u16::from_le_bytes(array(bytes, TABLE_LENGTH_AT)?));
    let column_length = usize::from(u16::from_le_bytes(array(bytes, COLUMN_LENGTH_AT)?));
    let split = HEADER_BYTES
        .checked_add(table_length)
        .ok_or(Error::InvalidRecord)?;
    if split.checked_add(column_length) != Some(bytes.len()) {
        return Err(Error::InvalidRecord);
    }
    let table = std::str::from_utf8(bytes.get(HEADER_BYTES..split).ok_or(Error::InvalidRecord)?)
        .map_err(|_| Error::InvalidRecord)?;
    let column = std::str::from_utf8(bytes.get(split..).ok_or(Error::InvalidRecord)?)
        .map_err(|_| Error::InvalidRecord)?;
    validate_table(table, budget, cancelled)?;
    // The actual declaration owner rejects annotations and names outside its
    // presently qualified ASCII construction language; no SQL is reconstructed.
    let shape = declaration::Shape {
        kind: bytes[KIND_AT],
        width: u16::from_le_bytes(array(bytes, WIDTH_AT)?),
        precision: bytes[PRECISION_AT],
        scale: bytes[SCALE_AT],
    };
    let definition =
        UnadornedFieldAdditionDefinition::capture(shape.column(column)?, budget, cancelled)?;
    let table = copy_table(table, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(Intent {
        table,
        definition,
        identity,
        before_length,
        before_binding: array(bytes, BINDING_AT)?,
    })
}
