//! Independent new-role decoding and exact preservation of every original span.
use super::source::Source;
use crate::{
    FirstTableDefinition, InspectError, NativeImage, NativeImageSource, check_table,
    creation::{
        add_table::{AddTableError, audit},
        catalog_page::Page,
    },
    format::{bytes::u32_at, layout::RECORD_COUNT_OFFSET, page::PAGE_BYTES, row::reference},
    header::check,
    table::table_append::Inventory,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const SLOT_DIRECTORY: usize = 14;
const MAP_BYTES: usize = 13;
const MAP_BITMAP_OFFSET: usize = 5;
const MAP_WINDOW: usize = 64;

fn page(bytes: &[u8], number: u32) -> Result<&[u8], AddTableError> {
    let start = number as usize * PAGE_BYTES;
    bytes
        .get(start..start + PAGE_BYTES)
        .ok_or(AddTableError::Preservation)
}

fn append_scope(offset: usize, before: &Page, after: &Page) -> bool {
    (2..4).contains(&offset)
        || (12..14).contains(&offset)
        || (SLOT_DIRECTORY + before.count * 2..SLOT_DIRECTORY + after.count * 2).contains(&offset)
        || (after.start..before.start).contains(&offset)
}

pub(super) fn verify(
    before: &NativeImage,
    after: &NativeImageSource,
    declaration: &FirstTableDefinition,
    source: &Source<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), AddTableError> {
    check(budget, cancelled)?;
    source
        .inventory
        .require_source(before)
        .map_err(super::source::inventory_error)?;
    let bytes = after.as_bytes();
    if bytes.len() != source.after_bytes {
        return Err(AddTableError::Preservation);
    }
    let catalog = Page::capture_retained(
        page(bytes, source.catalog_page)?,
        source.inventory.catalog_definition(),
        budget,
        cancelled,
    )?;
    let maps = Page::capture_retained(page(bytes, source.map_page)?, 0, budget, cancelled)?;
    if catalog.count != source.catalog.count + 1
        || maps.count != source.maps.count + source.added.pages()
    {
        return Err(AddTableError::Preservation);
    }
    let count_offset =
        source.inventory.catalog_definition() as usize * PAGE_BYTES + RECORD_COUNT_OFFSET;
    if u32_at(bytes, count_offset)?
        != source
            .inventory
            .catalog_live_rows()
            .checked_add(1)
            .ok_or(AddTableError::Preservation)?
    {
        return Err(AddTableError::Preservation);
    }
    for (page_index, (old, new)) in before
        .as_bytes()
        .chunks(PAGE_BYTES)
        .zip(bytes.chunks(PAGE_BYTES))
        .enumerate()
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, old.len() as u64)
            .map_err(InspectError::from)?;
        for (offset, (old, new)) in old.iter().zip(new).enumerate() {
            let at = page_index * PAGE_BYTES + offset;
            let permitted = (count_offset..count_offset + 4).contains(&at)
                || (page_index == source.catalog_page as usize
                    && append_scope(offset, &source.catalog, &catalog))
                || (page_index == source.map_page as usize
                    && append_scope(offset, &source.maps, &maps));
            if old != new && !permitted {
                return Err(AddTableError::Preservation);
            }
        }
    }
    for (range, member) in maps.records[source.maps.count..]
        .iter()
        .zip(source.added.members())
    {
        check(budget, cancelled)?;
        let mut expected = [0; MAP_BYTES];
        let base = member - member % MAP_WINDOW;
        expected[1..5].copy_from_slice(&(base as u32).to_le_bytes());
        let bit = member - base;
        expected[MAP_BITMAP_OFFSET + bit / 8] = 1 << (bit % 8);
        if page(bytes, source.map_page)?.get(range.start..range.end) != Some(expected.as_slice()) {
            return Err(AddTableError::Preservation);
        }
    }
    audit::empty_pages::verify(bytes, source.added, budget, cancelled)?;
    let columns = declaration.borrowed_columns(budget, cancelled)?;
    audit::definition::verify_at(
        bytes,
        source.added,
        declaration,
        &columns,
        source.map_page,
        budget,
        cancelled,
    )?;

    // NO_CLONE: The plan retains the immutable AFTER allocation while a fresh native
    // inspection owns the same complete-allocation witness. Sharing avoids copying
    // a potentially large image; a borrowed slice cannot supply that capture owner.
    let audit_owner = after.clone();
    let image = NativeImage::capture_shared_for_table_append(
        audit_owner,
        before.catalog().catalog().scope(),
        budget,
        cancelled,
    )?;
    let inventory =
        Inventory::capture(&image, budget, cancelled).map_err(super::source::inventory_error)?;
    if inventory.catalog_live_rows() != source.inventory.catalog_live_rows() + 1 {
        return Err(AddTableError::Preservation);
    }
    let checked = check_table(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        declaration.table(),
        budget,
        cancelled,
    )?;
    let schema = checked.schema();
    if schema.definition_page() != source.added.definition as u32
        || schema.declared_rows() != 0
        || checked.rows() != 0
        || schema.columns().len() != declaration.column_count()
        || schema.physical_layout().usage != reference(source.map_page, source.added.owned as u8)
        || checked.indexes().len() != usize::from(source.added.primary.is_some())
    {
        return Err(AddTableError::Preservation);
    }
    let properties = audit::properties::verify(&image, schema, &columns, budget, cancelled)?;
    let range = &catalog.records[source.catalog.count];
    audit::catalog::verify_at(
        &page(bytes, source.catalog_page)?[range.start..range.end],
        declaration.table(),
        source.added.definition as u32,
        &properties,
        source.inventory.tables_parent() as u32,
        budget,
        cancelled,
    )?;
    let origins = image.catalog().origins();
    if origins.len() != before.catalog().origins().len() + 1 {
        return Err(AddTableError::Preservation);
    }
    for original in before.catalog().origins() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, origins.len() as u64)
            .map_err(InspectError::from)?;
        if !origins.iter().any(|observed| {
            observed.native_id() == original.native_id()
                && observed.parent_id() == original.parent_id()
                && observed.type_code() == original.type_code()
                && observed.flags() == original.flags()
                && observed.page() == original.page()
                && observed.slot() == original.slot()
        }) {
            return Err(AddTableError::Preservation);
        }
    }
    let added = origins
        .iter()
        .find(|origin| origin.native_id() == source.added.definition as i32)
        .ok_or(AddTableError::Preservation)?;
    if added.is_system()
        || added.type_code() != 1
        || added.flags() != 0
        || added.parent_id() != source.inventory.tables_parent()
        || added.page() != source.catalog_page
        || added.slot() as usize != source.added.catalog
    {
        return Err(AddTableError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}
