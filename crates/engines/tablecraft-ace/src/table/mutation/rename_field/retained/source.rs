//! Positive current-schema proof and inert original storage-route selection.
use super::super::{FieldRenameError as Error, current_source::source_error};
use crate::{
    FileFormat, NativeImage,
    format::{
        bytes::{u16_at, u32_at},
        definition::LENGTH_OFFSET,
        page::PAGE_BYTES,
        reader::{Reader, row_count},
        row::{reference_page, reference_slot},
    },
    header::check,
    inspect_database_properties, inspect_table_schema,
    table::{
        long_value::trace::Kind,
        mutation::{
            catalog_property::{self, column, descriptor},
            current_schema,
            saved_field::Observed,
        },
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const SOURCE_PAGES: usize = 11;
const USER_DEFINITION: u32 = 7;
const FREE_OFFSET: usize = 2;
const DEFINITION_HEADER_BYTES: usize = 8;
const REFERENCE_OFFSET: usize = size_of::<u32>();

pub(super) struct Source {
    pub image: NativeImage,
    pub family: current_schema::Family,
    pub observed: Observed,
    pub maximum_name_units: usize,
}

impl Source {
    pub fn capture(
        image: NativeImage,
        table: &str,
        field: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        let proof = current_schema::Source::capture(image, table, budget, cancelled)
            .map_err(source_error)?;
        let (observed, maximum_name_units) =
            facts(proof.image(), proof.table(), field, budget, cancelled)?;
        Ok(Self {
            image: proof.into_image(),
            family: current_schema::Family::Current,
            observed,
            maximum_name_units,
        })
    }

    pub fn capture_retained_schema(
        image: NativeImage,
        table: &str,
        field: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, current_schema::Table), Error> {
        let proof = current_schema::Family::Retained
            .capture(image, table, budget, cancelled)
            .map_err(source_error)?;
        let (observed, maximum_name_units) =
            facts(proof.image(), proof.table(), field, budget, cancelled)?;
        let (image, table) = proof.into_parts();
        Ok((
            Self {
                image,
                family: current_schema::Family::Retained,
                observed,
                maximum_name_units,
            },
            table,
        ))
    }
}

pub(super) fn observe(
    image: &NativeImage,
    table: &str,
    field: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Observed, usize), Error> {
    let current = current_schema::observe(image, table, budget, cancelled).map_err(source_error)?;
    facts(image, &current, field, budget, cancelled)
}

pub(super) fn observe_retained_schema(
    image: &NativeImage,
    table: &str,
    field: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Observed, usize), Error> {
    let current = current_schema::Family::Retained
        .observe(image, table, budget, cancelled)
        .map_err(source_error)?;
    facts(image, &current, field, budget, cancelled)
}

fn facts(
    image: &NativeImage,
    current: &current_schema::Table,
    field: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Observed, usize), Error> {
    let observed = match current.family {
        current_schema::Family::Current => Observed::capture_current_rename(
            image,
            current.schema.name(),
            field,
            budget,
            cancelled,
        )?,
        current_schema::Family::Retained => Observed::capture_retained_schema_rename(
            image,
            current.schema.name(),
            field,
            budget,
            cancelled,
        )?,
    };
    let storage_matches = match (&current.user_storage, observed.property_reference) {
        (current_schema::UserStorage::Single { reference }, Some(actual)) => *reference == actual,
        (current_schema::UserStorage::Inline, None)
            if current.family == current_schema::Family::Retained =>
        {
            true
        }
        _ => false,
    };
    if !storage_matches || current.schema.object_id() != observed.schema.object_id() {
        return Err(Error::Source);
    }
    let base = observed.schema.definition_page() as usize * PAGE_BYTES;
    let page = image
        .as_bytes()
        .get(base..base + PAGE_BYTES)
        .ok_or(Error::Source)?;
    let length = u32_at(page, LENGTH_OFFSET)? as usize;
    let free = PAGE_BYTES
        .checked_sub(length + DEFINITION_HEADER_BYTES)
        .ok_or(Error::Source)?;
    if usize::from(u16_at(page, FREE_OFFSET)?) != free {
        return Err(Error::Source);
    }
    let capacity = match current.family {
        current_schema::Family::Current => {
            catalog_property::fitted_property_capacity(image, budget, cancelled)?
        }
        current_schema::Family::Retained => {
            catalog_property::retained_fitted_property_capacity(image, budget, cancelled)?
        }
    }
    .unwrap_or(0);
    let unchanged = observed
        .properties
        .raw_bytes()
        .len()
        .checked_sub(observed.identity.name_units * size_of::<u16>())
        .ok_or(Error::Source)?;
    let properties = capacity
        .checked_sub(unchanged)
        .map_or(0, |bytes| bytes / size_of::<u16>());
    let maximum = (observed.identity.name_units + free / size_of::<u16>())
        .min(properties)
        .min(crate::identifier::MAX_NAME_UNITS);
    Ok((observed, maximum))
}

pub(super) fn request(
    source: &Source,
    new_name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    for field in source.observed.schema.columns() {
        check(budget, cancelled)?;
        budget.charge(
            Resource::Steps,
            field.name().len().max(new_name.len()) as u64,
        )?;
        if field.number() != source.observed.identity.number
            && field.name().eq_ignore_ascii_case(new_name)
        {
            return Err(Error::DuplicateName);
        }
    }
    if new_name != source.observed.column().name() && new_name.len() > source.maximum_name_units {
        return Err(Error::FittedCapacity);
    }
    Ok(())
}

pub(super) fn selected(
    image: &NativeImage,
    table: &str,
    field: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, Error> {
    check(budget, cancelled)?;
    let header = image.catalog().header();
    if header.format() != FileFormat::Ace2016
        || header.subversion() != 0
        || header.encoded_pages()
        || image.as_bytes().len() != SOURCE_PAGES * PAGE_BYTES
    {
        return Ok(false);
    }
    let schema = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )?;
    if schema.definition_page() != USER_DEFINITION {
        return Ok(false);
    }
    let mut found = false;
    for candidate in schema.columns() {
        check(budget, cancelled)?;
        budget.charge(
            Resource::Steps,
            candidate.name().len().max(field.len()) as u64,
        )?;
        found |= candidate.name().eq_ignore_ascii_case(field);
    }
    if !found {
        return Err(Error::Source);
    }
    if schema.physical_layout().max_columns > schema.columns().len() {
        return Ok(true);
    }
    let catalog = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        "MSysObjects",
        budget,
        cancelled,
    )?;
    let origin = image
        .catalog()
        .origins()
        .iter()
        .find(|origin| origin.object_id() == schema.object_id())
        .ok_or(Error::Source)?;
    let database = inspect_database_properties(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        budget,
        cancelled,
    )?
    .ok_or(Error::Source)?;
    let database_origin = image
        .catalog()
        .origins()
        .iter()
        .find(|origin| origin.object_id() == database.object_id())
        .ok_or(Error::Source)?;
    let selected = column(&catalog)?;
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header,
        budget,
        cancelled,
        reads: 0,
    };
    let user = descriptor::capture(&mut reader, &catalog, selected, origin)?;
    let database = descriptor::capture(&mut reader, &catalog, selected, database_origin)?;
    if !matches!(user.kind()?, Kind::Single) || !matches!(database.kind()?, Kind::Single) {
        return Ok(false);
    }
    let user_span = user.absolute_range()?.ok_or(Error::Source)?;
    let database_span = database.absolute_range()?.ok_or(Error::Source)?;
    let user_reference = u32_at(image.as_bytes(), user_span.start + REFERENCE_OFFSET)?;
    let database_reference = u32_at(image.as_bytes(), database_span.start + REFERENCE_OFFSET)?;
    if reference_page(user_reference) != reference_page(database_reference) {
        return Ok(true);
    }
    let page = reader.page(reference_page(user_reference))?;
    let user_slot = usize::from(reference_slot(user_reference));
    let database_slot = usize::from(reference_slot(database_reference));
    Ok(user_slot.checked_add(1) != Some(database_slot)
        || database_slot.checked_add(1) != Some(row_count(&page)?))
}
