//! Paid native target selection before ambiguous NULL intent dispatch.
use super::super::{FixedColumnUpdates, name_work};
use crate::{
    EditError, NativeColumn, TableSchema, WriteBlocker,
    format::data_type::{CURRENCY, DATE_TIME, INT64},
    table::mutation::field,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::ScalarType;

pub(super) const PAYLOAD_BYTES: usize = size_of::<i64>();

pub(in crate::table::mutation::fixed::preserved) enum Target {
    Int32(usize),
    Scalar { column: usize, kind: ScalarType },
}

pub(in crate::table::mutation::fixed::preserved) fn requested_non_null(
    groups: &[FixedColumnUpdates<'_>],
) -> bool {
    let [group] = groups else {
        return false;
    };
    group.updates().first().is_some_and(|update| {
        matches!(
            update.expected(),
            Value::Int64(_) | Value::Currency(_) | Value::DateTime(_)
        )
    })
}

pub(in crate::table::mutation::fixed::preserved) fn native_kind(
    field: &NativeColumn,
) -> Result<ScalarType, EditError> {
    match (field.native_type(), field.scalar_type()) {
        (INT64, Some(ScalarType::Int64)) => Ok(ScalarType::Int64),
        (CURRENCY, Some(ScalarType::Currency)) => Ok(ScalarType::Currency),
        (DATE_TIME, Some(ScalarType::DateTime)) => Ok(ScalarType::DateTime),
        _ => Err(EditError::Unsupported(WriteBlocker::Column)),
    }
}

pub(super) fn position(
    schema: &TableSchema,
    group: &FixedColumnUpdates<'_>,
    budget: &mut ResourceBudget,
) -> Result<usize, EditError> {
    let mut selected = None;
    for (position, field) in schema.columns().iter().enumerate() {
        name_work(field.name(), group.column(), budget)?;
        if field.name().eq_ignore_ascii_case(group.column()) && selected.replace(position).is_some()
        {
            return Err(EditError::Preservation);
        }
    }
    selected.ok_or(EditError::Unsupported(WriteBlocker::Column))
}

pub(in crate::table::mutation::fixed::preserved) fn bind_null_target(
    schema: &TableSchema,
    group: &FixedColumnUpdates<'_>,
    budget: &mut ResourceBudget,
) -> Result<Target, EditError> {
    // This replaces, rather than precedes, the old numeric paid name loop.
    let column = position(schema, group, budget)?;
    if schema.columns()[column].scalar_type() == Some(ScalarType::Int32) {
        field::fixed_column_at(schema, column, ScalarType::Int32, size_of::<i32>())?;
        return Ok(Target::Int32(column));
    }
    let kind = native_kind(&schema.columns()[column])?;
    field::fixed_column_at(schema, column, kind, PAYLOAD_BYTES)?;
    Ok(Target::Scalar { column, kind })
}
