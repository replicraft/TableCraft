//! Fresh manual primary population, before any output image exists.
use super::{
    CreateDatabaseError,
    definition::{put16, put32},
    input::{Cell, Columns, Rows},
    profile, request,
};
use crate::{
    IndexConstraintViolation, InspectError,
    format::{page::PAGE_BYTES, row},
    header::check,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

const COUNT_BYTES: usize = 3 * size_of::<u32>();
const PHYSICAL_BYTES: usize = 52;
const LOGICAL_BYTES: usize = 28;
const UNUSED_KEY: u16 = u16::MAX;
const KEY_SLOTS: usize = 10;
const KEY_AT: usize = 4;
const KEY_BYTES: usize = size_of::<u16>() + size_of::<u8>();
const MAP_AT: usize = 34;
const ROOT_AT: usize = 38;
const FLAGS_AT: usize = 46;
const PHYSICAL_MAGIC: u32 = 1923;
const LOGICAL_MAGIC: u32 = 1625;
const PRIMARY_FLAGS: u8 = 0x89;
const PRIMARY_KIND: u8 = 1;
const NO_REFERENCE_AT: usize = 13;
const LOGICAL_KIND_AT: usize = 23;
const LEAF_KIND: u8 = 4;
const PAGE_VERSION: u8 = 1;
const OWNER_AT: usize = 4;
const FREE_AT: usize = 2;
const MASK_AT: usize = 27;
const ENTRY_AT: usize = 480;
const MARKER_BYTES: usize = size_of::<u8>();
const ORDERED_BYTES: usize = size_of::<i32>();
const ROW_AT: usize = MARKER_BYTES + ORDERED_BYTES;
const ENTRY_BYTES: usize = ROW_AT + size_of::<u32>();
const VALUE_MARKER: u8 = 0x7f;
const SIGN_BIT: u32 = 0x8000_0000;

#[derive(Clone, Copy)]
struct Entry {
    value: i32,
    slot: u8,
}

pub(super) struct Prepared<'a> {
    name: &'a str,
    name_bytes: usize,
    column: usize,
    entries: [Entry; profile::MAX_ROWS],
    count: usize,
}

pub(super) fn capture<'a>(
    columns: Columns<'a>,
    rows: Rows<'a>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<Prepared<'a>>, CreateDatabaseError> {
    check(budget, cancelled)?;
    let mut selected = None;
    for column in 0..columns.count() {
        check(budget, cancelled)?;
        if let Some(name) = columns.primary_key_name(column) {
            if selected.is_some() || columns.scalar(column) != ScalarType::Int32 {
                return Err(CreateDatabaseError::Declaration);
            }
            selected = Some((column, name));
        }
    }
    let Some((column, name)) = selected else {
        return Ok(None);
    };
    request::name(name, budget, cancelled)?;
    if rows.count() > profile::MAX_ROWS {
        return Err(CreateDatabaseError::RowLimit);
    }
    let active = rows
        .count()
        .checked_mul(ENTRY_BYTES)
        .ok_or(CreateDatabaseError::PageCapacity)?;
    if active > PAGE_BYTES - ENTRY_AT {
        return Err(CreateDatabaseError::PageCapacity);
    }
    let mut name_bytes = 0;
    for _ in name.encode_utf16() {
        check(budget, cancelled)?;
        name_bytes += size_of::<u16>();
    }
    let mut prepared = Prepared {
        name,
        name_bytes,
        column,
        entries: [Entry { value: 0, slot: 0 }; profile::MAX_ROWS],
        count: rows.count(),
    };
    for ordinal in 0..rows.count() {
        check(budget, cancelled)?;
        let value = match rows.cell(ordinal, column) {
            Cell::Int32(value) => value,
            Cell::Value(Value::Int32(value)) => *value,
            Cell::Value(Value::Null) => return Err(IndexConstraintViolation::NullKey.into()),
            _ => return Err(CreateDatabaseError::Carrier),
        };
        prepared.entries[ordinal] = Entry {
            value,
            slot: u8::try_from(ordinal).map_err(|_| CreateDatabaseError::Layout)?,
        };
    }
    let levels = rows.count().checked_ilog2().unwrap_or(0) as usize + 1;
    budget
        .charge(
            Resource::Steps,
            (rows.count() * levels * size_of::<Entry>()) as u64,
        )
        .map_err(InspectError::from)?;
    prepared.entries[..prepared.count].sort_unstable_by_key(|entry| (entry.value, entry.slot));
    for pair in prepared.entries[..prepared.count].windows(2) {
        check(budget, cancelled)?;
        if pair[0].value == pair[1].value {
            return Err(IndexConstraintViolation::DuplicateKey.into());
        }
    }
    check(budget, cancelled)?;
    Ok(Some(prepared))
}

