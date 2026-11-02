//! Decode a closed source declaration; LAST never selects a generation mode.
use crate::{
    EditError, InspectError, NativeImage, PropertyGroupKind, TableSchema, WriteBlocker,
    format::{
        data_type::INT32,
        layout::{AUTO_NUMBER_FLAG, AUTO_NUMBER_GUID_FLAG, FIXED_FLAG, UPDATABLE_FLAG},
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

const LONG_WIDTH: usize = size_of::<i32>();
const GENERATED_LONG_FLAGS: u8 = AUTO_NUMBER_FLAG | FIXED_FLAG | UPDATABLE_FLAG;
const ORDINARY_LONG_FLAGS: u8 = FIXED_FLAG | UPDATABLE_FLAG;
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
            .eq_ignore_ascii_case(schema.columns()[0].name())
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
    if schema.columns().len() != 2
        || schema.layout.max_columns != 2
        || schema.layout.variable_slots != 0
        || schema.layout.fixed_end != 2 + 2 * LONG_WIDTH
        || schema.layout.physical_indexes != 1
        || schema.layout.logical_indexes != 1
    {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if field.native_type() != INT32
            || !field.fixed()
            || field.number() != position
            || field.storage_length() != LONG_WIDTH
            || field.field.offset != position * LONG_WIDTH
            || field.extended_flags() != 0
            || field.complex_type_id().is_some()
            || field.flags()
                != if position == 0 {
                    GENERATED_LONG_FLAGS
                } else {
                    ORDINARY_LONG_FLAGS
                }
        {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
    }
    let [index] = source.indexes.indexes() else {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    };
    let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
    let [key] = index.columns() else {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    };
    if key.number() != 0
        || key.descending()
        || !constraints.primary_key()
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
    if inspect(image, schema, budget, cancelled)? != Mode::Increment {
        return Err(EditError::Unsupported(WriteBlocker::Properties));
    }
    record_insertion::layout::incrementing_variables(schema, 0, budget, cancelled)?;
    autonumber_counter::capture(image, schema, 0, budget, cancelled)
}
