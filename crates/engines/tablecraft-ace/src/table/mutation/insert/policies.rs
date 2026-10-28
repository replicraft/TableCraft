//! INSERT-only declaration coverage for explicitly requested empty Text.
use super::RowInsert;
use crate::{
    EditError, NativeImage, PropertyGroupKind, TableProperties, TableSchema, WriteBlocker,
    header::check, inspect_table_properties,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::ScalarType;

const ALLOW_ZERO_LENGTH: &str = "AllowZeroLength";

/// The before-INSERT gate must already have qualified stored property types.
/// Presence is required here, not a true value: an explicit false remains the
/// existing final field-constraint violation rather than an inferred default.
pub(super) fn require_text_declarations(
    image: &NativeImage,
    schema: &TableSchema,
    requests: &[RowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let mut properties: Option<TableProperties> = None;

    for (position, column) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if column.scalar_type() != Some(ScalarType::Text) {
            continue;
        }
        let mut empty_requested = false;
        for request in requests {
            check(budget, cancelled)?;
            if matches!(request.values().get(position), Some(Value::Text(value)) if value.is_empty())
            {
                empty_requested = true;
                break;
            }
        }
        if !empty_requested {
            continue;
        }

        if properties.is_none() {
            let inspected = inspect_table_properties(
                &mut image.reader(),
                image.catalog().catalog().scope(),
                schema.name(),
                budget,
                cancelled,
            )?;
            if inspected.object_id() != schema.object_id() {
                return Err(EditError::Preservation);
            }
            properties = Some(inspected);
        }
        let inspected = properties.as_ref().ok_or(EditError::Preservation)?;
        let mut declared = false;
        for group in inspected.groups() {
            check(budget, cancelled)?;
            if group.kind() != PropertyGroupKind::Column
                || !group.name().eq_ignore_ascii_case(column.name())
            {
                continue;
            }
            for property in group.properties() {
                check(budget, cancelled)?;
                if property.name().eq_ignore_ascii_case(ALLOW_ZERO_LENGTH) {
                    declared = true;
                    break;
                }
            }
            // Match the constraint reader's first-group lookup exactly; a
            // later similarly named group cannot supply missing authority.
            break;
        }
        if !declared {
            return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
        }
    }

    check(budget, cancelled)?;
    Ok(())
}
