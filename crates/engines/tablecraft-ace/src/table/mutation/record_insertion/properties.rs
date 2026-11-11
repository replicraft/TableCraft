//! INSERT-only declaration coverage for explicitly requested empty Text.
use crate::RowInsert;
use crate::{
    EditError, NativeImage, TableProperties, TableSchema, header::check, inspect_table_properties,
    table::mutation::text_declarations,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::ScalarType;

/// The before-INSERT gate must already have qualified stored property types.
/// Presence is required here, not a true value: an explicit false remains the
/// existing final field-constraint violation rather than an inferred default.
pub(in crate::table::mutation) fn require_text_declarations(
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
        text_declarations::require(inspected, schema, column, budget, cancelled)?;
    }

    check(budget, cancelled)?;
    Ok(())
}
