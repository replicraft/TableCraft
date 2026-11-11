//! Direct DATA/record/map framing; NULL catalog carriers are never fake values.
mod user;
pub(super) use user::{UserRow, write as user_row};

use super::{
    CreateDatabaseError,
    definition::{put16, put32},
    profile,
    request::Request,
    stored_text,
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
const CATALOG_PROPERTY_FIELD: usize = 14;
const CATALOG_PROPERTY_VARIABLE: usize = 8;
#[cfg(feature = "journal")]
const EXTERNAL_PROPERTY_BYTES: usize = 3 * size_of::<u32>();
#[cfg(feature = "journal")]
const SINGLE_PROPERTY_FLAG: u32 = 0x4000_0000;
pub(super) const DATABASE_SYSTEM_FLAGS: u32 = 0x8000_0000;

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
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), CreateDatabaseError> {
    let payloads = request
        .variable
        .as_ref()
        .map(|prepared| prepared.row_payloads(row))
        .transpose()?;
    user_row(
        record,
        UserRow {
            fields: request.fields(),
            columns: request.columns,
            cells: request.rows,
            index: row,
            payloads,
            length: request.row_length(row),
        },
        budget,
        cancelled,
    )
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

pub(super) fn catalog_length(name: stored_text::Plan<'_>) -> usize {
    let fields = profile::Fields::new(&profile::CATALOG_FIELDS);
    fields.fixed_end()
        + name.byte_len()
        + (fields.variable_count() + 2) * size_of::<u16>()
        + fields.count().div_ceil(u8::BITS as usize)
}

pub(super) fn catalog_length_with_properties(
    name: stored_text::Plan<'_>,
    properties: Option<&super::properties::Prepared<'_>>,
) -> Result<usize, CreateDatabaseError> {
    catalog_length(name)
        .checked_add(properties.map_or(0, |properties| properties.inline_length()))
        .ok_or(CreateDatabaseError::PageCapacity)
}

pub(super) fn catalog(
    record: &mut [u8],
    object: &profile::CatalogObject<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), CreateDatabaseError> {
    catalog_with_properties(record, object, None, budget, cancelled)
}

pub(super) fn catalog_with_properties(
    record: &mut [u8],
    object: &profile::CatalogObject<'_>,
    properties: Option<&super::properties::Prepared<'_>>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), CreateDatabaseError> {
    catalog_record(
        record,
        object,
        false,
        properties.map_or(0, |properties| properties.inline_length()),
        |destination, budget, cancelled| {
            if let Some(properties) = properties {
                properties.write_inline(destination, budget, cancelled)?;
            }
            Ok(())
        },
        budget,
        cancelled,
    )
}

/// A new user role with an external, explicitly linked native MR2 body.
pub(super) fn catalog_with_linked_properties(
    record: &mut [u8],
    object: &profile::CatalogObject<'_>,
    reference: u32,
    length: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), CreateDatabaseError> {
    let length = u32::try_from(length).map_err(|_| CreateDatabaseError::Layout)?;
    if length == 0 || length > crate::LongValueReadLimits::MAX_BYTES {
        return Err(CreateDatabaseError::Layout);
    }
    catalog_record(
        record,
        object,
        false,
        3 * size_of::<u32>(),
        |destination, budget, cancelled| {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, destination.len() as u64)
                .map_err(InspectError::from)?;
            put32(destination, 0, length)?;
            put32(destination, size_of::<u32>(), reference)?;
            put32(destination, 2 * size_of::<u32>(), 0)
        },
        budget,
        cancelled,
    )
}

/// A database metadata role with one already qualified external property body.
#[cfg(feature = "journal")]
pub(super) fn database_properties_catalog(
    record: &mut [u8],
    id: u32,
    parent: u32,
    reference: u32,
    length: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), CreateDatabaseError> {
    let object = profile::CatalogObject {
        id,
        parent,
        name: stored_text::Plan::ascii_literal("MSysDb"),
        system: true,
    };
    let length = u32::try_from(length).map_err(|_| CreateDatabaseError::Layout)?;
    if length >= SINGLE_PROPERTY_FLAG {
        return Err(CreateDatabaseError::Layout);
    }
    catalog_record(
        record,
        &object,
        true,
        EXTERNAL_PROPERTY_BYTES,
        |destination, budget, cancelled| {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, EXTERNAL_PROPERTY_BYTES as u64)
                .map_err(InspectError::from)?;
            put32(destination, 0, length | SINGLE_PROPERTY_FLAG)?;
            put32(destination, size_of::<u32>(), reference)?;
            put32(destination, 2 * size_of::<u32>(), 0)?;
            Ok(())
        },
        budget,
        cancelled,
    )
}

fn catalog_record(
    record: &mut [u8],
    object: &profile::CatalogObject<'_>,
    database: bool,
    property_bytes: usize,
    write_properties: impl FnOnce(
        &mut [u8],
        &mut ResourceBudget,
        &dyn Fn() -> bool,
    ) -> Result<(), CreateDatabaseError>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), CreateDatabaseError> {
    let profile::CatalogObject {
        id,
        parent,
        name,
        system,
    } = *object;
    if catalog_length(name).checked_add(property_bytes) != Some(record.len()) {
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
    let object_type = if database {
        2
    } else if id >= profile::ROOT_ID {
        3
    } else {
        1
    };
    put16(
        record,
        catalog_fixed_offset(profile::CATALOG_TYPE)?,
        object_type,
    )?;
    put32(
        record,
        catalog_fixed_offset(profile::CATALOG_FLAGS)?,
        if database {
            DATABASE_SYSTEM_FLAGS
        } else if system {
            profile::SYSTEM_FLAGS
        } else {
            0
        },
    )?;

    let mut position = fixed_end;
    name.write(
        &mut record[position..position + name.byte_len()],
        budget,
        cancelled,
    )?;
    position += name.byte_len();
    let name_end = position;
    if property_bytes != 0 {
        let end = position
            .checked_add(property_bytes)
            .ok_or(CreateDatabaseError::Layout)?;
        write_properties(
            record
                .get_mut(position..end)
                .ok_or(CreateDatabaseError::Layout)?,
            budget,
            cancelled,
        )?;
        position = end;
    }
    let eod = position;
    // Name and an explicitly selected inline LvProp are the only variable carriers;
    // the None branch retains the original all-other-NULL spans byte-exact.
    // NULL spans before LvProp stay at Name's end; later NULL spans stay at
    // EOD. Native17 declarations retain their interleaved variable numbers;
    // no payload slot is synthesized for an absent carrier.
    put16(record, position, eod)?;
    position += size_of::<u16>();
    for slot in (0..fields.variable_count()).rev() {
        let start = if slot == 0 {
            fixed_end
        } else if property_bytes != 0 && slot <= CATALOG_PROPERTY_VARIABLE {
            name_end
        } else {
            eod
        };
        put16(record, position, start)?;
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
        if database && matches!(field, profile::CATALOG_CREATED | profile::CATALOG_UPDATED) {
            continue;
        }
        record[position + field / u8::BITS as usize] |= 1 << (field % u8::BITS as usize);
    }
    if property_bytes != 0 {
        record[position + CATALOG_PROPERTY_FIELD / u8::BITS as usize] |=
            1 << (CATALOG_PROPERTY_FIELD % u8::BITS as usize);
    }
    // Present creation/update DATE values are explicit +0 bits already in the
    // fixed prefix. All unselected payload/link/owner carriers remain genuinely absent.
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
