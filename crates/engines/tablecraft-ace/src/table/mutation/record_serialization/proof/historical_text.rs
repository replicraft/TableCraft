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
    if matches!(value, Value::Null) {
        return record_serialization::copy(before, budget, cancelled);
    }
    let count = usize::from(u16_at(before, 0)?);
    let selected = schema
        .columns()
        .get(column)
        .ok_or(EditError::Preservation)?;
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
