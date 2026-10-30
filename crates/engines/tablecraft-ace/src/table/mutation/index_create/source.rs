use super::{SecondaryIndexCreate, buffer, layout::DefinitionLayout};
use crate::{
    EditError, InspectError, NativeImage, TableCheck, WriteBlocker,
    format::{
        data_type,
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
    },
    header::check,
    limits::MAX_INDEXES,
    table::mutation::{gates, preview},
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::SchemaName;

const MAX_INDEX_NAME_UNITS: usize = 64;
const SUPPORTED_OLD_INDEX_FLAGS: u8 = 0x89;

pub(in crate::table) struct Original {
    pub checked: TableCheck,
    pub layout: DefinitionLayout,
    pub name: SchemaName,
    pub column: usize,
    pub descending: bool,
}

pub(in crate::table) fn qualify(
    image: &NativeImage,
    table: &str,
    request: &SecondaryIndexCreate<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Original, EditError> {
    // Also guards private fresh-owner construction, before any read/allocation.
    preview::qualify(image)?;
    check(budget, cancelled)?;
    if request.column.is_empty()
        || request.column.len() > MAX_INDEX_NAME_UNITS
        || !request.column.is_ascii()
        || request.name.is_empty()
        || request.name.len() > MAX_INDEX_NAME_UNITS
        || !request.name.is_ascii()
        || !request.name.as_bytes()[0].is_ascii_alphabetic()
        || !request
            .name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    let encoded_name = buffer::copy(request.name.as_bytes(), budget, cancelled)?;
    let name =
        SchemaName::new(String::from_utf8(encoded_name).map_err(|_| InspectError::Malformed)?)
            .map_err(InspectError::from)?;
    let checked = gates::before(image, table, budget, cancelled)?;
    let schema = checked.schema();
    if schema.columns().len() != schema.layout.max_columns
        || schema.layout.variable_slots != 0
        || schema.columns().is_empty()
        || schema.layout.physical_indexes >= MAX_INDEXES
        || schema.layout.logical_indexes >= MAX_INDEXES
    {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let mut column = None;
    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if !field.name().is_ascii()
            || field.number() != position
            || field.native_type() != data_type::INT32
            || !field.fixed()
            || field.storage_length() != size_of::<i32>()
            || field.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
            || field.extended_flags() != 0
        {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        budget
            .charge(
                Resource::Steps,
                (field.name().len() + request.column.len()) as u64,
            )
            .map_err(InspectError::from)?;
        if field.name().eq_ignore_ascii_case(request.column) {
            column = Some(field.number());
        }
    }
    let column = column.ok_or(InspectError::UnknownColumn)?;
    for (position, checked_index) in checked.indexes().iter().enumerate() {
        check(budget, cancelled)?;
        let index = checked_index.index();
        budget
            .charge(
                Resource::Steps,
                (index.name().len() + request.name.len()) as u64,
            )
            .map_err(InspectError::from)?;
        if index.name().eq_ignore_ascii_case(request.name) {
            return Err(EditError::Conflict);
        }
        if !index.name().is_ascii()
            || index.number() != position as u32
            || !matches!(index.native_type(), 0 | 1)
            || index.columns().len() != 1
            || index.flags() & !SUPPORTED_OLD_INDEX_FLAGS != 0
        {
            return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
        }
        if index.columns().iter().any(|key| key.number() == column) {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }
    let layout = DefinitionLayout::read(image, &checked, budget, cancelled)?;
    Ok(Original {
        checked,
        layout,
        name,
        column,
        descending: request.descending,
    })
}
