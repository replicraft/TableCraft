//! Direct DATA/record/map framing; NULL catalog carriers are never fake values.
use super::{
    CreateDatabaseError,
    definition::{put16, put32},
    input::Cell,
    profile,
    request::Request,
};
use crate::{
    InspectError,
    format::{
        allocation::INLINE_BITMAP_OFFSET,
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        row,
    },
    header::check,
    table::scalar_encoding,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Guid, Value},
};
use tablecraft_schema::ScalarType;

pub(super) const DATA_HEADER_BYTES: usize = row::slot_offset(0);
const FREE_SPACE_OFFSET: usize = size_of::<u16>();
const DIRECTORY_ENTRY_BYTES: usize = row::SLOT_BYTES;
const FIXED_COLUMN_COUNT_BYTES: usize = row::COLUMN_COUNT_BYTES;
const BYTE_WIDTH: usize = size_of::<u8>();
const SHORT_WIDTH: usize = size_of::<i16>();
const LONG_WIDTH: usize = size_of::<i32>();
const WIDE_WIDTH: usize = size_of::<i64>();

pub(super) struct DataPage<'a> {
    bytes: &'a mut [u8],
    end: usize,
    rows: usize,
}

impl<'a> DataPage<'a> {
    pub fn new(bytes: &'a mut [u8], owner: usize) -> Result<Self, CreateDatabaseError> {
        if bytes.len() != PAGE_BYTES {
            return Err(CreateDatabaseError::Layout);
        }
        bytes[..DATA_SIGNATURE.len()].copy_from_slice(&DATA_SIGNATURE);
        put32(bytes, OWNER_OFFSET, owner as u32)?;
        Ok(Self {
            bytes,
            end: PAGE_BYTES,
            rows: 0,
        })
    }

    pub fn append(
        &mut self,
        length: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<&mut [u8], CreateDatabaseError> {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Rows, 1)
            .map_err(InspectError::from)?;
        budget
            .charge(Resource::Steps, (length + DIRECTORY_ENTRY_BYTES) as u64)
            .map_err(InspectError::from)?;
        let start = self
            .end
            .checked_sub(length)
            .ok_or(CreateDatabaseError::Layout)?;
        let directory_end = DATA_HEADER_BYTES + (self.rows + 1) * DIRECTORY_ENTRY_BYTES;
        if start < directory_end {
            return Err(CreateDatabaseError::Layout);
        }
        put16(
            self.bytes,
            DATA_HEADER_BYTES + self.rows * DIRECTORY_ENTRY_BYTES,
            start,
        )?;
        self.rows += 1;
        let end = self.end;
        self.end = start;
        self.bytes
            .get_mut(start..end)
            .ok_or(CreateDatabaseError::Layout)
    }

    pub fn finish(self) -> Result<(), CreateDatabaseError> {
        let free = self
            .end
            .checked_sub(DATA_HEADER_BYTES + self.rows * DIRECTORY_ENTRY_BYTES)
            .ok_or(CreateDatabaseError::Layout)?;
        put16(self.bytes, FREE_SPACE_OFFSET, free)?;
        put16(self.bytes, row::ROW_COUNT_OFFSET, self.rows)?;
        Ok(())
    }
}

pub(super) fn inline_map(
    record: &mut [u8],
    member: Option<usize>,
) -> Result<(), CreateDatabaseError> {
    if record.len() != profile::MAP_RECORD_BYTES {
        return Err(CreateDatabaseError::Layout);
    }
    // Explicit inline tag0, base0 and cleared membership come from the paid
    // zero initialization. The selected data page alone supplies a set bit.
    if let Some(member) = member {
        let offset = INLINE_BITMAP_OFFSET + member / u8::BITS as usize;
        let byte = record.get_mut(offset).ok_or(CreateDatabaseError::Layout)?;
        *byte |= 1 << (member % u8::BITS as usize);
    }
    Ok(())
}

pub(super) fn user(
    record: &mut [u8],
    request: &Request<'_>,
    row: usize,
) -> Result<(), CreateDatabaseError> {
    if record.len() != request.row_bytes {
        return Err(CreateDatabaseError::Layout);
    }
    let fields = request.fields();
    let mask_start = record.len() - fields.count().div_ceil(u8::BITS as usize);
    put16(record, 0, fields.count())?;
    for column in 0..fields.count() {
        let field = fields.get(column);
        let offset = field.offset.ok_or(CreateDatabaseError::Layout)?;
        let start = FIXED_COLUMN_COUNT_BYTES + offset;
        let destination = record
            .get_mut(start..start + field.width)
            .ok_or(CreateDatabaseError::Layout)?;
        let scalar = request.columns.scalar(column);
        let present = match request.rows.cell(row, column) {
            Cell::Int32(value) => {
                if scalar != ScalarType::Int32 || destination.len() != size_of::<i32>() {
                    return Err(CreateDatabaseError::Carrier);
                }
                destination.copy_from_slice(&value.to_le_bytes());
                true
            }
            Cell::Value(Value::Null) if scalar != ScalarType::Boolean => false,
            Cell::Value(Value::Boolean(value)) if scalar == ScalarType::Boolean => {
                if !destination.is_empty() {
                    return Err(CreateDatabaseError::Layout);
                }
                *value
            }
            Cell::Value(value) => {
                write_fixed(scalar, value, destination)?;
                true
            }
        };
        if present {
            record[mask_start + column / u8::BITS as usize] |= 1 << (column % u8::BITS as usize);
        }
    }
    Ok(())
}

