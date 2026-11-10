//! Populated current table DROP composes genuine row deletion and catalog removal.
pub(super) mod proof;

#[cfg(test)]
mod tests;

use super::{
    NativeTableDropIdentity, TableDropError,
    source::{Qualified, SourceProfile},
};
use crate::{
    EditError, FileFormat, InspectError, NativeImage, NativeTableStorage,
    format::{bytes::u32_at, definition::LENGTH_OFFSET, page::PAGE_BYTES},
    header::check,
    inspect_table_schema,
    table::mutation::{
        current_schema, normal_delete, ordinary_content::schema_dependencies, schema_source,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::ObjectKind;

const SOURCE_PAGES: usize = 11;
const USER_DEFINITION: u32 = 7;
const CATALOG_FIELDS: usize = 17;
const LAST_ISSUED_OFFSET: usize = 20;

pub(super) fn selected(
    image: &NativeImage,
    requested: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, TableDropError> {
    check(budget, cancelled)?;
    let header = image.catalog().header();
    if header.format() != FileFormat::Ace2016
        || header.subversion() != 0
        || header.encoded_pages()
        || image.as_bytes().len() != SOURCE_PAGES * PAGE_BYTES
    {
        return Ok(false);
    }
    let mut matched = false;
    for (object, origin) in image
        .catalog()
        .catalog()
        .objects()
        .iter()
        .zip(image.catalog().origins())
    {
        check(budget, cancelled)?;
        budget.charge(
            Resource::Steps,
            object.name().as_str().len().max(requested.len()) as u64,
        )?;
        if object.kind() == ObjectKind::Table
            && !origin.is_system()
            && origin.table_storage() == Some(NativeTableStorage::Local)
            && object.name().as_str().eq_ignore_ascii_case(requested)
        {
            matched = true;
        }
    }
    if !matched {
        return Ok(false);
    }
    let catalog = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        "MSysObjects",
        budget,
        cancelled,
    )?;
    if catalog.columns().len() != CATALOG_FIELDS {
        return Ok(false);
    }
    let schema = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        requested,
        budget,
        cancelled,
    )?;
    if schema.definition_page() != USER_DEFINITION {
        return Ok(false);
    }
    let start = USER_DEFINITION as usize * PAGE_BYTES;
    let page = &image.as_bytes()[start..start + PAGE_BYTES];
    let length = u32_at(page, LENGTH_OFFSET)? as usize;
    let slack = page.get(length..).ok_or(InspectError::Truncated)?;
    budget.charge(Resource::Steps, slack.len() as u64)?;
    let retained_definition = slack.iter().any(|byte| *byte != 0);
    Ok(schema.declared_rows() != 0
        || schema.physical_layout().max_columns > schema.columns().len()
        || u32_at(page, LAST_ISSUED_OFFSET)? != 0
        || retained_definition)
}

pub(super) fn qualify(
    image: &NativeImage,
    requested: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Qualified, TableDropError> {
    if !selected(image, requested, budget, cancelled)? {
        return Err(TableDropError::Source);
    }
    let observed = current_schema::observe(image, requested, budget, cancelled).map_err(
        |error| match error {
            current_schema::Error::Inspect(error) => TableDropError::Inspect(error),
            current_schema::Error::Native(error) => edit_error(error),
            current_schema::Error::Source => TableDropError::Source,
        },
    )?;
    let graph =
        schema_source::observe(image, requested, budget, cancelled).map_err(
            |error| match error {
                schema_source::Error::Inspect(error) => TableDropError::Inspect(error),
                schema_source::Error::Source => TableDropError::Source,
                schema_source::Error::Dependency => TableDropError::Dependency,
            },
        )?;
    schema_dependencies::require_empty(image, budget, cancelled).map_err(edit_error)?;
    drop(
        normal_delete::current::capture_table_drop(image, requested, budget, cancelled)
            .map_err(edit_error)?,
    );
    let name = observed.schema.name();
    if !crate::identifier::valid_name(name, budget, cancelled)? {
        return Err(TableDropError::Source);
    }
    budget.charge_allocation(name.len() as u64, 1)?;
    let mut table = String::new();
    table
        .try_reserve_exact(name.len())
        .map_err(|_| InspectError::Allocation)?;
    budget.charge(Resource::Steps, name.len() as u64)?;
    table.push_str(name);
    check(budget, cancelled)?;
    Ok(Qualified {
        table,
        identity: NativeTableDropIdentity::from_origin(
            graph.user,
            observed.schema.definition_page(),
        ),
        profile: SourceProfile::Current,
        declared_rows: observed.schema.declared_rows(),
        catalog_definition: observed.catalog.definition_page(),
        catalog_rows: observed.catalog.declared_rows(),
    })
}

pub(super) fn row_effects(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, TableDropError> {
    normal_delete::current::compose_table_drop(image, table, budget, cancelled).map_err(edit_error)
}

fn edit_error(error: EditError) -> TableDropError {
    match error {
        EditError::Inspect(error) => TableDropError::Inspect(error),
        EditError::Preservation => TableDropError::Preservation,
        EditError::Unsupported(_) => TableDropError::Dependency,
        _ => TableDropError::Source,
    }
}
