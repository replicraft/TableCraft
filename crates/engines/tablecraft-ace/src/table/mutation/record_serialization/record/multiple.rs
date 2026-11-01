//! Compose all supplied variable payloads from one immutable original record.
use super::super::super::{EditError, WriteBlocker};
use crate::{
    InspectError, TableSchema,
    format::{
        page::PAGE_BYTES,
        row::{MASK_BITS, VARIABLE_COUNT_BYTES, VARIABLE_OFFSET_BYTES, slot_offset},
    },
    header::check,
    table::record_layout,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::table::mutation) struct Payload {
    pub column: usize,
    pub bytes: Vec<u8>,
    pub present: bool,
}

pub(in crate::table::mutation) fn compose(
    schema: &TableSchema,
    before: &[u8],
    payloads: &[Payload],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    check(budget, cancelled)?;
    let layout = record_layout::parse(before, schema, budget)?;
    if layout.count != schema.layout.max_columns {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let variables = layout
        .offsets
        .len()
        .checked_sub(1)
        .ok_or(InspectError::Malformed)?;
    let start = *layout.offsets.first().ok_or(InspectError::Malformed)?;
    let mut previous = None;
    for payload in payloads {
        check(budget, cancelled)?;
        let field = schema
            .columns()
            .get(payload.column)
            .ok_or(InspectError::Malformed)?;
        if field.fixed()
            || field.field.variable >= variables
            || previous.is_some_and(|prior| prior >= field.field.variable)
        {
            return Err(EditError::Preservation);
        }
        previous = Some(field.field.variable);
    }

    budget
        .charge_allocation((variables + 1) as u64, size_of::<usize>() as u64)
        .map_err(InspectError::from)?;
    let mut offsets = Vec::new();
    offsets
        .try_reserve_exact(variables + 1)
        .map_err(|_| InspectError::Allocation)?;
    offsets.push(start);
    let mut end = start;
    let mut selected = 0;
    for variable in 0..variables {
        check(budget, cancelled)?;
        let length = if payloads
            .get(selected)
            .is_some_and(|payload| schema.columns()[payload.column].field.variable == variable)
        {
            let length = payloads[selected].bytes.len();
            selected += 1;
            length
        } else {
            layout.offsets[variable + 1]
                .checked_sub(layout.offsets[variable])
                .ok_or(InspectError::Malformed)?
        };
        end = end
            .checked_add(length)
            .ok_or(InspectError::StructureLimit)?;
        offsets.push(end);
    }
    if selected != payloads.len() {
        return Err(EditError::Preservation);
    }
    let footer = before
        .len()
        .checked_sub(layout.data_end)
        .ok_or(InspectError::Malformed)?;
    let length = before.len().max(
        end.checked_add(footer)
            .ok_or(InspectError::StructureLimit)?,
    );
    if length > PAGE_BYTES - slot_offset(1) {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
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
    after.extend_from_slice(before);
    after.resize(length, 0);

    selected = 0;
    for variable in 0..variables {
        check(budget, cancelled)?;
        let bytes = if payloads
            .get(selected)
            .is_some_and(|payload| schema.columns()[payload.column].field.variable == variable)
        {
            let bytes = payloads[selected].bytes.as_slice();
            selected += 1;
            bytes
        } else {
            &before[layout.offsets[variable]..layout.offsets[variable + 1]]
        };
        budget
            .charge(Resource::Steps, bytes.len() as u64)
            .map_err(InspectError::from)?;
        after[offsets[variable]..offsets[variable + 1]].copy_from_slice(bytes);
    }
    let mask = length
        .checked_sub(layout.count.div_ceil(MASK_BITS))
        .ok_or(InspectError::Malformed)?;
    budget
        .charge(Resource::Steps, (before.len() - layout.mask_start) as u64)
        .map_err(InspectError::from)?;
    after[mask..].copy_from_slice(&before[layout.mask_start..]);
    for payload in payloads {
        check(budget, cancelled)?;
        let number = schema.columns()[payload.column].number();
        let bit = 1 << (number % MASK_BITS);
        let byte = &mut after[mask + number / MASK_BITS];
        *byte = (*byte & !bit) | if payload.present { bit } else { 0 };
    }
    let count_at = mask
        .checked_sub(VARIABLE_COUNT_BYTES)
        .ok_or(InspectError::Malformed)?;
    after[count_at..count_at + VARIABLE_COUNT_BYTES].copy_from_slice(
        &u16::try_from(variables)
            .map_err(|_| InspectError::StructureLimit)?
            .to_le_bytes(),
    );
    for (position, offset) in offsets.into_iter().enumerate() {
        check(budget, cancelled)?;
        let at = count_at
            .checked_sub((position + 1) * VARIABLE_OFFSET_BYTES)
            .ok_or(InspectError::Malformed)?;
        after[at..at + VARIABLE_OFFSET_BYTES].copy_from_slice(
            &u16::try_from(offset)
                .map_err(|_| InspectError::StructureLimit)?
                .to_le_bytes(),
        );
    }
    record_layout::parse(&after, schema, budget)?;
    Ok(after)
}
