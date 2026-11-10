//! Current table ownership and safe schema dependencies, without creator replay.
use crate::{
    DatabaseProperties, EditError, FileFormat, InspectError, NativeImage, TableProperties,
    TableSchema,
    format::{
        bytes::{u16_at, u32_at},
        layout::CALCULATED_EXT_MASK,
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{Reader, row_count, row_range},
        row::{OFFSET_MASK, reference_page, slot_offset},
    },
    header::check,
    inspect_table_properties, inspect_table_schema,
    table::long_value::trace::Kind,
    table::mutation::{
        catalog_property::{column, descriptor, ledger},
        insert_defaults::literal,
        lval_ownership::Inventory,
        normal_ordinary,
        schema_source::{self, Error as SchemaSourceError},
    },
};
use tablecraft_core::limits::{LimitError, Resource, ResourceBudget};
use thiserror::Error;

const SOURCE_PAGES: usize = 11;
const USER_DEFINITION: u32 = 7;
const FREE_OFFSET: usize = DATA_SIGNATURE.len();
const LONG_VALUE_OWNER: &[u8; size_of::<u32>()] = b"LVAL";

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error(transparent)]
    Inspect(#[from] InspectError),
    #[error(transparent)]
    Native(#[from] EditError),
    #[error("native table is outside the current schema source profile")]
    Source,
}

impl From<LimitError> for Error {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

/// Current table facts are data; the owning Source alone retains the original.
pub(crate) struct Table {
    pub schema: TableSchema,
    pub properties: TableProperties,
    pub catalog: TableSchema,
    pub user_storage: UserStorage,
    pub database: DatabaseProperties,
    pub database_reference: u32,
}

/// Actual user-property carrier; no external storage is inferred for NULL/inline.
pub(crate) enum UserStorage {
    Absent,
    Inline,
    Single { reference: u32 },
}

pub(crate) struct Source {
    image: NativeImage,
    table: Table,
}

impl Source {
    pub(crate) fn capture(
        image: NativeImage,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        let table = observe(&image, table, budget, cancelled)?;
        Ok(Self { image, table })
    }

    pub(crate) fn into_image(self) -> NativeImage {
        self.image
    }

    pub(crate) fn image(&self) -> &NativeImage {
        &self.image
    }

    pub(crate) fn table(&self) -> &Table {
        &self.table
    }
}

pub(crate) fn observe(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Table, Error> {
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
    let source = normal_ordinary::capture_schema(image, table, budget, cancelled)?;
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
    let (schema, _, _) = source.table.into_parts();
    if schema.definition_page() != USER_DEFINITION {
        return Err(Error::Source);
    }
    let properties = inspect_table_properties(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )?;
    if graph.user.object_id() != schema.object_id() || properties.object_id() != schema.object_id()
    {
        return Err(Error::Source);
    }
    dependencies(&properties, &schema, budget, cancelled)?;

    let catalog = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        "MSysObjects",
        budget,
        cancelled,
    )?;
    let property_column = column(&catalog)?;
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let descriptor = descriptor::capture(&mut reader, &catalog, property_column, graph.user)?;
    let kind = descriptor.kind()?;
    let ledger = ledger::capture(image.as_bytes(), image.catalog(), budget, cancelled)?;
    let (database_reference, database_bytes) = ledger
        .single_reference(graph.database.object_id(), budget, cancelled)?
        .ok_or(Error::Source)?;
    if database_bytes != graph.properties.raw_bytes().len() {
        return Err(Error::Source);
    }

    let user_storage = match kind {
        Kind::Null if properties.raw_bytes().is_empty() => UserStorage::Absent,
        Kind::Inline if !properties.raw_bytes().is_empty() => UserStorage::Inline,
        Kind::Single => {
            let (reference, bytes) = ledger
                .single_reference(schema.object_id(), budget, cancelled)?
                .ok_or(Error::Source)?;
            if bytes != properties.raw_bytes().len()
                || reference == database_reference
                || reference_page(reference) != reference_page(database_reference)
            {
                return Err(Error::Source);
            }
            UserStorage::Single { reference }
        }
        _ => return Err(Error::Source),
    };
    let owned = match &user_storage {
        UserStorage::Single { reference, .. } => ledger.two_singles(
            [
                (schema.object_id(), *reference, properties.raw_bytes().len()),
                (
                    graph.database.object_id(),
                    database_reference,
                    database_bytes,
                ),
            ],
            budget,
            cancelled,
        )?,
        UserStorage::Absent | UserStorage::Inline => ledger.sole_single(
            graph.database.object_id(),
            database_reference,
            database_bytes,
            budget,
            cancelled,
        )?,
    };
    if !owned {
        return Err(Error::Source);
    }
    let property_page = reference_page(database_reference);
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
        let expected = if origin.object_id() == schema.object_id() {
            match &user_storage {
                UserStorage::Absent => None,
                UserStorage::Inline => Some(schema_source::PropertyPayload::Inline(
                    properties.raw_bytes(),
                )),
                UserStorage::Single { reference, .. } => {
                    Some(schema_source::PropertyPayload::Single(
                        *reference,
                        properties.raw_bytes().len(),
                    ))
                }
            }
        } else if origin.object_id() == graph.database.object_id() {
            Some(schema_source::PropertyPayload::Single(
                database_reference,
                database_bytes,
            ))
        } else {
            None
        };
        schema_source::require_external_payload(
            image, &catalog, origin, expected, budget, cancelled,
        )
        .map_err(graph_error)?;
    }
    Ok(Table {
        schema,
        properties,
        catalog,
        user_storage,
        database: graph.properties,
        database_reference,
    })
}

pub(in crate::table::mutation) fn dependencies(
    properties: &TableProperties,
    schema: &TableSchema,
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
                for column in schema.columns() {
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
