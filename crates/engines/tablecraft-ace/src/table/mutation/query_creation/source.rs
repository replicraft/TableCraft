//! New-object source authority over complete native catalog/query storage.
mod allocator;
mod catalog;
mod fragments;
pub(super) mod initialization;
mod schema;

use crate::{
    EditError, FileFormat, NativeImage, QueryCreationDefinition, QueryCreationError, TableSchema,
    compare_general_text,
    format::{
        allocation,
        bytes::u32_at,
        page::{DATA_SIGNATURE, OWNER_OFFSET},
        reader::{Reader, row_count, row_range},
        row::OFFSET_MASK,
    },
    header::check,
    table::{
        mutation::{lval_ownership, ordinary_content::schema_dependencies, schema_source},
        record_layout,
        table_append::storage,
    },
};
use std::cmp::Ordering;
use tablecraft_core::limits::ResourceBudget;

const LOCAL_TABLE_TYPE: i16 = 1;

pub(super) struct CheckedStorage {
    pub catalog: TableSchema,
    pub queries: TableSchema,
    pub parent: i32,
    pub catalog_page: u32,
    pub query_page: u32,
    greatest_negative: i32,
}

pub(super) struct Source {
    pub storage: CheckedStorage,
    pub native_id: i32,
}

pub(super) fn capture(
    image: &NativeImage,
    definition: &QueryCreationDefinition,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, QueryCreationError> {
    let storage = capture_storage(image, budget, cancelled)?;
    let order = storage
        .catalog
        .columns()
        .iter()
        .find(|column| column.name() == "Name")
        .and_then(|column| column.text_sort_order())
        .ok_or(QueryCreationError::Source)?;
    for (object, origin) in image
        .catalog()
        .catalog()
        .objects()
        .iter()
        .zip(image.catalog().origins())
    {
        check(budget, cancelled)?;
        if origin.parent_id() == storage.parent
            && compare_general_text(
                order,
                definition.name(),
                object.name().as_str(),
                budget,
                cancelled,
            )? == Ordering::Equal
        {
            return Err(QueryCreationError::Definition);
        }
    }
    let native_id = allocator::next(storage.greatest_negative)?;
    Ok(Source { storage, native_id })
}

/// Fresh AFTER observation preserves every source qualification. Only the
/// request's destination-absence and next-ID selection belong to BEFORE.
pub(super) fn capture_after(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CheckedStorage, QueryCreationError> {
    capture_storage(image, budget, cancelled)
}

fn capture_storage(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CheckedStorage, QueryCreationError> {
    check(budget, cancelled)?;
    image.require_retained_capture()?;
    let header = image.catalog().header();
    if header.format() != FileFormat::Ace2016 || header.subversion() != 0 || header.encoded_pages()
    {
        return Err(QueryCreationError::Source);
    }
    let greatest_negative = allocator::greatest(image, budget, cancelled)?;
    let parent = schema_source::find_role(image, "Tables", budget, cancelled)
        .map_err(|error| match error {
            schema_source::Error::Inspect(error) => QueryCreationError::Inspect(error),
            _ => QueryCreationError::Source,
        })?
        .native_id();
    let catalog = schema::capture(image, "MSysObjects", parent, budget, cancelled)?;
    let queries = schema::capture(image, "MSysQueries", parent, budget, cancelled)?;
    if catalog::qualify(image, &catalog, budget, cancelled)? != parent {
        return Err(QueryCreationError::Source);
    }
    schema_dependencies::require_empty(image, budget, cancelled).map_err(edit_error)?;
    fragments::qualify(image, &queries, budget, cancelled)?;

    // This actual shared physical census includes every live and retired TDEF,
    // represented map, DATA/INDEX and LVAL role, not merely selected pages.
    // Its source policy is unchanged; this operation owns the catalog predicate.
    let physical = storage::capture(image, &catalog, budget, cancelled).map_err(edit_error)?;
    let catalog_page = direct_page(image, &catalog, budget, cancelled)?;
    let query_page = direct_page(image, &queries, budget, cancelled)?;
    if physical.catalog.as_slice() != [catalog_page] || catalog_page == query_page {
        return Err(QueryCreationError::Source);
    }
    let lvals = lval_ownership::Inventory::capture_retained(image, budget, cancelled)
        .map_err(edit_error)?;
    lvals
        .require_no_long_value_data_overlap(&[catalog_page, query_page], budget, cancelled)
        .map_err(edit_error)?;
    Ok(CheckedStorage {
        catalog,
        queries,
        parent,
        catalog_page,
        query_page,
        greatest_negative,
    })
}

pub(super) fn direct_page(
    image: &NativeImage,
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<u32, QueryCreationError> {
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let owned = allocation::pages(&mut reader, schema.layout.usage)?;
    let [number] = owned.as_slice() else {
        return Err(QueryCreationError::Source);
    };
    let page = reader.page(*number)?;
    if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
        || u32_at(&page, OWNER_OFFSET)? != schema.definition_page()
        || u32::try_from(row_count(&page)?).ok() != Some(schema.declared_rows())
    {
        return Err(QueryCreationError::Source);
    }
    for slot in 0..row_count(&page)? {
        reader.tick()?;
        let (flags, start, end) = row_range(&page, slot)?;
        if flags & !OFFSET_MASK != 0 {
            return Err(QueryCreationError::Source);
        }
        let framing = record_layout::parse(&page[start..end], schema, reader.budget)?;
        if framing.count != schema.layout.max_columns
            || framing.offsets.len() != schema.layout.variable_slots + 1
        {
            return Err(QueryCreationError::Source);
        }
    }
    Ok(*number)
}

pub(super) fn edit_error(error: EditError) -> QueryCreationError {
    match error {
        EditError::Inspect(error) => QueryCreationError::Inspect(error),
        _ => QueryCreationError::Source,
    }
}

/// Absence is selected only by complete catalog membership. A conflicting or
/// malformed named role is terminal; construction never retries schema failure.
pub(super) fn missing_queries(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, QueryCreationError> {
    let mut found = None;
    for (object, origin) in image
        .catalog()
        .catalog()
        .objects()
        .iter()
        .zip(image.catalog().origins())
    {
        check(budget, cancelled)?;
        budget.charge(
            tablecraft_core::limits::Resource::Steps,
            object.name().as_str().len() as u64,
        )?;
        if object.name().as_str().eq_ignore_ascii_case("MSysQueries") {
            if found.replace(origin).is_some()
                || !origin.is_system()
                || origin.type_code() != LOCAL_TABLE_TYPE
                || origin.table_storage() != Some(crate::NativeTableStorage::Local)
            {
                return Err(QueryCreationError::Source);
            }
        }
    }
    Ok(found.is_none())
}
