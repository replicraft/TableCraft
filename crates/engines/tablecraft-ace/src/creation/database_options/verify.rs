//! Exact complement and independent native metadata/identity observation.
use super::{
    CATALOG_DEFINITION, CATALOG_PAGE, DATABASE_ID, DATABASE_PARENT, DatabaseOptionsError,
    LVAL_OWNER, MAP_HEADER_BYTES, MAP_PAGE, ORIGINAL_OBJECTS, PROPERTY_CANDIDATE, PROPERTY_OWNED,
    properties,
};
use crate::{
    InspectError, NativeImage, PropertyGroupKind,
    creation::{catalog_page::Page, record::DATABASE_SYSTEM_FLAGS},
    format::{
        bytes::u16_at,
        data_type,
        layout::RECORD_COUNT_OFFSET,
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{row_count, row_range},
        row::{OFFSET_MASK, ROW_COUNT_OFFSET, reference, slot_offset},
    },
    header::check,
    inspect_catalog, inspect_database_properties,
    table::mutation::catalog_property::ledger,
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn candidate(
    before: &NativeImage,
    after: &[u8],
    new_page: u32,
    catalog: &Page,
    maps: &Page,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), DatabaseOptionsError> {
    check(budget, cancelled)?;
    if after.len() != before.as_bytes().len() + PAGE_BYTES
        || new_page as usize * PAGE_BYTES != before.as_bytes().len()
    {
        return Err(DatabaseOptionsError::Preservation);
    }
    let base = CATALOG_PAGE * PAGE_BYTES;
    let updated = Page::capture(
        &after[base..base + PAGE_BYTES],
        CATALOG_DEFINITION as u32,
        budget,
        cancelled,
    )?;
    if updated.count != catalog.count + 1 || updated.records[..catalog.count] != catalog.records {
        return Err(DatabaseOptionsError::Preservation);
    }
    let row = &updated.records[catalog.count];
    let map_bit = |slot: usize| {
        MAP_PAGE * PAGE_BYTES + maps.records[slot].start + MAP_HEADER_BYTES + new_page as usize / 8
    };
    let owned = map_bit(PROPERTY_OWNED);
    let candidate = map_bit(PROPERTY_CANDIDATE);
    for at in [owned, candidate] {
        if after[at] != before.as_bytes()[at] | (1 << (new_page % 8)) {
            return Err(DatabaseOptionsError::Preservation);
        }
    }
    let count = CATALOG_DEFINITION * PAGE_BYTES + RECORD_COUNT_OFFSET;
    if after[count..count + size_of::<u32>()] != (ORIGINAL_OBJECTS as u32 + 1).to_le_bytes() {
        return Err(DatabaseOptionsError::Preservation);
    }
    let directory = base + 14 + catalog.count * size_of::<u16>();
    let scope = [
        base + 2..base + 4,
        base + 12..base + 14,
        directory..directory + size_of::<u16>(),
        base + row.start..base + row.end,
        count..count + size_of::<u32>(),
        owned..owned + 1,
        candidate..candidate + 1,
    ];
    budget
        .charge(
            Resource::Steps,
            (before.as_bytes().len() * (scope.len() + 1)) as u64,
        )
        .map_err(InspectError::from)?;
    for (offset, (original, replacement)) in before.as_bytes().iter().zip(after).enumerate() {
        if offset.is_multiple_of(PAGE_BYTES) {
            check(budget, cancelled)?;
        }
        if original != replacement && !scope.iter().any(|range| range.contains(&offset)) {
            return Err(DatabaseOptionsError::Preservation);
        }
    }

    let native = inspect_catalog(
        &mut Cursor::new(after),
        before.catalog().catalog().scope(),
        budget,
        cancelled,
    )?;
    let original_header = before.catalog().header();
    let updated_header = native.header();
    // Page count is derived from physical EOF, not a changed stored header byte.
    // The complement proof above already preserves the complete original header.
    if updated_header.format() != original_header.format()
        || updated_header.page_size() != original_header.page_size()
        || updated_header.subversion() != original_header.subversion()
        || updated_header.encoded_pages() != original_header.encoded_pages()
        || original_header.page_count().checked_add(1) != Some(updated_header.page_count())
        || native.origins().len() != before.catalog().origins().len() + 1
    {
        return Err(DatabaseOptionsError::Preservation);
    }
    for (original, replacement) in before.catalog().origins().iter().zip(native.origins()) {
        check(budget, cancelled)?;
        if original.object_id() != replacement.object_id()
            || original.native_id() != replacement.native_id()
            || original.parent_id() != replacement.parent_id()
            || original.type_code() != replacement.type_code()
            || original.flags() != replacement.flags()
            || original.page() != replacement.page()
            || original.slot() != replacement.slot()
        {
            return Err(DatabaseOptionsError::Preservation);
        }
    }
    let db = native
        .origins()
        .last()
        .ok_or(DatabaseOptionsError::Preservation)?;
    if db.native_id() != DATABASE_ID as i32
        || db.parent_id() != DATABASE_PARENT as i32
        || db.type_code() != 2
        || db.flags() != DATABASE_SYSTEM_FLAGS
        || db.page() != CATALOG_PAGE as u32
        || usize::from(db.slot()) != ORIGINAL_OBJECTS
    {
        return Err(DatabaseOptionsError::Preservation);
    }
    let properties = inspect_database_properties(
        &mut Cursor::new(after),
        native.catalog().scope(),
        budget,
        cancelled,
    )?
    .ok_or(DatabaseOptionsError::Preservation)?;
    let [group] = properties.groups() else {
        return Err(DatabaseOptionsError::Preservation);
    };
    if properties.object_id() != db.object_id()
        || group.kind() != PropertyGroupKind::Table
        || !group.name().is_empty()
        || group.properties().len() != properties::NAMES.len()
        || properties.raw_bytes().len() != properties::ENCODED_BYTES
    {
        return Err(DatabaseOptionsError::Preservation);
    }
    for (property, name) in group.properties().iter().zip(properties::NAMES) {
        check(budget, cancelled)?;
        if property.name() != name
            || property.native_type() != data_type::INT32
            || property.flags() != properties::DDL_FLAG
            || property.raw_value() != 0_i32.to_le_bytes()
        {
            return Err(DatabaseOptionsError::Preservation);
        }
    }
    let page = &after[new_page as usize * PAGE_BYTES..];
    if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
        || page[OWNER_OFFSET..OWNER_OFFSET + size_of::<u32>()] != (LVAL_OWNER as u32).to_le_bytes()
        || page[OWNER_OFFSET + size_of::<u32>()..ROW_COUNT_OFFSET] != [0; size_of::<u32>()]
        || row_count(page)? != 1
    {
        return Err(DatabaseOptionsError::Preservation);
    }
    let (raw_slot, start, end) = row_range(page, 0)?;
    if raw_slot & !OFFSET_MASK != 0
        || end != PAGE_BYTES
        || end - start != properties::ENCODED_BYTES
        || page[start..end] != *properties.raw_bytes()
        || usize::from(u16_at(page, DATA_SIGNATURE.len())?) != start - slot_offset(1)
        || page[slot_offset(1)..start].iter().any(|byte| *byte != 0)
    {
        return Err(DatabaseOptionsError::Preservation);
    }
    // The database role and its observed body are distinct from the catalog
    // column's ownership. Bind its actual descriptor to the produced row too.
    let ledger = ledger::capture(after, &native, budget, cancelled)?;
    if !ledger.sole_single(
        db.object_id(),
        reference(new_page, 0),
        properties::ENCODED_BYTES,
        budget,
        cancelled,
    )? {
        return Err(DatabaseOptionsError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}
