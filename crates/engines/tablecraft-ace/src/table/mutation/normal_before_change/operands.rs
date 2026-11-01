//! Raw original bindings for every actual readable Int32 macro operand.
use crate::{
    EditError, InspectError,
    format::{
        data_type::INT32,
        row::{COLUMN_COUNT_BYTES, MASK_BITS},
    },
    header::check,
    table::{mutation::preserved_source::Source, record_layout},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

/// Bind every readable Int32 operand, including counters and indexed fields,
/// to its original physical row. Opaque siblings never acquire frame values.
pub(in crate::table::mutation::normal_before_change) fn original_frames(
    source: &Source,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let schema = source.table.schema();
    for (ordinal, record) in source.records.iter().enumerate() {
        check(budget, cancelled)?;
        let layout = record_layout::parse(&record.data, schema, budget)?;
        let row = source
            .table
            .rows()
            .get(ordinal)
            .ok_or(EditError::Preservation)?;
        for field in schema.columns() {
            check(budget, cancelled)?;
            if field.native_type() != INT32
                || field.scalar_type() != Some(ScalarType::Int32)
                || !field.fixed()
                || field.storage_length() != size_of::<i32>()
                || field.extended_flags() != 0
            {
                continue;
            }

            let mut position = None;
            for (dense, column) in source.table.columns().iter().enumerate() {
                check(budget, cancelled)?;
                budget
                    .charge(
                        Resource::Steps,
                        field.name().len().max(column.name().len()) as u64,
                    )
                    .map_err(InspectError::from)?;
                if !field.name().eq_ignore_ascii_case(column.name()) {
                    continue;
                }
                if position.replace(dense).is_some() {
                    return Err(EditError::Preservation);
                }
            }
            let value = row
                .get(position.ok_or(EditError::Preservation)?)
                .ok_or(EditError::Preservation)?;
            let start = COLUMN_COUNT_BYTES
                .checked_add(field.field.offset)
                .ok_or(EditError::Preservation)?;
            let end = start
                .checked_add(size_of::<i32>())
                .ok_or(EditError::Preservation)?;
            let mask = layout
                .mask_start
                .checked_add(field.number() / MASK_BITS)
                .ok_or(EditError::Preservation)?;
            let bit = record.data.get(mask).ok_or(EditError::Preservation)?;
            if end > layout.mask_start {
                return Err(EditError::Preservation);
            }
            if bit & (1 << (field.number() % MASK_BITS)) == 0 {
                if value != &Value::Null {
                    return Err(EditError::Preservation);
                }
                continue;
            }

            budget
                .charge(Resource::Steps, size_of::<i32>() as u64)
                .map_err(InspectError::from)?;
            let bytes = record.data.get(start..end).ok_or(EditError::Preservation)?;
            let decoded =
                i32::from_le_bytes(bytes.try_into().map_err(|_| EditError::Preservation)?);
            if value != &Value::Int32(decoded) {
                return Err(EditError::Preservation);
            }
        }
    }
    Ok(())
}
