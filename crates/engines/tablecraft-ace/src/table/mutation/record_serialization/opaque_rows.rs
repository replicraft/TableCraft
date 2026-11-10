//! A known LONG write can materialize a row without decoding its retired tail.
use crate::{
    EditError, InspectError, TableSchema, WriteBlocker,
    format::{
        bytes::u16_at,
        data_type::INT32,
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        page::PAGE_BYTES,
        row::{
            COLUMN_COUNT_BYTES, MASK_BITS, VARIABLE_COUNT_BYTES, VARIABLE_OFFSET_BYTES, slot_offset,
        },
    },
    header::check,
    table::{record_layout, row_layout},
};
use std::ops::Range;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub(in crate::table::mutation) fn is_ambiguous(
    schema: &TableSchema,
    before: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    check(budget, cancelled)?;
    budget
        .charge(
            Resource::Steps,
            schema.columns().len() as u64 + COLUMN_COUNT_BYTES as u64,
        )
        .map_err(InspectError::from)?;
    Ok(row_layout::geometry(schema, usize::from(u16_at(before, 0)?))?.ambiguous_tail())
}

struct Frame {
    prefix_end: usize,
    old_count: usize,
    selected: Range<usize>,
    expanded: usize,
    variables: usize,
    fields: usize,
    mask: usize,
    length: usize,
}

fn frame(
    schema: &TableSchema,
    column: usize,
    before: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Frame, EditError> {
    if !is_ambiguous(schema, before, budget, cancelled)? {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let field = schema.columns().get(column).ok_or(EditError::Conflict)?;
    if field.native_type() != INT32
        || !field.fixed()
        || field.storage_length() != size_of::<i32>()
        || field.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
        || field.extended_flags() != 0
    {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    let original = record_layout::parse_values(before, schema, budget)?;
    let start = COLUMN_COUNT_BYTES
        .checked_add(field.physical_field().offset)
        .ok_or(InspectError::StructureLimit)?;
    let end = start
        .checked_add(size_of::<i32>())
        .ok_or(InspectError::StructureLimit)?;
    if end > schema.physical_layout().fixed_end || !original.offsets.is_empty() {
        return Err(EditError::Preservation);
    }
    for (position, live) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if !live.fixed() && live.number() < original.count {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        if position != column && live.fixed() {
            let left = COLUMN_COUNT_BYTES
                .checked_add(live.physical_field().offset)
                .ok_or(InspectError::StructureLimit)?;
            let right = left
                .checked_add(live.storage_length())
                .ok_or(InspectError::StructureLimit)?;
            if start < right && left < end {
                return Err(EditError::Preservation);
            }
        }
    }
    let expanded = original.mask_start.max(schema.physical_layout().fixed_end);
    let variables = schema.physical_layout().variable_slots;
    let footer = if variables == 0 {
        0
    } else {
        variables
            .checked_add(1)
            .and_then(|words| words.checked_mul(VARIABLE_OFFSET_BYTES))
            .and_then(|bytes| bytes.checked_add(VARIABLE_COUNT_BYTES))
            .ok_or(InspectError::StructureLimit)?
    };
    let fields = schema.physical_layout().max_columns;
    let mask = expanded
        .checked_add(footer)
        .ok_or(InspectError::StructureLimit)?;
    let length = mask
        .checked_add(fields.div_ceil(MASK_BITS))
        .ok_or(InspectError::StructureLimit)?;
    // One native DATA record must fit beside its header and at least one slot.
    if length > PAGE_BYTES - slot_offset(1) {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    Ok(Frame {
        prefix_end: original.mask_start,
        old_count: original.count,
        selected: start..end,
        expanded,
        variables,
        fields,
        mask,
        length,
    })
}

pub(in crate::table::mutation) fn qualify(
    schema: &TableSchema,
    column: usize,
    before: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    frame(schema, column, before, budget, cancelled)?;
    Ok(())
}

pub(in crate::table::mutation) fn build(
    schema: &TableSchema,
    column: usize,
    before: &[u8],
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let frame = frame(schema, column, before, budget, cancelled)?;
    if !matches!(value, Value::Int32(_) | Value::Null) {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    budget
        .charge_allocation(frame.length as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, (frame.length + frame.prefix_end) as u64)
        .map_err(InspectError::from)?;
    let mut after = Vec::new();
    after
        .try_reserve_exact(frame.length)
        .map_err(|_| InspectError::Allocation)?;
    after.extend_from_slice(&before[..frame.prefix_end]);
    after.resize(frame.length, 0);
    // The prefix includes all opaque retired bytes, including any old footer.
    // Only the selected declared fixed slot and the logical count are overwritten.
    after[..COLUMN_COUNT_BYTES].copy_from_slice(
        &u16::try_from(frame.fields)
            .map_err(|_| InspectError::StructureLimit)?
            .to_le_bytes(),
    );
    if let Value::Int32(value) = value {
        after[frame.selected].copy_from_slice(&value.to_le_bytes());
    }
    if frame.variables != 0 {
        let count_at = frame.mask - VARIABLE_COUNT_BYTES;
        after[count_at..frame.mask].copy_from_slice(
            &u16::try_from(frame.variables)
                .map_err(|_| InspectError::StructureLimit)?
                .to_le_bytes(),
        );
        let offset = u16::try_from(frame.expanded)
            .map_err(|_| InspectError::StructureLimit)?
            .to_le_bytes();
        for slot in 0..=frame.variables {
            check(budget, cancelled)?;
            let at = count_at - (slot + 1) * VARIABLE_OFFSET_BYTES;
            after[at..at + VARIABLE_OFFSET_BYTES].copy_from_slice(&offset);
        }
    }
    let old_mask = &before[frame.prefix_end..];
    after[frame.mask..frame.mask + old_mask.len()].copy_from_slice(old_mask);
    for number in frame.old_count..frame.fields {
        check(budget, cancelled)?;
        after[frame.mask + number / MASK_BITS] &= !(1 << (number % MASK_BITS));
    }
    let number = schema.columns()[column].number();
    let bit = 1 << (number % MASK_BITS);
    after[frame.mask + number / MASK_BITS] = (after[frame.mask + number / MASK_BITS] & !bit)
        | if matches!(value, Value::Null) { 0 } else { bit };
    record_layout::parse(&after, schema, budget)?;
    Ok(after)
}
