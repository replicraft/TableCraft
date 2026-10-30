//! Immutable requested serialization and complete historical record framing.
mod payload;
#[cfg(test)]
mod tests;

use super::super::{EditError, WriteBlocker};
use crate::{
    InspectError, TableSchema,
    format::row::{MASK_BITS, VARIABLE_COUNT_BYTES, VARIABLE_OFFSET_BYTES},
    header::check,
    table::record_layout,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const COPY_BYTES: usize = 256;

pub(in crate::table::mutation) fn reconstruct(
    schema: &TableSchema,
    column: usize,
    before: &[u8],
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    reconstruct_with(
        schema,
        column,
        before,
        value,
        PayloadEncoding::MinimumSize,
        budget,
        cancelled,
    )
}

/// Independently reconstruct mixed records using the declared compression
/// preference. Existing variable-update proofs retain their minimum-size policy.
pub(in crate::table::mutation) fn reconstruct_declared(
    schema: &TableSchema,
    column: usize,
    before: &[u8],
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    reconstruct_with(
        schema,
        column,
        before,
        value,
        PayloadEncoding::DeclaredPreference,
        budget,
        cancelled,
    )
}

enum PayloadEncoding {
    MinimumSize,
    DeclaredPreference,
}

fn reconstruct_with(
    schema: &TableSchema,
    column: usize,
    before: &[u8],
    value: &Value,
    encoding: PayloadEncoding,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    check(budget, cancelled)?;
    let layout = record_layout::parse(before, schema, budget)?;
    let field = schema.columns.get(column).ok_or(EditError::Preservation)?;
    if layout.count != schema.layout.max_columns || field.fixed() {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let variable = field.field.variable;
    let start = *layout
        .offsets
        .get(variable)
        .ok_or(EditError::Preservation)?;
    let stop = *layout
        .offsets
        .get(variable + 1)
        .ok_or(EditError::Preservation)?;
    let end = *layout.offsets.last().ok_or(EditError::Preservation)?;
    let footer = before
        .len()
        .checked_sub(layout.data_end)
        .ok_or(EditError::Preservation)?;
    let span = stop.checked_sub(start).ok_or(EditError::Preservation)?;
    let payload = match encoding {
        PayloadEncoding::MinimumSize => payload::encode(field, value, budget, cancelled)?,
        PayloadEncoding::DeclaredPreference => {
            let preserved_payload = end.checked_sub(span).ok_or(EditError::Preservation)?;
            let capacity = layout
                .data_end
                .checked_sub(preserved_payload)
                .ok_or(EditError::Preservation)?;
            payload::encode_declared(field, value, capacity, budget, cancelled)?
        }
    };
    let logical_end = end
        .checked_sub(span)
        .and_then(|end| end.checked_add(payload.len()))
        .ok_or(EditError::Preservation)?;
    let length = before.len().max(
        logical_end
            .checked_add(footer)
            .ok_or(EditError::Preservation)?,
    );

    budget
        .charge_allocation(length as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, length as u64)
        .map_err(InspectError::from)?;
    let mut expected = Vec::new();
    expected
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    expected.resize(length, 0);
    copy(before, &mut expected[..before.len()], budget, cancelled)?;
    let payload_end = start
        .checked_add(payload.len())
        .ok_or(EditError::Preservation)?;
    copy(
        &payload,
        expected
            .get_mut(start..payload_end)
            .ok_or(EditError::Preservation)?,
        budget,
        cancelled,
    )?;
    copy(
        &before[stop..end],
        expected
            .get_mut(payload_end..logical_end)
            .ok_or(EditError::Preservation)?,
        budget,
        cancelled,
    )?;

    let mask_length = layout.count.div_ceil(MASK_BITS);
    let mask = length
        .checked_sub(mask_length)
        .ok_or(EditError::Preservation)?;
    copy(
        &before[layout.mask_start..],
        &mut expected[mask..],
        budget,
        cancelled,
    )?;
    let bit = 1_u8 << (field.number() % MASK_BITS);
    let selected_mask = expected
        .get_mut(mask + field.number() / MASK_BITS)
        .ok_or(EditError::Preservation)?;
    if matches!(value, Value::Null) {
        *selected_mask &= !bit;
    } else {
        *selected_mask |= bit;
    }

    let count_at = mask
        .checked_sub(VARIABLE_COUNT_BYTES)
        .ok_or(EditError::Preservation)?;
    let variables = layout
        .offsets
        .len()
        .checked_sub(1)
        .ok_or(EditError::Preservation)?;
    let encoded = u16::try_from(variables).map_err(|_| EditError::Preservation)?;
    expected[count_at..mask].copy_from_slice(&encoded.to_le_bytes());
    for (position, &offset) in layout.offsets.iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, 1)
            .map_err(InspectError::from)?;
        let offset = if position <= variable {
            offset
        } else {
            offset
                .checked_sub(span)
                .and_then(|offset| offset.checked_add(payload.len()))
                .ok_or(EditError::Preservation)?
        };
        let offset = u16::try_from(offset).map_err(|_| EditError::Preservation)?;
        let at = count_at
            .checked_sub((position + 1) * VARIABLE_OFFSET_BYTES)
            .ok_or(EditError::Preservation)?;
        expected[at..at + VARIABLE_OFFSET_BYTES].copy_from_slice(&offset.to_le_bytes());
    }
    record_layout::parse(&expected, schema, budget)?;
    Ok(expected)
}

pub(in crate::table::mutation) fn equal(
    expected: &[u8],
    actual: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    if expected.len() != actual.len() {
        return Err(EditError::Preservation);
    }
    budget
        .charge(Resource::Steps, expected.len() as u64)
        .map_err(InspectError::from)?;
    for (expected, actual) in expected.chunks(COPY_BYTES).zip(actual.chunks(COPY_BYTES)) {
        check(budget, cancelled)?;
        if expected != actual {
            return Err(EditError::Preservation);
        }
    }
    Ok(())
}

fn copy(
    source: &[u8],
    target: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    if source.len() != target.len() {
        return Err(EditError::Preservation);
    }
    budget
        .charge(Resource::Steps, source.len() as u64)
        .map_err(InspectError::from)?;
    for (source, target) in source.chunks(COPY_BYTES).zip(target.chunks_mut(COPY_BYTES)) {
        check(budget, cancelled)?;
        target.copy_from_slice(source);
    }
    Ok(())
}
