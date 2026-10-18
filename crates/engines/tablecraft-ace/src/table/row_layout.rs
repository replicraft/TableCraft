use super::schema::TableSchema;
use crate::{InspectError, format::row::COLUMN_COUNT_BYTES};

pub(super) struct Geometry {
    pub fixed_end: usize,
    pub variable_slots: usize,
}

// Only dense, append-only column histories are qualified for older row counts.
// Deleted/gapped histories cannot be reconstructed from this metadata profile.
pub(super) fn geometry(schema: &TableSchema, count: usize) -> Result<Geometry, InspectError> {
    if count == schema.layout.max_columns {
        return Ok(Geometry {
            fixed_end: schema.layout.fixed_end,
            variable_slots: schema.layout.variable_slots,
        });
    }
    if count == 0
        || count > schema.layout.max_columns
        || schema.columns.len() != schema.layout.max_columns
    {
        return Err(InspectError::Unsupported);
    }
    let mut variable_slots = 0;
    let mut result = Geometry {
        fixed_end: COLUMN_COUNT_BYTES,
        variable_slots: 0,
    };
    for (number, column) in schema.columns.iter().enumerate() {
        let field = &column.field;
        if field.number != number {
            return Err(InspectError::Unsupported);
        }
        if !field.fixed {
            if field.variable != variable_slots {
                return Err(InspectError::Unsupported);
            }
            variable_slots += 1;
        }
        if number < count {
            if field.fixed {
                result.fixed_end = result
                    .fixed_end
                    .max(COLUMN_COUNT_BYTES + field.offset + field.length);
            }
            result.variable_slots = variable_slots;
        }
    }
    if variable_slots != schema.layout.variable_slots {
        return Err(InspectError::Unsupported);
    }
    Ok(result)
}
