//! Closed version5 framing; target variants fix every field policy independently.
use super::{
    BINDING_BYTES, FieldAdditionIntentError as Error, NativeFieldAdditionIntent as Intent, binding,
    copy_name, validate_name,
};
use crate::{FieldAdditionTarget, InspectError, header::check};
use tablecraft_core::limits::ResourceBudget;

const MAGIC: &[u8; 8] = b"TCAC0005";
const PROFILE: u16 = 5;
const NUMBER_LONG_ZERO: u16 = 1;
const SHORT_TEXT: u16 = 2;
const COMPRESSION_DISABLED: u16 = 0;
const COMPRESSION_ENABLED: u16 = 1;
const MAX_TEXT_UNITS: usize = 255;
const TOTAL_AT: usize = MAGIC.len();
const PROFILE_AT: usize = TOTAL_AT + size_of::<u32>();
const TARGET_AT: usize = PROFILE_AT + size_of::<u16>();
const WIDTH_AT: usize = TARGET_AT + size_of::<u16>();
const COMPRESSION_AT: usize = WIDTH_AT + size_of::<u16>();
const BEFORE_LENGTH_AT: usize = COMPRESSION_AT + size_of::<u16>();
const BINDING_AT: usize = BEFORE_LENGTH_AT + size_of::<u64>();
const TABLE_LENGTH_AT: usize = BINDING_AT + BINDING_BYTES;
const COLUMN_LENGTH_AT: usize = TABLE_LENGTH_AT + size_of::<u16>();
pub(super) const HEADER_BYTES: usize = COLUMN_LENGTH_AT + size_of::<u16>();

fn field<const N: usize>(input: &[u8], at: usize) -> Result<[u8; N], Error> {
    input
        .get(at..at + N)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(Error::InvalidRecord)
}

fn target(tag: u16, width: u16, compression: u16) -> Result<FieldAdditionTarget, Error> {
    match (tag, usize::from(width), compression) {
        (NUMBER_LONG_ZERO, 0, COMPRESSION_DISABLED) => Ok(FieldAdditionTarget::NumberLongZero),
        (SHORT_TEXT, width, COMPRESSION_DISABLED | COMPRESSION_ENABLED)
            if (1..=MAX_TEXT_UNITS).contains(&width) =>
        {
            Ok(FieldAdditionTarget::ShortText {
                max_utf16_units: width,
                unicode_compression: compression == COMPRESSION_ENABLED,
            })
        }
        _ => Err(Error::InvalidRecord),
    }
}

fn target_fields(value: FieldAdditionTarget) -> Result<(u16, u16, u16), Error> {
    match value {
        FieldAdditionTarget::NumberLongZero => Ok((NUMBER_LONG_ZERO, 0, COMPRESSION_DISABLED)),
        FieldAdditionTarget::ShortText {
            max_utf16_units,
            unicode_compression,
        } if (1..=MAX_TEXT_UNITS).contains(&max_utf16_units) => Ok((
            SHORT_TEXT,
            max_utf16_units as u16,
            if unicode_compression {
                COMPRESSION_ENABLED
            } else {
                COMPRESSION_DISABLED
            },
        )),
        _ => Err(Error::InvalidRecord),
    }
}

pub(super) fn decode(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Intent, Error> {
    check(budget, cancelled)?;
    if !(HEADER_BYTES..=Intent::MAX_ENCODED_BYTES).contains(&bytes.len())
        || field::<8>(bytes, 0)? != *MAGIC
        || u32::from_le_bytes(field(bytes, TOTAL_AT)?) as usize != bytes.len()
        || u16::from_le_bytes(field(bytes, PROFILE_AT)?) != PROFILE
    {
        return Err(Error::InvalidRecord);
    }
    let target = target(
        u16::from_le_bytes(field(bytes, TARGET_AT)?),
        u16::from_le_bytes(field(bytes, WIDTH_AT)?),
        u16::from_le_bytes(field(bytes, COMPRESSION_AT)?),
    )?;
    let before_length = u64::from_le_bytes(field(bytes, BEFORE_LENGTH_AT)?);
    binding::length(before_length)?;
    let table_length = usize::from(u16::from_le_bytes(field(bytes, TABLE_LENGTH_AT)?));
    let column_length = usize::from(u16::from_le_bytes(field(bytes, COLUMN_LENGTH_AT)?));
    if HEADER_BYTES + table_length + column_length != bytes.len() {
        return Err(Error::InvalidRecord);
    }
    let split = HEADER_BYTES + table_length;
    let table =
        std::str::from_utf8(&bytes[HEADER_BYTES..split]).map_err(|_| Error::InvalidRecord)?;
    let column = std::str::from_utf8(&bytes[split..]).map_err(|_| Error::InvalidRecord)?;
    validate_name(table, budget, cancelled)?;
    validate_name(column, budget, cancelled)?;
    Ok(Intent {
        table: copy_name(table, budget, cancelled)?,
        column: copy_name(column, budget, cancelled)?,
        target,
        before_length,
        before_binding: field(bytes, BINDING_AT)?,
    })
}

pub(super) fn encode(
    intent: &Intent,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    let (target, width, compression) = target_fields(intent.target())?;
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
    bytes.extend_from_slice(&target.to_le_bytes());
    bytes.extend_from_slice(&width.to_le_bytes());
    bytes.extend_from_slice(&compression.to_le_bytes());
    bytes.extend_from_slice(&intent.before_length.to_le_bytes());
    bytes.extend_from_slice(&intent.before_binding);
    bytes.extend_from_slice(&(intent.table().len() as u16).to_le_bytes());
    bytes.extend_from_slice(&(intent.column().len() as u16).to_le_bytes());
    bytes.extend_from_slice(intent.table().as_bytes());
    bytes.extend_from_slice(intent.column().as_bytes());
    check(budget, cancelled)?;
    Ok(bytes)
}
