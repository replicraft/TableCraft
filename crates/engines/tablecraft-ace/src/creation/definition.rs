//! Complete table declarations, including explicit column LVAL map pairs.
use super::{
    CreateDatabaseError,
    profile::{self, Fields},
};
use crate::format::{
    data_type::{OLE, SHORT_TEXT},
    definition as native_definition, layout,
    page::{PAGE_BYTES, TABLE_DEFINITION_SIGNATURE},
    row,
};
use crate::{InspectError, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

const HEADER_BYTES: usize = layout::COLUMN_BLOCK_OFFSET;
const COLUMN_BYTES: usize = 25;
const MAGIC: u32 = 1625;
const MAGIC_OFFSET: usize = 12;
const AUTO_SUPPORT_OFFSET: usize = 24;
const TYPE_OFFSET: usize = 40;
const MAX_COLUMNS_OFFSET: usize = 41;
const VARIABLE_COLUMNS_OFFSET: usize = 43;
const COLUMNS_OFFSET: usize = 45;
const USER_TABLE: u8 = 0x4e;
const SYSTEM_TABLE: u8 = 0x53;
const COLUMN_MAGIC: usize = 1;
const COLUMN_NUMBER: usize = 5;
const COLUMN_VARIABLE: usize = 7;
const COLUMN_ID: usize = 9;
const COLUMN_SORT: usize = 11;
const COLUMN_PRECISION: usize = 11;
const COLUMN_SCALE: usize = COLUMN_PRECISION + size_of::<u8>();
const COLUMN_FLAGS: usize = 15;
const COLUMN_OFFSET: usize = 21;
const COLUMN_WIDTH: usize = 23;
const LVAL_PAIR_BYTES: usize = size_of::<u16>() + 2 * size_of::<u32>();
const END_BYTES: usize = size_of::<u16>();
const FREE_SPACE_OFFSET: usize = size_of::<u16>();
const OWNED_MAP_OFFSET: usize = 55;
const CANDIDATE_MAP_OFFSET: usize = OWNED_MAP_OFFSET + size_of::<u32>();

pub(super) fn length(fields: Fields<'_>, catalog: bool) -> Result<usize, CreateDatabaseError> {
    let mut length = HEADER_BYTES
        .checked_add(
            fields
                .count()
                .checked_mul(COLUMN_BYTES)
                .ok_or(CreateDatabaseError::Layout)?,
        )
        .ok_or(CreateDatabaseError::Layout)?;
    for index in 0..fields.count() {
        length = length
            .checked_add(
                size_of::<u16>() + fields.get(index).name.encode_utf16().count() * size_of::<u16>(),
            )
            .ok_or(CreateDatabaseError::Layout)?;
    }
    length
        .checked_add(if catalog {
            profile::LONG_VALUE_COLUMNS.len() * LVAL_PAIR_BYTES
        } else {
            0
        })
        .and_then(|bytes| bytes.checked_add(END_BYTES))
        .ok_or(CreateDatabaseError::Layout)
}

pub(super) fn write(
    page: &mut [u8],
    fields: Fields<'_>,
    rows: usize,
    map_slot: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), CreateDatabaseError> {
    write_role(
        page,
        fields,
        rows,
        Role {
            user: map_slot == profile::USER_MAP_SLOT,
            catalog: map_slot == profile::CATALOG_MAP_SLOT,
            owned: row::reference(profile::MAP_PAGE as u32, map_slot as u8),
            candidate: row::reference(
                profile::MAP_PAGE as u32,
                (map_slot + profile::TABLE_COUNT) as u8,
            ),
        },
        budget,
        cancelled,
    )
}

pub(super) struct Role {
    pub user: bool,
    pub catalog: bool,
    pub owned: u32,
    pub candidate: u32,
}

pub(super) fn write_role(
    page: &mut [u8],
    fields: Fields<'_>,
    rows: usize,
    role: Role,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), CreateDatabaseError> {
    let catalog = role.catalog;
    let length = length(fields, catalog)?;
    if page.len() != PAGE_BYTES || length > PAGE_BYTES {
        return Err(CreateDatabaseError::Layout);
    }
    page[..TABLE_DEFINITION_SIGNATURE.len()].copy_from_slice(&TABLE_DEFINITION_SIGNATURE);
    put16(
        page,
        FREE_SPACE_OFFSET,
        PAGE_BYTES.saturating_sub(length + native_definition::PAGE_HEADER_BYTES),
    )?;
    put32(page, native_definition::LENGTH_OFFSET, length as u32)?;
    put32(page, MAGIC_OFFSET, MAGIC)?;
    put32(page, layout::RECORD_COUNT_OFFSET, rows as u32)?;
    page[AUTO_SUPPORT_OFFSET] = 1;
    page[TYPE_OFFSET] = if role.user { USER_TABLE } else { SYSTEM_TABLE };
    put16(page, MAX_COLUMNS_OFFSET, fields.count())?;
    put16(page, VARIABLE_COLUMNS_OFFSET, fields.variable_count())?;
    put16(page, COLUMNS_OFFSET, fields.count())?;
    put32(page, OWNED_MAP_OFFSET, role.owned)?;
    put32(page, CANDIDATE_MAP_OFFSET, role.candidate)?;

    let mut names = HEADER_BYTES + fields.count() * COLUMN_BYTES;
    let mut ordinary = 0;
    let mut long_value = fields.ordinary_variable_count();
    for index in 0..fields.count() {
        check(budget, cancelled)?;
        let field = fields.get(index);
        let start = HEADER_BYTES + index * COLUMN_BYTES;
        page[start] = field.kind;
        put32(page, start + COLUMN_MAGIC, MAGIC)?;
        put16(page, start + COLUMN_NUMBER, index)?;
        put16(page, start + COLUMN_ID, index)?;
        let variable = if field.offset.is_none() && field.kind == OLE {
            let next = long_value;
            long_value += 1;
            next
        } else {
            let next = ordinary;
            ordinary += usize::from(field.offset.is_none());
            next
        };
        put16(page, start + COLUMN_VARIABLE, variable)?;
        if field.kind == SHORT_TEXT {
            page[start + COLUMN_SORT..start + COLUMN_SORT + profile::GENERAL_SORT.len()]
                .copy_from_slice(&profile::GENERAL_SORT);
        } else if let Some(constraints) = field.decimal {
            page[start + COLUMN_PRECISION] = constraints.precision();
            page[start + COLUMN_SCALE] = constraints.scale();
        }
        page[start + COLUMN_FLAGS] = layout::UPDATABLE_FLAG
            | if field.offset.is_some() {
                layout::FIXED_FLAG
            } else {
                0
            };
        put16(page, start + COLUMN_OFFSET, field.offset.unwrap_or(0))?;
        put16(page, start + COLUMN_WIDTH, field.width)?;
        let name_bytes = field.name.encode_utf16().count() * size_of::<u16>();
        put16(page, names, name_bytes)?;
        names += size_of::<u16>();
        for unit in field.name.encode_utf16() {
            check(budget, cancelled)?;
            put16(page, names, usize::from(unit))?;
            names += size_of::<u16>();
        }
        budget
            .charge(Resource::Steps, COLUMN_BYTES as u64)
            .map_err(InspectError::from)?;
    }
    if catalog {
        for (index, field) in profile::LONG_VALUE_COLUMNS.into_iter().enumerate() {
            check(budget, cancelled)?;
            put16(page, names, field)?;
            put32(
                page,
                names + size_of::<u16>(),
                ((profile::MAP_PAGE as u32) << u8::BITS)
                    | (profile::FIRST_LONG_VALUE_MAP_SLOT + 2 * index) as u32,
            )?;
            put32(
                page,
                names + size_of::<u16>() + size_of::<u32>(),
                ((profile::MAP_PAGE as u32) << u8::BITS)
                    | (profile::FIRST_LONG_VALUE_MAP_SLOT + 2 * index + 1) as u32,
            )?;
            names += LVAL_PAIR_BYTES;
        }
    }
    put16(page, names, usize::from(u16::MAX))?;
    if names + END_BYTES != length {
        return Err(CreateDatabaseError::Layout);
    }
    Ok(())
}

pub(super) fn put16(
    bytes: &mut [u8],
    offset: usize,
    value: usize,
) -> Result<(), CreateDatabaseError> {
    let value = u16::try_from(value).map_err(|_| CreateDatabaseError::Layout)?;
    bytes
        .get_mut(offset..offset + size_of::<u16>())
        .ok_or(CreateDatabaseError::Layout)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

pub(super) fn put32(
    bytes: &mut [u8],
    offset: usize,
    value: u32,
) -> Result<(), CreateDatabaseError> {
    bytes
        .get_mut(offset..offset + size_of::<u32>())
        .ok_or(CreateDatabaseError::Layout)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}
