use super::schema::TableSchema;
use crate::{InspectError, format::row::COLUMN_COUNT_BYTES};

pub(super) struct Geometry {
    pub fixed_end: usize,
    pub variable_slots: usize,
    pub minimum_variable_slots: usize,
}

impl Geometry {
    pub(super) fn ambiguous_tail(&self) -> bool {
        self.minimum_variable_slots == 0 && self.variable_slots != 0
    }
}

// Visible identities bound where deleted variable columns could have existed.
// The stored row supplies its own offset count; no deleted type is invented.
pub(super) fn geometry(schema: &TableSchema, count: usize) -> Result<Geometry, InspectError> {
    if count == schema.layout.max_columns {
        return Ok(Geometry {
            fixed_end: schema.layout.fixed_end,
            variable_slots: schema.layout.variable_slots,
            minimum_variable_slots: usize::from(schema.layout.variable_slots != 0),
        });
    }
    if count == 0 || count > schema.layout.max_columns {
        return Err(InspectError::Unsupported);
    }
    let mut present = 0;
    let mut present_variables = 0;
    let mut absent = 0;
    let mut absent_variables = 0;
    let mut result = Geometry {
        fixed_end: COLUMN_COUNT_BYTES,
        variable_slots: 0,
        minimum_variable_slots: 0,
    };
    for column in &schema.columns {
        let field = &column.field;
        if field.number < count {
            present += 1;
            if field.fixed {
                result.fixed_end = result
                    .fixed_end
                    .max(COLUMN_COUNT_BYTES + field.offset + field.length);
            } else {
                present_variables += 1;
                result.minimum_variable_slots =
                    result.minimum_variable_slots.max(field.variable + 1);
            }
        } else {
            absent += 1;
            absent_variables += usize::from(!field.fixed);
        }
    }
    let past_gaps = count.checked_sub(present).ok_or(InspectError::Malformed)?;
    let future_gaps = (schema.layout.max_columns - count)
        .checked_sub(absent)
        .ok_or(InspectError::Malformed)?;
    let available_variables = schema
        .layout
        .variable_slots
        .checked_sub(absent_variables)
        .ok_or(InspectError::Malformed)?;
    result.variable_slots = available_variables.min(present_variables + past_gaps);
    result.minimum_variable_slots = result
        .minimum_variable_slots
        .max(available_variables.saturating_sub(future_gaps));
    if result.minimum_variable_slots > result.variable_slots {
        return Err(InspectError::Malformed);
    }
    Ok(result)
}
