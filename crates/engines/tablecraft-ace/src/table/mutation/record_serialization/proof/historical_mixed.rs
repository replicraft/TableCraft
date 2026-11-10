//! Independent append-based reconstruction of a mixed historical record.
use crate::{
    EditError, InspectError, TableSchema,
    format::{
        bytes::u16_at,
        row::{COLUMN_COUNT_BYTES, MASK_BITS, VARIABLE_COUNT_BYTES, VARIABLE_OFFSET_BYTES},
    },
    header::check,
    table::record_layout,
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
    let fields = schema.columns().len();
    let Value::Int32(value) = value else {
        return Err(EditError::Preservation);
    };
    let mut fixed = COLUMN_COUNT_BYTES;
    for field in schema.columns().iter().take(count) {
        check(budget, cancelled)?;
        if field.fixed() {
            let end = COLUMN_COUNT_BYTES
                .checked_add(field.field.offset)
                .and_then(|at| at.checked_add(field.storage_length()))
                .ok_or(InspectError::StructureLimit)?;
            fixed = fixed.max(end);
        }
    }
    let expanded = schema.layout.fixed_end;
    let delta = expanded.checked_sub(fixed).ok_or(EditError::Preservation)?;
    let variables = schema.layout.variable_slots;
    let data_end = original
        .data_end
        .checked_add(delta)
        .ok_or(InspectError::StructureLimit)?;
    let footer = (variables + 1)
        .checked_mul(VARIABLE_OFFSET_BYTES)
        .and_then(|bytes| bytes.checked_add(VARIABLE_COUNT_BYTES))
        .ok_or(InspectError::StructureLimit)?;
    let capacity = data_end
        .checked_add(footer)
        .and_then(|bytes| bytes.checked_add(fields.div_ceil(MASK_BITS)))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge_allocation(capacity as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, capacity as u64)
        .map_err(InspectError::from)?;
    let mut expected = Vec::new();
    expected
        .try_reserve_exact(capacity)
        .map_err(|_| InspectError::Allocation)?;
    expected.extend(
        u16::try_from(fields)
            .map_err(|_| InspectError::StructureLimit)?
            .to_le_bytes(),
    );
    expected.extend_from_slice(
        before
            .get(COLUMN_COUNT_BYTES..fixed)
            .ok_or(EditError::Preservation)?,
    );
    expected.resize(expanded, 0);
    let at = COLUMN_COUNT_BYTES + schema.columns()[column].field.offset;
    expected
        .get_mut(at..at + size_of::<i32>())
        .ok_or(EditError::Preservation)?
        .copy_from_slice(&value.to_le_bytes());
    expected.extend_from_slice(
        before
            .get(fixed..original.data_end)
            .ok_or(EditError::Preservation)?,
    );
    for position in (0..=variables).rev() {
        check(budget, cancelled)?;
        let previous = if original.offsets.is_empty() {
            fixed
        } else {
            original.offsets[position.min(original.offsets.len() - 1)]
        };
        let relocated = previous
            .checked_add(delta)
            .ok_or(InspectError::StructureLimit)?;
        expected.extend(
            u16::try_from(relocated)
                .map_err(|_| InspectError::StructureLimit)?
                .to_le_bytes(),
        );
    }
    expected.extend(
        u16::try_from(variables)
            .map_err(|_| InspectError::StructureLimit)?
            .to_le_bytes(),
    );
    let selected = schema.columns()[column].number();
    let old_mask = &before[original.mask_start..];
    for byte in 0..fields.div_ceil(MASK_BITS) {
        check(budget, cancelled)?;
        let mut bits = if byte < old_mask.len() {
            old_mask[byte]
        } else {
            0
        };
        for bit in 0..MASK_BITS {
            let number = byte * MASK_BITS + bit;
            if (count..fields).contains(&number) {
                bits &= !(1 << bit);
                if number == selected {
                    bits |= 1 << bit;
                }
            }
        }
        expected.push(bits);
    }
    if expected.len() != capacity {
        return Err(EditError::Preservation);
    }
    Ok(expected)
}
