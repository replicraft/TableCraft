//! Independent append reconstruction; existing payload and opaque padding survive.
use crate::{
    EditError, InspectError, TableSchema,
    format::{
        bytes::u16_at,
        row::{COLUMN_COUNT_BYTES, MASK_BITS, VARIABLE_COUNT_BYTES, VARIABLE_OFFSET_BYTES},
    },
    header::check,
    table::{mutation::record_serialization, record_layout},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub(in crate::table::mutation) fn reconstruct(
    schema: &TableSchema,
    column: usize,
    before: &[u8],
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let original = record_layout::parse(before, schema, budget)?;
    let count = usize::from(u16_at(before, 0)?);
    let selected = schema
        .columns()
        .get(column)
        .ok_or(EditError::Preservation)?;
    if selected.number() < count {
        return reconstruct_present(schema, column, before, value, budget, cancelled);
    }
    if matches!(value, Value::Null) {
        return record_serialization::copy(before, budget, cancelled);
    }
    let payload = super::payload::encode(selected, value, budget, cancelled)?;
    let mut fixed = COLUMN_COUNT_BYTES;
    for field in schema.columns().iter().take(count) {
        if field.fixed() {
            fixed = fixed.max(
                COLUMN_COUNT_BYTES
                    .checked_add(field.field.offset)
                    .and_then(|at| at.checked_add(field.storage_length()))
                    .ok_or(InspectError::StructureLimit)?,
            );
        }
    }
    let expanded = schema.layout.fixed_end;
    let gap = expanded.checked_sub(fixed).ok_or(EditError::Preservation)?;
    let old_variables = original.offsets.len().saturating_sub(1);
    let old_end = original.offsets.last().copied().unwrap_or(fixed);
    let variables = schema.layout.variable_slots;
    let fields = schema.columns().len();
    let length = original
        .data_end
        .checked_add(gap)
        .and_then(|bytes| bytes.checked_add(payload.len()))
        .and_then(|bytes| bytes.checked_add((variables + 1).checked_mul(VARIABLE_OFFSET_BYTES)?))
        .and_then(|bytes| bytes.checked_add(VARIABLE_COUNT_BYTES))
        .and_then(|bytes| bytes.checked_add(fields.div_ceil(MASK_BITS)))
        .ok_or(InspectError::StructureLimit)?;
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
    expected[..COLUMN_COUNT_BYTES].copy_from_slice(
        &u16::try_from(fields)
            .map_err(|_| InspectError::StructureLimit)?
            .to_le_bytes(),
    );
    expected[COLUMN_COUNT_BYTES..fixed].copy_from_slice(&before[COLUMN_COUNT_BYTES..fixed]);
    expected[expanded..old_end + gap].copy_from_slice(&before[fixed..old_end]);
    let start = old_end + gap;
    expected[start..start + payload.len()].copy_from_slice(&payload);
    let data_end = original.data_end + gap + payload.len();
    expected[start + payload.len()..data_end].copy_from_slice(&before[old_end..original.data_end]);
    for slot in 0..=variables {
        check(budget, cancelled)?;
        let offset = if slot <= old_variables {
            original.offsets.get(slot).copied().unwrap_or(fixed) + gap
        } else if slot <= selected.field.variable {
            start
        } else {
            start + payload.len()
        };
        let at = data_end + (variables - slot) * VARIABLE_OFFSET_BYTES;
        expected[at..at + VARIABLE_OFFSET_BYTES].copy_from_slice(
            &u16::try_from(offset)
                .map_err(|_| InspectError::StructureLimit)?
                .to_le_bytes(),
        );
    }
    let count_at = data_end + (variables + 1) * VARIABLE_OFFSET_BYTES;
    expected[count_at..count_at + VARIABLE_COUNT_BYTES].copy_from_slice(
        &u16::try_from(variables)
            .map_err(|_| InspectError::StructureLimit)?
            .to_le_bytes(),
    );
    let mask_at = count_at + VARIABLE_COUNT_BYTES;
    for number in 0..fields.div_ceil(MASK_BITS) * MASK_BITS {
        check(budget, cancelled)?;
        let old = before
            .get(original.mask_start + number / MASK_BITS)
            .copied()
            .unwrap_or(0)
            & (1 << (number % MASK_BITS))
            != 0;
        let present = if number >= count && number < fields {
            number == column
        } else {
            old
        };
        if present {
            expected[mask_at + number / MASK_BITS] |= 1 << (number % MASK_BITS);
        }
    }
    Ok(expected)
}

fn reconstruct_present(
    schema: &TableSchema,
    column: usize,
    before: &[u8],
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let original = record_layout::parse(before, schema, budget)?;
    let field = schema
        .columns()
        .get(column)
        .ok_or(EditError::Preservation)?;
    let variable = field.physical_field().variable;
    let begin = *original
        .offsets
        .get(variable)
        .ok_or(EditError::Preservation)?;
    let finish = *original
        .offsets
        .get(variable + 1)
        .ok_or(EditError::Preservation)?;
    let payload_end = *original.offsets.last().ok_or(EditError::Preservation)?;
    let payload = super::payload::encode(field, value, budget, cancelled)?;
    let removed = finish.checked_sub(begin).ok_or(EditError::Preservation)?;
    let new_data_end = original
        .data_end
        .checked_sub(removed)
        .and_then(|end| end.checked_add(payload.len()))
        .ok_or(InspectError::StructureLimit)?
        .max(original.data_end);
    let tail = before
        .len()
        .checked_sub(original.data_end)
        .ok_or(EditError::Preservation)?;
    let length = new_data_end
        .checked_add(tail)
        .ok_or(InspectError::StructureLimit)?;
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
    expected[..begin].copy_from_slice(&before[..begin]);
    let replacement_end = begin + payload.len();
    expected[begin..replacement_end].copy_from_slice(&payload);
    let siblings_end = replacement_end + payload_end - finish;
    expected[replacement_end..siblings_end].copy_from_slice(&before[finish..payload_end]);
    let padding_end = siblings_end + original.data_end - payload_end;
    expected[siblings_end..padding_end].copy_from_slice(&before[payload_end..original.data_end]);
    let variables = original.offsets.len() - 1;
    for (slot, old_offset) in original.offsets.iter().enumerate() {
        check(budget, cancelled)?;
        let shifted = if slot > variable {
            old_offset
                .checked_sub(removed)
                .and_then(|at| at.checked_add(payload.len()))
                .ok_or(InspectError::StructureLimit)?
        } else {
            *old_offset
        };
        let at = new_data_end + (variables - slot) * VARIABLE_OFFSET_BYTES;
        expected[at..at + VARIABLE_OFFSET_BYTES].copy_from_slice(
            &u16::try_from(shifted)
                .map_err(|_| InspectError::StructureLimit)?
                .to_le_bytes(),
        );
    }
    let count_at = new_data_end + (variables + 1) * VARIABLE_OFFSET_BYTES;
    expected[count_at..count_at + VARIABLE_COUNT_BYTES].copy_from_slice(
        &u16::try_from(variables)
            .map_err(|_| InspectError::StructureLimit)?
            .to_le_bytes(),
    );
    let mask_at = count_at + VARIABLE_COUNT_BYTES;
    expected[mask_at..].copy_from_slice(&before[original.mask_start..]);
    let bit = 1 << (field.number() % MASK_BITS);
    let selected = expected
        .get_mut(mask_at + field.number() / MASK_BITS)
        .ok_or(EditError::Preservation)?;
    *selected = (*selected & !bit) | if matches!(value, Value::Null) { 0 } else { bit };
    Ok(expected)
}
