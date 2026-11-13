//! Source-bound missing-role, full-storage and fresh-EOF allocation witnesses.
use super::{self as source, allocator, catalog, schema};
use crate::{
    CreateDatabaseError, NativeImage, NativeSortOrder, QueryCreationError, TableSchema,
    format::{data_append::Page, row::reference},
    header::check,
    table::{
        mutation::{lval_ownership, ordinary_content::schema_dependencies, schema_source},
        table_append::{
            allocation::{Allocation, AllocationError},
            storage,
        },
    },
};
use tablecraft_core::limits::ResourceBudget;

const MAP_RECORD_BYTES: usize = 13;
const ADDED_MAP_ROWS: usize = 4;
const MAX_MAP_ROWS: usize = u8::MAX as usize + 1;

pub(in crate::table::mutation::query_creation) struct Source<'image> {
    pub catalog: TableSchema,
    pub parent: i32,
    pub catalog_page: u32,
    pub map_page: u32,
    pub map_count: usize,
    pub definition_page: u32,
    pub data_page: u32,
    pub order: NativeSortOrder,
    pub allocation: Allocation<'image>,
}

impl Source<'_> {
    pub(in crate::table::mutation::query_creation) fn map_references(
        &self,
    ) -> Result<[u32; 4], QueryCreationError> {
        let mut result = [0; 4];
        for (ordinal, item) in result.iter_mut().enumerate() {
            let slot =
                u8::try_from(self.map_count + ordinal).map_err(|_| QueryCreationError::Source)?;
            *item = reference(self.map_page, slot);
        }
        Ok(result)
    }
}

pub(in crate::table::mutation::query_creation) fn capture<'image>(
    image: &'image NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source<'image>, QueryCreationError> {
    check(budget, cancelled)?;
    image.require_retained_capture()?;
    let header = image.catalog().header();
    if header.format() != crate::FileFormat::Ace2016
        || header.subversion() != 0
        || header.encoded_pages()
        || !source::missing_queries(image, budget, cancelled)?
    {
        return Err(QueryCreationError::Source);
    }
    allocator::next(allocator::greatest(image, budget, cancelled)?)?;
    let parent = schema_source::find_role(image, "Tables", budget, cancelled)
        .map_err(|error| match error {
            schema_source::Error::Inspect(error) => QueryCreationError::Inspect(error),
            _ => QueryCreationError::Source,
        })?
        .native_id();
    let catalog_schema = schema::capture(image, "MSysObjects", parent, budget, cancelled)?;
    if catalog::qualify_missing(image, &catalog_schema, budget, cancelled)? != parent {
        return Err(QueryCreationError::Source);
    }
    schema_dependencies::require_empty(image, budget, cancelled).map_err(source::edit_error)?;
    let physical =
        storage::capture(image, &catalog_schema, budget, cancelled).map_err(source::edit_error)?;
    let catalog_page = source::direct_page(image, &catalog_schema, budget, cancelled)?;
    if physical.catalog.as_slice() != [catalog_page] {
        return Err(QueryCreationError::Source);
    }
    let lvals = lval_ownership::Inventory::capture_retained(image, budget, cancelled)
        .map_err(source::edit_error)?;
    lvals
        .require_no_long_value_data_overlap(&[catalog_page], budget, cancelled)
        .map_err(source::edit_error)?;
    let mut selected = None;
    for &number in &physical.maps {
        check(budget, cancelled)?;
        let page = super::super::append::page(image.as_bytes(), number)?;
        let frame = Page::capture_retained(page, 0, budget, cancelled)?;
        let count = frame
            .count
            .checked_add(ADDED_MAP_ROWS)
            .filter(|count| *count <= MAX_MAP_ROWS);
        if count.is_some_and(|count| {
            frame.start
                >= crate::format::row::slot_offset(count) + ADDED_MAP_ROWS * MAP_RECORD_BYTES
        }) {
            selected = Some((number, frame.count));
            break;
        }
    }
    let (map_page, map_count) = selected.ok_or(QueryCreationError::Source)?;
    let definition_page = header.page_count();
    let data_page = definition_page
        .checked_add(1)
        .ok_or(QueryCreationError::Source)?;
    let after_pages = data_page.checked_add(1).ok_or(QueryCreationError::Source)?;
    let allocation = physical
        .plan_allocation(after_pages, budget, cancelled)
        .map_err(allocation_error)?;
    let order = catalog_schema
        .columns()
        .iter()
        .find(|column| column.name() == "Name")
        .and_then(|column| column.text_sort_order())
        .ok_or(QueryCreationError::Source)?;
    Ok(Source {
        catalog: catalog_schema,
        parent,
        catalog_page,
        map_page,
        map_count,
        definition_page,
        data_page,
        order,
        allocation,
    })
}

pub(in crate::table::mutation::query_creation) fn allocation_error(
    error: AllocationError,
) -> QueryCreationError {
    match error {
        AllocationError::Inspect(error) => QueryCreationError::Inspect(error),
        AllocationError::Capacity | AllocationError::Source => QueryCreationError::Source,
    }
}

pub(in crate::table::mutation::query_creation) fn creation_error(
    error: CreateDatabaseError,
) -> QueryCreationError {
    match error {
        CreateDatabaseError::Inspect(error) => QueryCreationError::Inspect(error),
        _ => QueryCreationError::Source,
    }
}
