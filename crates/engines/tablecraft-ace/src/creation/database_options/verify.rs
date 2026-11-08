//! Exact complement and independent native metadata/identity observation.
use super::{
    CATALOG_DEFINITION, CATALOG_PAGE, DATABASE_ID, DATABASE_PARENT, DatabaseOptionsError,
    MAP_HEADER_BYTES, MAP_PAGE, ORIGINAL_OBJECTS, PROPERTY_CANDIDATE, PROPERTY_OWNED,
};
use crate::{
    InspectError, NativeImage, PropertyGroupKind,
    creation::catalog_page::Page,
    format::{layout::RECORD_COUNT_OFFSET, page::PAGE_BYTES},
    header::check,
    inspect_catalog, inspect_database_properties,
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
    if after.len() != before.as_bytes().len() + PAGE_BYTES {
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
    if native.header() != before.catalog().header()
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
        || db.flags() != 0x8000_0000
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
        || group.properties().len() != 2
    {
        return Err(DatabaseOptionsError::Preservation);
    }
    for (property, name) in group
        .properties()
        .iter()
        .zip(["Track Name AutoCorrect Info", "Perform Name AutoCorrect"])
    {
        check(budget, cancelled)?;
        if property.name() != name
            || property.native_type() != 4
            || property.flags() != 1
            || property.raw_value() != 0_i32.to_le_bytes()
        {
            return Err(DatabaseOptionsError::Preservation);
        }
    }
    check(budget, cancelled)?;
    Ok(())
}
