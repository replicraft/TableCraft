//! Read-only ordinary integral-index qualification; no key mutation authority.
mod ownership;
use super::ColumnFraming;
use crate::{
    EditError, IndexCheckStatus, IndexInspection, InspectError, NativeColumn, NativeImage,
    NativeIndex, TableSnapshot, WriteBlocker,
    format::{
        allocation,
        data_type::{BYTE, CURRENCY, INT16, INT32, INT64},
        reader::Reader,
    },
    header::check,
    table::{indexes::check_table_retained, mutation::lval_ownership::Inventory},
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

fn supported(field: &NativeColumn) -> bool {
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
    admission: Option<ColumnFraming>,
    long_values: &Inventory,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    let Some(framing) = admission else {
        return Ok(false);
    };
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
            wide |= matches!(field.native_type(), BYTE | INT16 | INT64 | CURRENCY);
        }
    }
    if !wide {
        return Ok(false);
    }
    for index in indexes.indexes() {
        check(budget, cancelled)?;
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
            || !supported(field)
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
