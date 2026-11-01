//! Complete joint target and carrier preflight before any record visitor.
use super::indexed::CheckedIndexes;
use super::name_work;
use crate::table::mutation::{preserved_source::Source, requests::FixedColumnUpdates};
use crate::{
    EditError, FixedScalarUpdate, InspectError, NativeImage, WriteBlocker,
    format::data_type::{
        BOOLEAN, BOOLEAN_BYTES, CURRENCY, CURRENCY_BYTES, DATE_TIME, DATE_TIME_BYTES, INT32,
        INT32_BYTES, INT64, INT64_BYTES,
    },
    header::check,
    table::mutation::{field, rows},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

pub(in crate::table::mutation) struct Bound {
    pub(in crate::table::mutation) column: usize,
    pub(in crate::table::mutation) kind: ScalarType,
    pub(in crate::table::mutation) width: usize,
}

fn carrier(kind: ScalarType, value: &Value) -> bool {
    matches!(
        (kind, value),
        (ScalarType::Boolean, Value::Boolean(_))
            | (ScalarType::Int32, Value::Null | Value::Int32(_))
            | (ScalarType::Int64, Value::Null | Value::Int64(_))
            | (ScalarType::Currency, Value::Null | Value::Currency(_))
            | (ScalarType::DateTime, Value::Null | Value::DateTime(_))
    )
}

#[cfg(feature = "ace16-write-preview")]
pub(super) fn all(
    source: &Source,
    groups: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<Bound>, usize), EditError> {
    all_with_minimum(source, groups, 2, None, budget, cancelled)
}

#[cfg(feature = "journal")]
pub(in crate::table::mutation) fn all_normal(
    source: &Source,
    groups: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<Bound>, usize), EditError> {
    all_with_minimum(source, groups, 1, None, budget, cancelled)
}

#[cfg(feature = "journal")]
pub(in crate::table::mutation) fn all_indexed(
    image: &NativeImage,
    source: &Source,
    groups: &[FixedColumnUpdates<'_>],
    indexed: &CheckedIndexes<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<Bound>, usize), EditError> {
    indexed.require_source(image, source.table.schema(), budget, cancelled)?;
    all_with_minimum(source, groups, 1, Some(indexed), budget, cancelled)
}

fn all_with_minimum(
    source: &Source,
    groups: &[FixedColumnUpdates<'_>],
    minimum: usize,
    indexed: Option<&CheckedIndexes<'_>>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<Bound>, usize), EditError> {
    check(budget, cancelled)?;
    if groups.len() > NativeImage::MAX_FIXED_BATCH_GROUPS {
        return Err(InspectError::StructureLimit.into());
    }
    if groups.len() < minimum {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    let schema = source.table.schema();
    let mut assignments = 0_usize;
    budget
        .charge_allocation(groups.len() as u64, size_of::<Bound>() as u64)
        .map_err(InspectError::from)?;
    let mut bound = Vec::new();
    bound
        .try_reserve_exact(groups.len())
        .map_err(|_| InspectError::Allocation)?;

    for group in groups {
        check(budget, cancelled)?;
        assignments = assignments
            .checked_add(group.updates().len())
            .filter(|count| *count <= NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS)
            .ok_or(InspectError::StructureLimit)?;
        name_work(schema.name(), group.table(), budget)?;
        if !schema.name().eq_ignore_ascii_case(group.table()) {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }

        let mut selected = None;
        for (position, field) in schema.columns().iter().enumerate() {
            check(budget, cancelled)?;
            name_work(field.name(), group.column(), budget)?;
            if field.name().eq_ignore_ascii_case(group.column())
                && selected.replace(position).is_some()
            {
                return Err(EditError::Preservation);
            }
        }
        let column = selected.ok_or(EditError::Unsupported(WriteBlocker::Column))?;
        budget
            .charge(Resource::Steps, bound.len() as u64)
            .map_err(InspectError::from)?;
        if bound.iter().any(|prior: &Bound| prior.column == column) {
            return Err(EditError::Conflict);
        }
        bound.push(qualify_with_indexes(
            source,
            column,
            group.updates(),
            indexed,
            budget,
            cancelled,
        )?);
    }
    check(budget, cancelled)?;
    Ok((bound, assignments))
}

// Callers resolve and reject duplicate targets before field qualification.
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table::mutation) fn qualify_selected(
    source: &Source,
    column: usize,
    updates: &[FixedScalarUpdate],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Bound, EditError> {
    qualify_with_indexes(source, column, updates, None, budget, cancelled)
}

fn qualify_with_indexes(
    source: &Source,
    column: usize,
    updates: &[FixedScalarUpdate],
    indexed: Option<&CheckedIndexes<'_>>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Bound, EditError> {
    let schema = source.table.schema();
    let field = &schema.columns()[column];
    let (kind, width) = match (field.native_type(), field.scalar_type()) {
        (BOOLEAN, Some(ScalarType::Boolean)) => (ScalarType::Boolean, BOOLEAN_BYTES),
        (INT32, Some(ScalarType::Int32)) => (ScalarType::Int32, INT32_BYTES),
        (INT64, Some(ScalarType::Int64)) => (ScalarType::Int64, INT64_BYTES),
        (CURRENCY, Some(ScalarType::Currency)) => (ScalarType::Currency, CURRENCY_BYTES),
        (DATE_TIME, Some(ScalarType::DateTime)) => (ScalarType::DateTime, DATE_TIME_BYTES),
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    };
    field::fixed_column_at(schema, column, kind, width)?;

    for dependency in &source.dependencies {
        check(budget, cancelled)?;
        if *dependency == column {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }
    for index in source.indexes.indexes() {
        for key in index.columns() {
            check(budget, cancelled)?;
            if key.number() == column {
                let Some(indexed) = indexed else {
                    return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                };
                indexed.require_target(schema, column, budget, cancelled)?;
            }
        }
    }
    rows::unique(updates, budget, cancelled)?;
    for update in updates {
        check(budget, cancelled)?;
        if !carrier(kind, update.expected()) || !carrier(kind, update.value()) {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        if update.row() >= source.records.len() {
            return Err(EditError::Conflict);
        }
    }
    Ok(Bound {
        column,
        kind,
        width,
    })
}
