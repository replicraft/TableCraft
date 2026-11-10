//! Closed TCAC0008/9 declarations share framing, never decoder fallback.
use super::{
    super::binding,
    record::{
        BINDING_BYTES, CurrentFieldAdditionIntentError as Error,
        NativeCurrentFieldAdditionIntent as Intent, copy_name, validate_column, validate_name,
    },
};
use crate::{FieldAdditionTarget, InspectError, header::check};
use tablecraft_core::limits::ResourceBudget;

const MAGIC: &[u8; 8] = b"TCAC0008";
const PROFILE: u16 = 8;
const NUMBER_LONG_ZERO: u16 = 1;
const SHORT_TEXT_MAGIC: &[u8; 8] = b"TCAC0009";
const SHORT_TEXT_PROFILE: u16 = 9;
const SHORT_TEXT: u16 = 2;
const MAX_SHORT_TEXT_UNITS: u16 = 255;
const RESERVED_WIDTH: u16 = 0;
const RESERVED_COMPRESSION: u16 = 0;
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

pub(super) enum Language {
    Number,
    ShortText,
}

fn field<const N: usize>(input: &[u8], at: usize) -> Result<[u8; N], Error> {
    input
        .get(at..at + N)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(Error::InvalidRecord)
}

pub(super) fn decode(
    bytes: &[u8],
    language: Language,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Intent, Error> {
    check(budget, cancelled)?;
    let (magic, profile, tag) = match language {
        Language::Number => (MAGIC, PROFILE, NUMBER_LONG_ZERO),
        Language::ShortText => (SHORT_TEXT_MAGIC, SHORT_TEXT_PROFILE, SHORT_TEXT),
    };
    if !(HEADER_BYTES..=Intent::MAX_ENCODED_BYTES).contains(&bytes.len())
        || field::<8>(bytes, 0)? != *magic
        || u32::from_le_bytes(field(bytes, TOTAL_AT)?) as usize != bytes.len()
        || u16::from_le_bytes(field(bytes, PROFILE_AT)?) != profile
        || u16::from_le_bytes(field(bytes, TARGET_AT)?) != tag
    {
        return Err(Error::InvalidRecord);
    }
    let width = u16::from_le_bytes(field(bytes, WIDTH_AT)?);
    let compression = u16::from_le_bytes(field(bytes, COMPRESSION_AT)?);
    let target = match language {
        Language::Number if width == RESERVED_WIDTH && compression == RESERVED_COMPRESSION => {
            FieldAdditionTarget::NumberLongZero
        }
        Language::ShortText if (1..=MAX_SHORT_TEXT_UNITS).contains(&width) && compression <= 1 => {
            FieldAdditionTarget::ShortText {
                max_utf16_units: usize::from(width),
                unicode_compression: compression == 1,
            }
        }
        _ => return Err(Error::InvalidRecord),
    };

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
    validate_column(column, budget, cancelled)?;

    Ok(Intent {
        target,
        table: copy_name(table, budget, cancelled)?,
        column: copy_name(column, budget, cancelled)?,
        before_length,
        before_binding: field(bytes, BINDING_AT)?,
    })
}

pub(super) fn encode(
    intent: &Intent,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    check(budget, cancelled)?;
    validate_name(intent.table(), budget, cancelled)?;
    validate_column(intent.column(), budget, cancelled)?;
    let (magic, profile, tag, width, compression) = match intent.target() {
        FieldAdditionTarget::NumberLongZero => (
            MAGIC,
            PROFILE,
            NUMBER_LONG_ZERO,
            RESERVED_WIDTH,
            RESERVED_COMPRESSION,
        ),
        FieldAdditionTarget::ShortText {
            max_utf16_units,
            unicode_compression,
        } => {
            let width = u16::try_from(max_utf16_units).map_err(|_| Error::InvalidRecord)?;
            if !(1..=MAX_SHORT_TEXT_UNITS).contains(&width) {
                return Err(Error::InvalidRecord);
            }
            (
                SHORT_TEXT_MAGIC,
                SHORT_TEXT_PROFILE,
                SHORT_TEXT,
                width,
                u16::from(unicode_compression),
            )
        }
    };
    let length = HEADER_BYTES + intent.table().len() + intent.column().len();
    budget.charge_allocation(length as u64, 1)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    bytes.extend_from_slice(magic);
    bytes.extend_from_slice(&(length as u32).to_le_bytes());
    bytes.extend_from_slice(&profile.to_le_bytes());
    bytes.extend_from_slice(&tag.to_le_bytes());
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
