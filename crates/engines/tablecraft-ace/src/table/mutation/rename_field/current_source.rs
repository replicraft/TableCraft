//! Current native ownership and dependencies, independent of construction history.
use super::{FieldRenameError as Error, source::Qualified};
use crate::{
    FileFormat, NativeImage, TableProperties,
    format::{
        bytes::{u16_at, u32_at},
        layout::CALCULATED_EXT_MASK,
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{row_count, row_range},
        row::{OFFSET_MASK, reference_page, reference_slot, slot_offset},
    },
    header::check,
    inspect_table_schema,
    table::mutation::{
        catalog_property::{column, ledger},
        insert_defaults::literal,
        lval_ownership::Inventory,
        normal_ordinary,
        saved_field::Observed,
        schema_source::{self, Error as SchemaSourceError},
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const SOURCE_PAGES: usize = 11;
const FREE_OFFSET: usize = DATA_SIGNATURE.len();
const LONG_VALUE_OWNER: &[u8; size_of::<u32>()] = b"LVAL";

pub(super) fn capture(
    image: NativeImage,
    table: &str,
    column: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Qualified, Error> {
    let observed = observe(&image, table, column, budget, cancelled)?;
    Ok(Qualified::from_current(image, observed))
}

pub(super) fn observe(
    image: &NativeImage,
    table: &str,
    field: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Observed, Error> {
    check(budget, cancelled)?;
    image.require_standard_capture()?;
    let header = image.catalog().header();
    if header.format() != FileFormat::Ace2016
        || header.subversion() != 0
        || header.encoded_pages()
        || image.as_bytes().len() != SOURCE_PAGES * PAGE_BYTES
    {
        return Err(Error::Source);
    }

    // Actual row/index/program/constraint evidence is independent of empty
    // creator seeds and retained counters, deleted slots or previous spellings.
    let source = normal_ordinary::capture_insert(image, table, budget, cancelled)?;
    if !source.dependencies.is_empty()
        || source
            .table
            .schema()
            .columns()
            .iter()
            .any(|column| column.extended_flags() & CALCULATED_EXT_MASK != 0)
    {
        return Err(Error::Source);
    }
    let graph = schema_source::observe(image, table, budget, cancelled).map_err(graph_error)?;
    let observed = Observed::capture(image, table, field, budget, cancelled)?;
    if graph.user.object_id() != observed.schema.object_id()
        || source.table.schema().object_id() != observed.schema.object_id()
    {
        return Err(Error::Source);
    }
    dependencies(&observed.properties, &observed, budget, cancelled)?;

    let catalog = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        "MSysObjects",
        budget,
        cancelled,
    )?;
    let property_column = column(&catalog)?;
    let ledger = ledger::capture(image.as_bytes(), image.catalog(), budget, cancelled)?;
    let (database_reference, database_bytes) = ledger
        .single_reference(graph.database.object_id(), budget, cancelled)?
        .ok_or(Error::Source)?;
    if database_bytes != graph.properties.raw_bytes().len()
        || !ledger.two_singles(
            [
                (
                    observed.schema.object_id(),
                    observed.property_reference,
                    observed.property_body.len(),
                ),
                (
                    graph.database.object_id(),
                    database_reference,
                    database_bytes,
                ),
            ],
            budget,
            cancelled,
        )?
    {
        return Err(Error::Source);
    }
    let property_page = reference_page(observed.property_reference);
    if reference_page(database_reference) != property_page
        || usize::from(reference_slot(database_reference))
            != usize::from(reference_slot(observed.property_reference)) + 1
    {
        return Err(Error::Source);
    }
    let inventory = Inventory::capture(image, budget, cancelled)?;
    inventory.require_exclusive(
        &catalog,
        property_column,
        &[property_page],
        budget,
        cancelled,
    )?;
    property_geometry(image, property_page, budget, cancelled)?;

    for origin in image.catalog().origins() {
        check(budget, cancelled)?;
        let expected = if origin.object_id() == observed.schema.object_id() {
            Some((observed.property_reference, observed.property_body.len()))
        } else if origin.object_id() == graph.database.object_id() {
            Some((database_reference, database_bytes))
        } else {
            None
        };
        schema_source::require_external_payload(
            image, &catalog, origin, expected, budget, cancelled,
        )
        .map_err(graph_error)?;
    }
    Ok(observed)
}

fn dependencies(
    properties: &TableProperties,
    observed: &Observed,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    for group in properties.groups() {
        for property in group.properties() {
            check(budget, cancelled)?;
            budget.charge(Resource::Steps, property.name().len() as u64)?;
            let name = property.name();
            if name.eq_ignore_ascii_case("NameMap") || name.eq_ignore_ascii_case("Expression") {
                return Err(Error::Source);
            }
            if name.eq_ignore_ascii_case("ValidationRule") && !property.text()?.is_empty() {
                return Err(Error::Source);
            }
            if name.eq_ignore_ascii_case("DefaultValue") {
                let mut selected = None;
                for column in observed.schema.columns() {
                    check(budget, cancelled)?;
                    budget.charge(
                        Resource::Steps,
                        column.name().len().max(group.name().len()) as u64,
                    )?;
                    if column.name().eq_ignore_ascii_case(group.name())
                        && selected.replace(column).is_some()
                    {
                        return Err(Error::Source);
                    }
                }
                let column = selected.ok_or(Error::Source)?;
                // Parse only the existing bounded literal grammar. No default,
                // query, clock or database program is evaluated for inspection.
                literal::resolve(column, property.text()?, budget, cancelled)?;
            }
        }
    }
    Ok(())
}

fn property_geometry(
    image: &NativeImage,
    page: u32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    let start = page as usize * PAGE_BYTES;
    let raw = image
        .as_bytes()
        .get(start..start + PAGE_BYTES)
        .ok_or(Error::Source)?;
    if raw[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
        || u32_at(raw, OWNER_OFFSET)? != u32::from_le_bytes(*LONG_VALUE_OWNER)
    {
        return Err(Error::Source);
    }
    let count = row_count(raw)?;
    let mut end = PAGE_BYTES;
    for slot in 0..count {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, size_of::<u16>() as u64)?;
        let (word, current, previous) = row_range(raw, slot)?;
        if word & !OFFSET_MASK != 0 || current >= previous || previous != end {
            return Err(Error::Source);
        }
        end = current;
    }
    let free = end.checked_sub(slot_offset(count)).ok_or(Error::Source)?;
    if usize::from(u16_at(raw, FREE_OFFSET)?) != free {
        return Err(Error::Source);
    }
    Ok(())
}

fn graph_error(error: SchemaSourceError) -> Error {
    match error {
        SchemaSourceError::Inspect(error) => Error::Inspect(error),
        SchemaSourceError::Source | SchemaSourceError::Dependency => Error::Source,
    }
}
