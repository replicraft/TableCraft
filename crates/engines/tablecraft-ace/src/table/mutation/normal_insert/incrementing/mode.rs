//! Decode a closed source declaration; LAST never selects a generation mode.
mod indexes;
use crate::{
    EditError, InspectError, NativeImage, PropertyGroupKind, TableSchema, WriteBlocker,
    format::{
        data_type::{BOOLEAN, CURRENCY, DATE_TIME, INT32, INT64, SHORT_BINARY, SHORT_TEXT},
        layout::{AUTO_NUMBER_FLAG, AUTO_NUMBER_GUID_FLAG},
        page::PAGE_BYTES,
    },
    header::check,
    inspect_table_properties,
    table::{
        indexes::page::LEAF,
        mutation::{autonumber_counter, preserved_source::Source, record_insertion},
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const LOCAL_INDEX_KIND_MAX: u8 = 1;
const MEMO_PROPERTY: u8 = 12;
const DDL_PROPERTY: u8 = 1;
const RANDOM_DEFAULT: &str = "GenUniqueID()";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Increment,
    Random,
    Unknown,
}

pub(in crate::table::mutation::normal_insert) fn selected(
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    let mut generated = false;

    for field in schema.columns() {
        check(budget, cancelled)?;
        generated |= field.flags() & (AUTO_NUMBER_FLAG | AUTO_NUMBER_GUID_FLAG) != 0;
    }

    Ok(generated)
}

pub(super) fn inspect(
    image: &NativeImage,
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Mode, EditError> {
    let mut generated = None;

    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if field.flags() & (AUTO_NUMBER_FLAG | AUTO_NUMBER_GUID_FLAG) != 0
            && generated.replace(position).is_some()
        {
            return Ok(Mode::Unknown);
        }
    }

    let Some(generated) = generated else {
        return Ok(Mode::Unknown);
    };

    let properties = inspect_table_properties(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        schema.name(),
        budget,
        cancelled,
    )?;
    budget
        .charge(Resource::Steps, schema.name().len() as u64)
        .map_err(InspectError::from)?;
    if properties.object_id() != schema.object_id() || properties.name() != schema.name() {
        return Err(EditError::Preservation);
    }

    if properties.raw_bytes().is_empty() {
        if !properties.groups().is_empty() {
            return Err(EditError::Preservation);
        }
        // This is complete inspected absence on the already-qualified native
        // AutoIncrement declaration, not an empty inventory fallback.
        return Ok(Mode::Increment);
    }
    crate::properties::framing::validate(&properties, budget, cancelled)?;
    let [group] = properties.groups() else {
        return Ok(Mode::Unknown);
    };
    budget
        .charge(Resource::Steps, group.name().len() as u64)
        .map_err(InspectError::from)?;
    if group.kind() != PropertyGroupKind::Column
        || !group
            .name()
            .eq_ignore_ascii_case(schema.columns()[generated].name())
    {
        return Ok(Mode::Unknown);
    }

    let [property] = group.properties() else {
        return Ok(Mode::Unknown);
    };
    budget
        .charge(
            Resource::Steps,
            property.name().len() as u64 + RANDOM_DEFAULT.len() as u64,
        )
        .map_err(InspectError::from)?;
    if property.name().eq_ignore_ascii_case("DefaultValue")
        && property.native_type() == MEMO_PROPERTY
        && property.flags() == DDL_PROPERTY
        && property.text()? == RANDOM_DEFAULT
    {
        return Ok(Mode::Random);
    }

    check(budget, cancelled)?;
    Ok(Mode::Unknown)
}

pub(super) fn capture(
    image: &NativeImage,
    source: &Source,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<autonumber_counter::Source, EditError> {
    let schema = source.table.schema();

    if schema.columns().is_empty()
        || schema.layout.max_columns != schema.columns().len()
        || schema.layout.physical_indexes == 0
        || schema.layout.logical_indexes != source.indexes.indexes().len()
    {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }

    let mut generated = None;

    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if field.number() != position
            || !matches!(
                field.native_type(),
                BOOLEAN | INT32 | INT64 | CURRENCY | DATE_TIME | SHORT_TEXT | SHORT_BINARY
            )
        {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        if field.flags() & (AUTO_NUMBER_FLAG | AUTO_NUMBER_GUID_FLAG) != 0
            && generated.replace(position).is_some()
        {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
    }

    let column = generated.ok_or(EditError::Unsupported(WriteBlocker::Column))?;
    let counter = autonumber_counter::capture(image, schema, column, budget, cancelled)?;
    record_insertion::layout::incrementing_variables(schema, column, budget, cancelled)?;

    if schema.layout.physical_indexes == 1 {
        // All logical aliases must name the same actual physical owner. One is
        // primary; ordinary required/unique aliases do not need primary kind 1.
        let first = source
            .indexes
            .indexes()
            .first()
            .ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?;
        let mut primaries = 0;

        for index in source.indexes.indexes() {
            check(budget, cancelled)?;
            let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
            let [key] = index.columns() else {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            };
            if index.data_number() != first.data_number()
                || index.root_page() != first.root_page()
                || index.usage_map() != first.usage_map()
                || index.flags() != first.flags()
                || index.native_type() > LOCAL_INDEX_KIND_MAX
                || key.number() != schema.columns()[column].number()
                || key.descending()
                || !constraints.unique()
                || !constraints.required()
                || constraints.ignore_nulls()
                || image
                    .as_bytes()
                    .get(index.root_page() as usize * PAGE_BYTES)
                    != Some(&LEAF)
            {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            primaries += usize::from(constraints.primary_key());
        }

        if primaries != 1 {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    } else {
        indexes::capture(image, source, column, budget, cancelled)?;
    }

    if inspect(image, schema, budget, cancelled)? != Mode::Increment {
        return Err(EditError::Unsupported(WriteBlocker::Properties));
    }

    Ok(counter)
}
