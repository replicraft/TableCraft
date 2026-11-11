//! Construct only fresh EOF roles and actual physical catalog/map append spans.
use super::source::Source;
use crate::{
    FirstTableDefinition, InspectError, NativeImage,
    creation::{
        add_table::AddTableError,
        catalog_page::Page,
        definition,
        input::{Columns, Rows},
        profile,
        record::{self, DataPage},
        request,
    },
    format::{layout::RECORD_COUNT_OFFSET, page::PAGE_BYTES, row::reference},
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const COPY_BYTES: usize = 16 * 1024;
const MAP_BYTES: usize = 13;
const MAP_BITMAP_BYTES: usize = 8;
const MAP_BITMAP_OFFSET: usize = 5;
const MAP_WINDOW_PAGES: usize = MAP_BITMAP_BYTES * u8::BITS as usize;

fn page_mut(bytes: &mut [u8], page: usize) -> Result<&mut [u8], AddTableError> {
    let start = page
        .checked_mul(PAGE_BYTES)
        .ok_or(AddTableError::Preservation)?;
    bytes
        .get_mut(start..start + PAGE_BYTES)
        .ok_or(AddTableError::Preservation)
}

pub(super) fn build(
    before: &NativeImage,
    declaration: &FirstTableDefinition,
    source: &Source<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, AddTableError> {
    check(budget, cancelled)?;
    source
        .inventory
        .require_source(before)
        .map_err(super::source::inventory_error)?;
    let columns = declaration.borrowed_columns(budget, cancelled)?;
    let request = request::validate(
        declaration.table(),
        Columns::Values(&columns),
        Rows::Values(&[]),
        budget,
        cancelled,
    )?;
    let catalog_length =
        record::catalog_length_with_properties(request.catalog_name, request.properties.as_ref())?;
    budget
        .charge_allocation(source.after_bytes as u64, 1)
        .map_err(InspectError::from)?;
    let mut after = Vec::new();
    after
        .try_reserve_exact(source.after_bytes)
        .map_err(|_| InspectError::Allocation)?;
    for chunk in before.as_bytes().chunks(COPY_BYTES) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(InspectError::from)?;
        after.extend_from_slice(chunk);
    }
    budget
        .charge(Resource::Steps, (source.after_bytes - after.len()) as u64)
        .map_err(InspectError::from)?;
    after.resize(source.after_bytes, 0);

    source
        .allocation
        .apply(&mut after, budget, cancelled)
        .map_err(super::source::allocation_preservation_error)?;

    let role = source.added;
    let primary = request.primary.as_ref();
    if primary.is_some() != role.primary.is_some() {
        return Err(AddTableError::Preservation);
    }
    definition::write_role_with_primary(
        page_mut(&mut after, role.definition)?,
        request.fields(),
        0,
        definition::Role {
            text_sort_order: Some(profile::GENERAL_SORT),
            user: true,
            catalog: false,
            owned: reference(source.map_page, role.owned as u8),
            candidate: reference(source.map_page, role.candidate as u8),
        },
        primary
            .zip(role.primary)
            .map(|(prepared, primary)| definition::PrimaryRole {
                prepared,
                root: primary.root,
                map: reference(source.map_page, primary.map as u8),
            }),
        budget,
        cancelled,
    )?;
    DataPage::new(page_mut(&mut after, role.data)?, role.definition)?.finish()?;
    if let Some((prepared, primary)) = primary.zip(role.primary) {
        prepared.write_leaf(
            page_mut(&mut after, primary.root)?,
            role.definition,
            role.data,
            budget,
            cancelled,
        )?;
    }

    let catalog = page_mut(&mut after, source.catalog_page as usize)?;
    let mut physical = Page::capture_retained(
        catalog,
        source.inventory.catalog_definition(),
        budget,
        cancelled,
    )?;
    Page::charge_append(catalog_length, budget, cancelled)?;
    let range = physical.append(catalog, catalog_length)?;
    record::catalog_with_properties(
        &mut catalog[range],
        &profile::CatalogObject {
            id: role.definition as u32,
            parent: u32::try_from(source.inventory.tables_parent())
                .map_err(|_| AddTableError::Source)?,
            name: request.catalog_name,
            system: false,
        },
        request.properties.as_ref(),
        budget,
        cancelled,
    )?;
    let live = source
        .inventory
        .catalog_live_rows()
        .checked_add(1)
        .ok_or(AddTableError::Capacity)?;
    definition::put32(
        page_mut(&mut after, source.inventory.catalog_definition() as usize)?,
        RECORD_COUNT_OFFSET,
        live,
    )?;

    let maps = page_mut(&mut after, source.map_page as usize)?;
    let mut physical = Page::capture_retained(maps, 0, budget, cancelled)?;
    for member in role.members() {
        Page::charge_append(MAP_BYTES, budget, cancelled)?;
        let range = physical.append(maps, MAP_BYTES)?;
        let record = &mut maps[range];
        let base = member / MAP_WINDOW_PAGES * MAP_WINDOW_PAGES;
        let base = u32::try_from(base).map_err(|_| AddTableError::Capacity)?;
        record[1..5].copy_from_slice(&base.to_le_bytes());
        let bit = member - base as usize;
        record[MAP_BITMAP_OFFSET + bit / u8::BITS as usize] = 1 << (bit % u8::BITS as usize);
    }
    check(budget, cancelled)?;
    Ok(after)
}
