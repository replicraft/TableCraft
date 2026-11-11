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
    source::{self, Source},
};
use crate::creation::catalog_page::Page;
use crate::{
    InspectError, NativeImage,
    format::{layout::RECORD_COUNT_OFFSET, page::PAGE_BYTES, row::reference},
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const COPY_BYTES: usize = 16 * 1024;
const CATALOG_DEFINITION: usize = 2;
const MAP_RECORD_BYTES: usize = 13;
const HEADER_BYTES: usize = 14;
const SLOT_BYTES: usize = size_of::<u16>();

pub(super) fn build(
    before: &NativeImage,
    definition: &FirstTableDefinition,
    source: &Source,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, AddTableError> {
    let role = source.geometry.added;
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
            < role.pages() * MAP_RECORD_BYTES
                + HEADER_BYTES
                + (source.maps.count + role.pages()) * SLOT_BYTES
    {
        return Err(AddTableError::Capacity);
    }

    let length = source.geometry.after_pages * PAGE_BYTES;
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

    let primary = request.primary.as_ref();
    if primary.is_some() != role.primary.is_some() {
        return Err(AddTableError::Preservation);
    }
    definition::write_role_with_primary(
        page_mut(&mut after, role.definition),
        request.fields(),
        0,
        definition::Role {
            text_sort_order: Some(profile::GENERAL_SORT),
            user: true,
            catalog: false,
            owned: reference(source::MAP_PAGE as u32, role.owned as u8),
            candidate: reference(source::MAP_PAGE as u32, role.candidate as u8),
        },
        primary
            .zip(role.primary)
            .map(|(prepared, primary)| definition::PrimaryRole {
                prepared,
                root: primary.root,
                map: reference(source::MAP_PAGE as u32, primary.map as u8),
            }),
        budget,
        cancelled,
    )?;
    DataPage::new(page_mut(&mut after, role.data), role.definition)?.finish()?;
    if let Some((prepared, primary)) = primary.zip(role.primary) {
        prepared.write_leaf(
            page_mut(&mut after, primary.root),
            role.definition,
            role.data,
            budget,
            cancelled,
        )?;
    }

    let catalog = page_mut(&mut after, source::CATALOG_PAGE);
    let mut catalog_layout = Page::capture(catalog, CATALOG_DEFINITION as u32, budget, cancelled)?;
    Page::charge_append(catalog_length, budget, cancelled)?;
    let record_range = catalog_layout.append(catalog, catalog_length)?;
    record::catalog_with_properties(
        &mut catalog[record_range],
        &profile::CatalogObject {
            id: role.definition as u32,
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
    for member in role.members() {
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