fn write_fixed(
    scalar: ScalarType,
    value: &Value,
    destination: &mut [u8],
) -> Result<(), CreateDatabaseError> {
    match destination.len() {
        BYTE_WIDTH => copy_fixed::<BYTE_WIDTH>(scalar, value, destination),
        SHORT_WIDTH => copy_fixed::<SHORT_WIDTH>(scalar, value, destination),
        LONG_WIDTH => copy_fixed::<LONG_WIDTH>(scalar, value, destination),
        WIDE_WIDTH => copy_fixed::<WIDE_WIDTH>(scalar, value, destination),
        Guid::BYTE_LEN => copy_fixed::<{ Guid::BYTE_LEN }>(scalar, value, destination),
        _ => Err(CreateDatabaseError::Carrier),
    }
}

fn copy_fixed<const WIDTH: usize>(
    scalar: ScalarType,
    value: &Value,
    destination: &mut [u8],
) -> Result<(), CreateDatabaseError> {
    let bytes =
        scalar_encoding::encode::<WIDTH>(scalar, value).ok_or(CreateDatabaseError::Carrier)?;
    destination.copy_from_slice(&bytes);
    Ok(())
}

pub(super) fn catalog_length(name: &str) -> usize {
    let fields = profile::Fields::new(&profile::CATALOG_FIELDS);
    fields.fixed_end()
        + name.encode_utf16().count() * size_of::<u16>()
        + (fields.variable_count() + 2) * size_of::<u16>()
        + fields.count().div_ceil(u8::BITS as usize)
}

pub(super) fn catalog(
    record: &mut [u8],
    object: &profile::CatalogObject<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), CreateDatabaseError> {
    let profile::CatalogObject {
        id,
        parent,
        name,
        system,
    } = *object;
    if record.len() != catalog_length(name) {
        return Err(CreateDatabaseError::Layout);
    }
    let fields = profile::Fields::new(&profile::CATALOG_FIELDS);
    let fixed_end = fields.fixed_end();
    put16(record, 0, fields.count())?;
    put32(record, catalog_fixed_offset(profile::CATALOG_ID)?, id)?;
    put32(
        record,
        catalog_fixed_offset(profile::CATALOG_PARENT)?,
        parent,
    )?;
    let object_type = if id >= profile::ROOT_ID { 3 } else { 1 };
    put16(
        record,
        catalog_fixed_offset(profile::CATALOG_TYPE)?,
        object_type,
    )?;
    put32(
        record,
        catalog_fixed_offset(profile::CATALOG_FLAGS)?,
        if system { profile::SYSTEM_FLAGS } else { 0 },
    )?;

    let mut position = fixed_end;
    for unit in name.encode_utf16() {
        check(budget, cancelled)?;
        put16(record, position, usize::from(unit))?;
        position += size_of::<u16>();
    }
    let eod = position;
    // Name is the only present variable carrier; all other ordinary and LVAL
    // slots remain NULL at EOD. The actual SDK ordering puts LVAL slots last.
    put16(record, position, eod)?;
    position += size_of::<u16>();
    for slot in (0..fields.variable_count()).rev() {
        put16(record, position, if slot == 0 { fixed_end } else { eod })?;
        position += size_of::<u16>();
    }
    put16(record, position, fields.variable_count())?;
    position += size_of::<u16>();
    for field in [
        profile::CATALOG_ID,
        profile::CATALOG_PARENT,
        profile::CATALOG_NAME,
        profile::CATALOG_TYPE,
        profile::CATALOG_FLAGS,
        profile::CATALOG_CREATED,
        profile::CATALOG_UPDATED,
    ] {
        record[position + field / u8::BITS as usize] |= 1 << (field % u8::BITS as usize);
    }
    // Present creation/update DATE values are explicit +0 bits already in the
    // fixed prefix. All payload/link/owner carriers remain genuinely absent.
    if position + fields.count().div_ceil(u8::BITS as usize) != record.len() {
        return Err(CreateDatabaseError::Layout);
    }
    Ok(())
}

fn catalog_fixed_offset(column: usize) -> Result<usize, CreateDatabaseError> {
    profile::CATALOG_FIELDS[column]
        .offset
        .and_then(|offset| offset.checked_add(FIXED_COLUMN_COUNT_BYTES))
        .ok_or(CreateDatabaseError::Layout)
}
