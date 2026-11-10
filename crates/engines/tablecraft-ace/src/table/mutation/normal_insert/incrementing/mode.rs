//! Decode a closed source declaration; LAST never selects a generation mode.
mod indexes;
use crate::{
    EditError, InspectError, NativeImage, PropertyGroupKind, TableSchema, WriteBlocker,
    format::{
        data_type::{
            BOOLEAN, BYTE, CURRENCY, DATE_TIME, DOUBLE, GUID, INT16, INT32, INT64, NUMERIC,
            SHORT_BINARY, SHORT_TEXT, SINGLE,
        },
        layout::{AUTO_NUMBER_FLAG, AUTO_NUMBER_GUID_FLAG},
        page::PAGE_BYTES,
    },
    header::check,
    inspect_table_properties,
    table::{
        indexes::page::LEAF,
        mutation::{
            autonumber_counter,
            insert_defaults::{declaration, literal},
            preserved_source::Source,
            record_insertion,
        },
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const LOCAL_INDEX_KIND_MAX: u8 = 1;
const MEMO_PROPERTY: u8 = 12;
const DDL_PROPERTY: u8 = 1;
const MAX_DESCRIPTION_UNITS: usize = 255;
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
    inspect_with_geometry(image, schema, Geometry::Existing, budget, cancelled)
}

fn inspect_with_geometry(
    image: &NativeImage,
    schema: &TableSchema,
    geometry: Geometry,
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
    match geometry {
        Geometry::Existing => crate::properties::framing::validate(&properties, budget, cancelled)?,
        #[cfg(feature = "ace16-create")]
        Geometry::Current => {
            crate::properties::framing::current_schema_names(&properties, budget, cancelled)?;
        }
    }
    if companion_declarations(schema, generated, &properties, budget, cancelled)? {
        return Ok(Mode::Increment);
    }
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Geometry {
    Existing,
    #[cfg(feature = "ace16-create")]
    Current,
}

pub(super) fn capture(
    image: &NativeImage,
    source: &Source,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<autonumber_counter::Source, EditError> {
    capture_with_geometry(image, source, Geometry::Existing, budget, cancelled)
}

#[cfg(feature = "ace16-create")]
pub(in crate::table::mutation::normal_insert) fn capture_current(
    image: &NativeImage,
    source: &Source,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<autonumber_counter::Source, EditError> {
    capture_with_geometry(image, source, Geometry::Current, budget, cancelled)
}

fn capture_with_geometry(
    image: &NativeImage,
    source: &Source,
    geometry: Geometry,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<autonumber_counter::Source, EditError> {
    let schema = source.table.schema();

    let sparse = schema.layout.max_columns != schema.columns().len();
    if schema.columns().is_empty()
        || geometry == Geometry::Existing && sparse && schema.layout.variable_slots != 0
        || schema.layout.physical_indexes == 0
        || schema.layout.logical_indexes != source.indexes.indexes().len()
    {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }

    let mut generated = None;

    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if geometry == Geometry::Existing
            && sparse
            && (!field.fixed() || field.native_type() != INT32)
            || (!sparse && field.number() != position)
            || (sparse
                && (field.number() >= schema.layout.max_columns
                    || position != 0 && schema.columns()[position - 1].number() >= field.number()))
            || !matches!(
                field.native_type(),
                BOOLEAN
                    | BYTE
                    | INT16
                    | INT32
                    | INT64
                    | CURRENCY
                    | SINGLE
                    | DOUBLE
                    | DATE_TIME
                    | GUID
                    | NUMERIC
                    | SHORT_TEXT
                    | SHORT_BINARY
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
    if geometry == Geometry::Existing {
        if sparse {
            record_insertion::layout::sparse_incrementing_variables(
                schema, column, budget, cancelled,
            )?;
        } else {
            record_insertion::layout::incrementing_variables(schema, column, budget, cancelled)?;
        }
    }

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

    let mode = match geometry {
        Geometry::Existing => inspect(image, schema, budget, cancelled)?,
        #[cfg(feature = "ace16-create")]
        Geometry::Current => inspect_with_geometry(image, schema, geometry, budget, cancelled)?,
    };
    if mode != Mode::Increment {
        return Err(EditError::Unsupported(WriteBlocker::Properties));
    }

    Ok(counter)
}

// Checked constants and Boolean policies on ordinary companions, plus bounded
// inert Description metadata on any field, preserve Increment mode. The final source audit enforces these
// policies on complete stored rows, including independently expanded omissions.
fn companion_declarations(
    schema: &TableSchema,
    generated: usize,
    properties: &crate::TableProperties,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    if properties.groups().is_empty() {
        return Ok(false);
    }
    for (group_position, group) in properties.groups().iter().enumerate() {
        check(budget, cancelled)?;
        if group.kind() != PropertyGroupKind::Column || group.properties().is_empty() {
            return Ok(false);
        }
        for prior in &properties.groups()[..group_position] {
            check(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    (prior.name().len() + group.name().len()) as u64,
                )
                .map_err(InspectError::from)?;
            if prior.name().eq_ignore_ascii_case(group.name()) {
                return Ok(false);
            }
        }
        let mut selected = None;
        for (position, column) in schema.columns().iter().enumerate() {
            check(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    (column.name().len() + group.name().len()) as u64,
                )
                .map_err(InspectError::from)?;
            if !column.name().eq_ignore_ascii_case(group.name()) {
                continue;
            }
            if selected.replace((position, column)).is_some() {
                return Ok(false);
            }
        }
        let Some((position, column)) = selected else {
            return Ok(false);
        };
        let mut names = [false; 4];
        for property in group.properties() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, property.name().len() as u64)
                .map_err(InspectError::from)?;
            if property.flags() != DDL_PROPERTY {
                return Ok(false);
            }
            let description = property.name().eq_ignore_ascii_case("Description");
            if position == generated && !description {
                return Ok(false);
            }
            let selected = if description {
                if property.native_type() != SHORT_TEXT {
                    return Ok(false);
                }
                let text = property.text()?;
                budget
                    .charge(Resource::Steps, text.len() as u64)
                    .map_err(InspectError::from)?;
                if text.encode_utf16().take(MAX_DESCRIPTION_UNITS + 1).count()
                    > MAX_DESCRIPTION_UNITS
                {
                    return Ok(false);
                }
                3
            } else if property.name().eq_ignore_ascii_case("DefaultValue") {
                if property.native_type() != MEMO_PROPERTY
                    || declaration::capture(column, properties, budget, cancelled)?.is_none()
                {
                    return Ok(false);
                }
                literal::resolve(column, property.text()?, budget, cancelled)?;
                0
            } else if property.name().eq_ignore_ascii_case("Required") {
                if property.native_type() != BOOLEAN || property.boolean().is_err() {
                    return Ok(false);
                }
                1
            } else if property.name().eq_ignore_ascii_case("AllowZeroLength") {
                if column.native_type() != SHORT_TEXT
                    || property.native_type() != BOOLEAN
                    || property.boolean().is_err()
                {
                    return Ok(false);
                }
                2
            } else {
                return Ok(false);
            };
            if names[selected] {
                return Ok(false);
            }
            names[selected] = true;
        }
    }
    check(budget, cancelled)?;
    Ok(true)
}

#[cfg(test)]
mod description_tests;
