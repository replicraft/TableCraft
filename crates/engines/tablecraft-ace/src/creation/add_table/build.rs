//! Append new native roles and audit the original-byte preservation envelope.
use super::super::{
    FirstTableDefinition, definition,
    input::{Columns, Rows},
    profile,
    record::{self, DataPage},
    request,
};
use super::{
    AddTableError,
    append::Page,
    source::{self, Source},
};
use crate::{
    InspectError, NativeImage,
    format::{layout::RECORD_COUNT_OFFSET, page::PAGE_BYTES, row::reference},
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const COPY_BYTES: usize = 16 * 1024;
const CATALOG_DEFINITION: usize = 2;
const MAP_RECORD_BYTES: usize = 13;
const NEW_PAGE_COUNT: usize = 13;
const HEADER_BYTES: usize = 14;
const SLOT_BYTES: usize = size_of::<u16>();

pub(super) fn build(
    before: &NativeImage,
    definition: &FirstTableDefinition,
    source: &Source,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, AddTableError> {
    let columns = definition.borrowed_columns(budget, cancelled)?;
    let request = request::validate(
        definition.table(),
        Columns::Values(&columns),
        Rows::Values(&[]),
        budget,
        cancelled,
    )?;
    let catalog_length =
        record::catalog_length_with_properties(request.catalog_name, request.properties.as_ref())?;
    if source.catalog.start
        < catalog_length + HEADER_BYTES + (source.catalog.count + 1) * SLOT_BYTES
        || source.maps.start
            < 3 * MAP_RECORD_BYTES + HEADER_BYTES + (source.maps.count + 3) * SLOT_BYTES
    {
        return Err(AddTableError::Capacity);
    }

    let length = NEW_PAGE_COUNT * PAGE_BYTES;
    budget
        .charge_allocation(length as u64, 1)
        .map_err(InspectError::from)?;
    let mut after = Vec::new();
    after
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    for chunk in before.as_bytes().chunks(COPY_BYTES) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(InspectError::from)?;
        after.extend_from_slice(chunk);
    }
    budget
        .charge(Resource::Steps, (length - after.len()) as u64)
        .map_err(InspectError::from)?;
    after.resize(length, 0);

    let primary = request
        .primary
        .as_ref()
        .ok_or(AddTableError::Preservation)?;
    definition::write_role_with_primary(
        page_mut(&mut after, source::NEW_DEFINITION),
        request.fields(),
        0,
        definition::Role {
            user: true,
            catalog: false,
            owned: reference(source::MAP_PAGE as u32, source::OWNED_SLOT as u8),
            candidate: reference(source::MAP_PAGE as u32, source::CANDIDATE_SLOT as u8),
        },
        Some(definition::PrimaryRole {
            prepared: primary,
            root: source::NEW_ROOT,
            map: reference(source::MAP_PAGE as u32, source::INDEX_SLOT as u8),
        }),
        budget,
        cancelled,
    )?;
    DataPage::new(
        page_mut(&mut after, source::NEW_DATA),
        source::NEW_DEFINITION,
    )?
    .finish()?;
    primary.write_leaf(
        page_mut(&mut after, source::NEW_ROOT),
        source::NEW_DEFINITION,
        source::NEW_DATA,
        budget,
        cancelled,
    )?;

    let catalog = page_mut(&mut after, source::CATALOG_PAGE);
    let mut catalog_layout = Page::capture(catalog, CATALOG_DEFINITION as u32, budget, cancelled)?;
    Page::charge_append(catalog_length, budget, cancelled)?;
    let record_range = catalog_layout.append(catalog, catalog_length)?;
    record::catalog_with_properties(
        &mut catalog[record_range],
        &profile::CatalogObject {
            id: source::NEW_DEFINITION as u32,
            parent: profile::TABLES_ID,
            name: request.catalog_name,
            system: false,
        },
        request.properties.as_ref(),
        budget,
        cancelled,
    )?;
    definition::put32(
        page_mut(&mut after, CATALOG_DEFINITION),
        RECORD_COUNT_OFFSET,
        (source.catalog.count + 1) as u32,
    )?;

    let maps = page_mut(&mut after, source::MAP_PAGE);
    let mut map_layout = Page::capture(maps, 0, budget, cancelled)?;
    for member in [source::NEW_DATA, source::NEW_DATA, source::NEW_ROOT] {
        Page::charge_append(MAP_RECORD_BYTES, budget, cancelled)?;
        let range = map_layout.append(maps, MAP_RECORD_BYTES)?;
        record::inline_map(&mut maps[range], Some(member))?;
    }
    check(budget, cancelled)?;
    Ok(after)
}

fn page_mut(bytes: &mut [u8], number: usize) -> &mut [u8] {
    &mut bytes[number * PAGE_BYTES..(number + 1) * PAGE_BYTES]
}
