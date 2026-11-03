//! Complete readonly scalar indexes; GUID remains dense UPDATE Source only.
mod ownership;
use super::{ColumnFraming, ReadProfile, RetainedKeys};
use crate::{
    EditError, IndexCheckStatus, IndexInspection, InspectError, NativeColumn, NativeImage,
    NativeIndex, TableSnapshot, WriteBlocker,
    format::{
        allocation,
        data_type::{
            BYTE, CURRENCY, GUID, INT16, INT32, INT64, MAX_SHORT_BINARY_BYTES,
            MAX_SHORT_TEXT_UNITS, SHORT_BINARY, SHORT_TEXT,
        },
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        reader::Reader,
    },
    header::check,
    table::{
        indexes::{check_table_retained, validate_manual_guid_tuple},
        mutation::lval_ownership::Inventory,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Guid,
};
use tablecraft_schema::ScalarType;

fn supported(field: &NativeColumn, profile: ReadProfile) -> bool {
    let keys = profile.keys;
    if matches!(profile.columns, ColumnFraming::Dense)
        && matches!(keys, RetainedKeys::Scalar)
        && field.native_type() == GUID
        && field.scalar_type() == Some(ScalarType::Guid)
    {
        return field.fixed()
            && field.storage_length() == Guid::BYTE_LEN
            && field.flags() == (FIXED_FLAG | UPDATABLE_FLAG)
            && field.extended_flags() == 0
            && field.complex_type_id().is_none();
    }
    if matches!(keys, RetainedKeys::Scalar)
        && field.native_type() == SHORT_TEXT
        && field.scalar_type() == Some(ScalarType::Text)
    {
        return !field.fixed()
            && field.flags() == UPDATABLE_FLAG
            && field.extended_flags() == 0
            && field.storage_length() > 0
            && field.storage_length().is_multiple_of(size_of::<u16>())
            && field.storage_length() / size_of::<u16>() <= MAX_SHORT_TEXT_UNITS
            && field
                .text_sort_order()
                .and_then(|order| order.general_family())
                .is_some();
    }
    if matches!(keys, RetainedKeys::Scalar)
        && field.native_type() == SHORT_BINARY
        && field.scalar_type() == Some(ScalarType::Binary)
    {
        return !field.fixed()
            && field.extended_flags() == 0
            && (1..=MAX_SHORT_BINARY_BYTES).contains(&field.storage_length());
    }

    let width = match (field.native_type(), field.scalar_type()) {
        (BYTE, Some(ScalarType::Byte)) => size_of::<u8>(),
        (INT16, Some(ScalarType::Int16)) => size_of::<i16>(),
        (INT32, Some(ScalarType::Int32)) => size_of::<i32>(),
        (INT64, Some(ScalarType::Int64)) => size_of::<i64>(),
        (CURRENCY, Some(ScalarType::Currency)) => size_of::<i64>(),
        _ => return false,
    };
    field.fixed() && field.extended_flags() == 0 && field.storage_length() == width
}

/// False is the unchanged all-I32/complex route, selected before any new read.
/// A selected wide profile either proves its complete original index population
/// or returns its actual error; there is no failed-read fallback.
pub(super) fn qualify(
    image: &NativeImage,
    indexes: &IndexInspection,
    projected: &TableSnapshot,
    admission: Option<ReadProfile>,
    long_values: &Inventory,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    let Some(profile) = admission else {
        return Ok(false);
    };
    let framing = profile.columns;
    let keys = profile.keys;
    let schema = indexes.schema();
    let mut wide = false;
    for index in indexes.indexes() {
        check(budget, cancelled)?;
        for key in index.columns() {
            check(budget, cancelled)?;
            let field = match framing {
                ColumnFraming::Dense => schema.columns().get(key.number()),
                ColumnFraming::Actions => {
                    budget
                        .charge(Resource::Steps, schema.columns().len() as u64)
                        .map_err(InspectError::from)?;
                    schema
                        .columns()
                        .iter()
                        .find(|field| field.number() == key.number())
                }
            }
            .ok_or(InspectError::Malformed)?;
            wide |= matches!(field.native_type(), BYTE | INT16 | INT64 | CURRENCY)
                || matches!(keys, RetainedKeys::Scalar)
                    && matches!(field.native_type(), SHORT_BINARY | SHORT_TEXT)
                || matches!(framing, ColumnFraming::Dense)
                    && matches!(keys, RetainedKeys::Scalar)
                    && field.native_type() == GUID;
        }
    }
    if !wide {
        return Ok(false);
    }
    for index in indexes.indexes() {
        check(budget, cancelled)?;
        if matches!(framing, ColumnFraming::Dense)
            && matches!(keys, RetainedKeys::Scalar)
            && index.columns().len() > 1
        {
            validate_manual_guid_tuple(schema, index, budget, cancelled).map_err(|error| {
                match error {
                    InspectError::Unsupported => {
                        EditError::Unsupported(WriteBlocker::ConstraintCoverage)
                    }
                    other => other.into(),
                }
            })?;
            continue;
        }
        let [key] = index.columns() else {
            return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
        };
        let field = match framing {
            ColumnFraming::Dense => schema.columns().get(key.number()),
            ColumnFraming::Actions => {
                budget
                    .charge(Resource::Steps, schema.columns().len() as u64)
                    .map_err(InspectError::from)?;
                schema
                    .columns()
                    .iter()
                    .find(|field| field.number() == key.number())
            }
        }
        .ok_or(InspectError::Malformed)?;
        if index.native_type() > 1
            || index.foreign_reference().is_some()
            || !index
                .constraints()
                .is_some_and(|constraints| !constraints.ignore_nulls())
            || field.number() != key.number()
            || !supported(field, profile)
        {
            return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
        }
    }
    let checked = check_table_retained(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        schema.name(),
        budget,
        cancelled,
    )?;
    if checked.schema().object_id() != schema.object_id()
        || checked.schema().definition_page() != schema.definition_page()
        || checked.rows() != projected.rows().len()
        || checked.schema().scalar_columns_with_budget(budget)? != projected.columns()
        || checked.indexes().len() != indexes.indexes().len()
        || checked
            .indexes()
            .iter()
            .any(|index| index.status() != IndexCheckStatus::Checked)
    {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    ownership::require(
        image,
        schema.definition_page(),
        long_values,
        budget,
        cancelled,
    )?;
    check(budget, cancelled)?;
    Ok(true)
}

/// Read members only after the strict check proved exact visited/owned closure.
/// Caller still enforces GLOBAL allocation, aliases and DATA/INDEX disjointness.
pub(super) fn pages(
    image: &NativeImage,
    index: &NativeIndex,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u32>, EditError> {
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    Ok(allocation::pages(&mut reader, index.usage_map())?)
}
