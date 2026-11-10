//! Count-selected Short Text updates preserve historical sibling payloads and framing.
use super::{
    historical_mixed::{self, Frame},
    storage,
};
use crate::{
    EditError, InspectError, TableSchema, WriteBlocker,
    format::{
        data_type::SHORT_TEXT,
        row::{COLUMN_COUNT_BYTES, MASK_BITS, VARIABLE_COUNT_BYTES, VARIABLE_OFFSET_BYTES},
    },
    header::check,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub(in crate::table::mutation) fn qualify(
    schema: &TableSchema,
    column: usize,
    before: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Frame, EditError> {
    let frame = historical_mixed::frame(schema, before, budget, cancelled)?;
    let field = schema.columns().get(column).ok_or(EditError::Conflict)?;
    if field.native_type() != SHORT_TEXT || field.fixed() {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    storage::qualify(field)?;
    Ok(frame)
}

pub(in crate::table::mutation) fn build(
    schema: &TableSchema,
    column: usize,
    before: &[u8],
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let frame = qualify(schema, column, before, budget, cancelled)?;
    if !matches!(value, Value::Text(_) | Value::Null) {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    let field = &schema.columns()[column];
    if field.number() < frame.count {
        return present(schema, column, before, value, &frame, budget, cancelled);
    }
    if matches!(value, Value::Null) {
        return super::copy(before, budget, cancelled);
    }
    let payload = storage::encode_preferred(field, value, budget, cancelled)?;
    let old_end = frame
        .original
        .offsets
        .last()
        .copied()
        .unwrap_or(frame.fixed);
    let delta = frame
        .expanded
        .checked_sub(frame.fixed)
        .ok_or(EditError::Preservation)?;
    let data_end = frame
        .original
        .data_end
        .checked_add(delta)
        .and_then(|length| length.checked_add(payload.len()))
        .ok_or(InspectError::StructureLimit)?;
    let footer = (frame.variables + 1)
        .checked_mul(VARIABLE_OFFSET_BYTES)
        .and_then(|length| length.checked_add(VARIABLE_COUNT_BYTES))
        .ok_or(InspectError::StructureLimit)?;
    let fields = schema.columns().len();
    let capacity = data_end
        .checked_add(footer)
        .and_then(|length| length.checked_add(fields.div_ceil(MASK_BITS)))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge_allocation(capacity as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, capacity as u64)
        .map_err(InspectError::from)?;
    let mut after = Vec::new();
    after
        .try_reserve_exact(capacity)
        .map_err(|_| InspectError::Allocation)?;
    after.extend(
        u16::try_from(fields)
            .map_err(|_| InspectError::StructureLimit)?
            .to_le_bytes(),
    );
    after.extend_from_slice(&before[COLUMN_COUNT_BYTES..frame.fixed]);
    after.resize(frame.expanded, 0);
    after.extend_from_slice(&before[frame.fixed..old_end]);
    after.extend_from_slice(&payload);
    after.extend_from_slice(&before[old_end..frame.original.data_end]);
    let old_variables = frame.original.offsets.len().saturating_sub(1);
    for variable in (0..=frame.variables).rev() {
        check(budget, cancelled)?;
        let offset = if variable <= old_variables {
            frame
                .original
                .offsets
                .get(variable)
                .copied()
                .unwrap_or(frame.fixed)
                + delta
        } else {
            old_end
                + delta
                + if variable > field.field.variable {
                    payload.len()
                } else {
                    0
                }
        };
        after.extend(
            u16::try_from(offset)
                .map_err(|_| InspectError::StructureLimit)?
                .to_le_bytes(),
        );
    }
    after.extend(
        u16::try_from(frame.variables)
            .map_err(|_| InspectError::StructureLimit)?
            .to_le_bytes(),
    );
    for byte in 0..fields.div_ceil(MASK_BITS) {
        check(budget, cancelled)?;
        let mut bits = before
            .get(frame.original.mask_start + byte)
            .copied()
            .unwrap_or(0);
        for bit in 0..MASK_BITS {
            let number = byte * MASK_BITS + bit;
            if number >= frame.count && number < fields {
                bits &= !(1 << bit);
                if number == column {
                    bits |= 1 << bit;
                }
            }
        }
        after.push(bits);
    }
    if after.len() != capacity {
        return Err(EditError::Preservation);
    }
    Ok(after)
}

fn present(
    schema: &TableSchema,
    column: usize,
    before: &[u8],
    value: &Value,
    frame: &Frame,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let field = &schema.columns()[column];
    let variable = field.physical_field().variable;
    let start = *frame
        .original
        .offsets
        .get(variable)
        .ok_or(EditError::Preservation)?;
    let stop = *frame
        .original
        .offsets
        .get(variable + 1)
        .ok_or(EditError::Preservation)?;
    let end = *frame
        .original
        .offsets
        .last()
        .ok_or(EditError::Preservation)?;
    let removed = stop.checked_sub(start).ok_or(EditError::Preservation)?;
    let payload = storage::encode_preferred(field, value, budget, cancelled)?;
    let data_end = frame
        .original
        .data_end
        .checked_sub(removed)
        .and_then(|bytes| bytes.checked_add(payload.len()))
        .ok_or(InspectError::StructureLimit)?
        .max(frame.original.data_end);
    let footer_bytes = before
        .len()
        .checked_sub(frame.original.data_end)
        .ok_or(EditError::Preservation)?;
    let length = data_end
        .checked_add(footer_bytes)
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge_allocation(length as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, length as u64)
        .map_err(InspectError::from)?;
    let mut after = Vec::new();
    after
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    after.extend_from_slice(&before[..start]);
    after.extend_from_slice(&payload);
    after.extend_from_slice(&before[stop..end]);
    after.extend_from_slice(&before[end..frame.original.data_end]);
    after.resize(data_end, 0);
    for (position, offset) in frame.original.offsets.iter().enumerate().rev() {
        check(budget, cancelled)?;
        let offset = if position <= variable {
            *offset
        } else {
            offset
                .checked_sub(removed)
                .and_then(|at| at.checked_add(payload.len()))
                .ok_or(InspectError::StructureLimit)?
        };
        after.extend(
            u16::try_from(offset)
                .map_err(|_| InspectError::StructureLimit)?
                .to_le_bytes(),
        );
    }
    let variables = frame.original.offsets.len() - 1;
    after.extend(
        u16::try_from(variables)
            .map_err(|_| InspectError::StructureLimit)?
            .to_le_bytes(),
    );
    let mask = after.len();
    after.extend_from_slice(&before[frame.original.mask_start..]);
    let byte = after
        .get_mut(mask + field.number() / MASK_BITS)
        .ok_or(EditError::Preservation)?;
    let bit = 1 << (field.number() % MASK_BITS);
    *byte = (*byte & !bit) | if matches!(value, Value::Null) { 0 } else { bit };
    if after.len() != length {
        return Err(EditError::Preservation);
    }
    Ok(after)
}
