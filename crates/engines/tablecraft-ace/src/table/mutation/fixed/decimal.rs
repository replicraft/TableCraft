//! Decimal has a fixed carrier width inside a native variable-field slot.
use super::super::{EditError, WriteBlocker};
use crate::{
    InspectError, TableSchema,
    format::{
        bytes::u16_at,
        data_type::NUMERIC_BYTES,
        layout::UPDATABLE_FLAG,
        row::{VARIABLE_COUNT_BYTES, VARIABLE_OFFSET_BYTES},
    },
};
use std::ops::Range;
use tablecraft_schema::ScalarType;

pub(super) fn column(schema: &TableSchema, name: &str) -> Result<usize, EditError> {
    let position = schema
        .columns
        .iter()
        .position(|c| c.name().eq_ignore_ascii_case(name))
        .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
    let field = &schema.columns[position];
    if field.scalar_type() != Some(ScalarType::Decimal)
        || field.fixed()
        || field.field.length != NUMERIC_BYTES
        || field.field.flags != UPDATABLE_FLAG
        || field.field.extended_flags != 0
        || schema.columns.iter().enumerate().any(|(other, c)| {
            other != position && !c.fixed() && c.field.variable == field.field.variable
        })
    {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    Ok(position)
}
pub(super) fn span(
    schema: &TableSchema,
    column: usize,
    data: &[u8],
    mask: usize,
) -> Result<Range<usize>, EditError> {
    let count_pos = mask
        .checked_sub(VARIABLE_COUNT_BYTES)
        .ok_or(InspectError::Malformed)?;
    let variables = usize::from(u16_at(data, count_pos)?);
    let selected = schema.columns[column].field.variable;
    if variables == 0 || variables > schema.layout.variable_slots || selected >= variables {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let tail = count_pos
        .checked_sub(VARIABLE_OFFSET_BYTES * (variables + 1))
        .ok_or(InspectError::Malformed)?;
    let mut previous = schema.layout.fixed_end;
    let mut start = None;
    let mut end = None;
    for slot in 0..=variables {
        let next = usize::from(u16_at(
            data,
            count_pos - VARIABLE_OFFSET_BYTES * (slot + 1),
        )?);
        if next < previous || next > tail {
            return Err(InspectError::Malformed.into());
        }
        if slot == selected {
            start = Some(next);
        }
        if slot == selected + 1 {
            end = Some(next);
        }
        previous = next;
    }
    Ok(start.ok_or(InspectError::Malformed)?..end.ok_or(InspectError::Malformed)?)
}