impl Prepared<'_> {
    pub fn extra_definition_bytes(&self) -> usize {
        COUNT_BYTES + PHYSICAL_BYTES + LOGICAL_BYTES + size_of::<u16>() + self.name_bytes
    }

    pub fn column_shift(&self) -> usize {
        COUNT_BYTES
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn write_tail(
        &self,
        page: &mut [u8],
        at: &mut usize,
        root: usize,
        map: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), CreateDatabaseError> {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, (PHYSICAL_BYTES + LOGICAL_BYTES) as u64)
            .map_err(InspectError::from)?;
        let physical = page
            .get_mut(*at..*at + PHYSICAL_BYTES)
            .ok_or(CreateDatabaseError::Layout)?;
        put32(physical, 0, PHYSICAL_MAGIC)?;
        for key in 0..KEY_SLOTS {
            put16(physical, KEY_AT + key * KEY_BYTES, usize::from(UNUSED_KEY))?;
        }
        put16(physical, KEY_AT, self.column)?;
        physical[KEY_AT + size_of::<u16>()] = 1;
        put32(physical, MAP_AT, map)?;
        put32(
            physical,
            ROOT_AT,
            u32::try_from(root).map_err(|_| CreateDatabaseError::Layout)?,
        )?;
        physical[FLAGS_AT] = PRIMARY_FLAGS;
        *at += PHYSICAL_BYTES;
        let logical = page
            .get_mut(*at..*at + LOGICAL_BYTES)
            .ok_or(CreateDatabaseError::Layout)?;
        put32(logical, 0, LOGICAL_MAGIC)?;
        put32(logical, NO_REFERENCE_AT, u32::MAX)?;
        logical[LOGICAL_KIND_AT] = PRIMARY_KIND;
        *at += LOGICAL_BYTES;
        put16(page, *at, self.name_bytes)?;
        *at += size_of::<u16>();
        for unit in self.name.encode_utf16() {
            check(budget, cancelled)?;
            put16(page, *at, usize::from(unit))?;
            *at += size_of::<u16>();
        }
        Ok(())
    }

    pub fn write_leaf(
        &self,
        page: &mut [u8],
        definition: usize,
        data: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), CreateDatabaseError> {
        check(budget, cancelled)?;
        if page.len() != PAGE_BYTES {
            return Err(CreateDatabaseError::Layout);
        }
        page[..2].copy_from_slice(&[LEAF_KIND, PAGE_VERSION]);
        put32(
            page,
            OWNER_AT,
            u32::try_from(definition).map_err(|_| CreateDatabaseError::Layout)?,
        )?;
        let mut active = 0;
        for entry in &self.entries[..self.count] {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, ENTRY_BYTES as u64)
                .map_err(InspectError::from)?;
            let at = ENTRY_AT + active;
            page[at] = VALUE_MARKER;
            let ordered = u32::from_le_bytes(entry.value.to_le_bytes()) ^ SIGN_BIT;
            page[at + MARKER_BYTES..at + ROW_AT].copy_from_slice(&ordered.to_be_bytes());
            let reference = row::reference(
                u32::try_from(data).map_err(|_| CreateDatabaseError::Layout)?,
                entry.slot,
            );
            page[at + ROW_AT..at + ENTRY_BYTES].copy_from_slice(&reference.to_be_bytes());
            active += ENTRY_BYTES;
            page[MASK_AT + active / u8::BITS as usize] |= 1 << (active % u8::BITS as usize);
        }
        put16(page, FREE_AT, PAGE_BYTES - ENTRY_AT - active)?;
        check(budget, cancelled)?;
        Ok(())
    }
}
