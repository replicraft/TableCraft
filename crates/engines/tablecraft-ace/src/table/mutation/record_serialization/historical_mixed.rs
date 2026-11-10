//! Absent LONG materialization with byte-preserving variable-field relocation.
use crate::{
    EditError, InspectError, TableSchema, WriteBlocker,
    format::{
        data_type::{INT32, SHORT_BINARY, SHORT_TEXT},
        layout::{UNICODE_COMPRESSION_FLAG, UPDATABLE_FLAG},
        row::{COLUMN_COUNT_BYTES, MASK_BITS, VARIABLE_COUNT_BYTES, VARIABLE_OFFSET_BYTES},
    },
    header::check,
    table::record_layout::{self, RecordLayout},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub(in crate::table::mutation) struct Frame {
    pub(super) count: usize,
    pub(super) fixed: usize,
    pub(super) expanded: usize,
    pub(super) variables: usize,
    pub(super) original: RecordLayout,
}

pub(super) fn frame(
    schema: &TableSchema,
    before: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Frame, EditError> {
    check(budget, cancelled)?;
    let original = record_layout::parse(before, schema, budget)?;
    let count = original.count;
    if count == 0
        || count >= schema.layout.max_columns
        || schema.columns().len() != schema.layout.max_columns
        || schema.layout.variable_slots == 0
    {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let mut fixed = COLUMN_COUNT_BYTES;
    let mut expanded = COLUMN_COUNT_BYTES;
    let mut variables = 0;
    let mut old_variables = 0;
    for (number, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if field.number() != number {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        if field.fixed() {
            if field.scalar_type().is_none() || field.storage_length() == 0 {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
            let start = COLUMN_COUNT_BYTES
                .checked_add(field.field.offset)
                .ok_or(InspectError::StructureLimit)?;
            let end = start
                .checked_add(field.storage_length())
                .ok_or(InspectError::StructureLimit)?;
            for prior in &schema.columns()[..number] {
                check(budget, cancelled)?;
                if prior.fixed() {
                    let other = COLUMN_COUNT_BYTES
                        .checked_add(prior.field.offset)
                        .ok_or(InspectError::StructureLimit)?;
                    let stop = other
                        .checked_add(prior.storage_length())
                        .ok_or(InspectError::StructureLimit)?;
                    if start < stop && other < end {
                        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                    }
                }
            }
            expanded = expanded.max(end);
            if number < count {
                fixed = fixed.max(end);
            }
        } else {
            if !matches!(field.native_type(), SHORT_TEXT | SHORT_BINARY)
                || field.flags() != UPDATABLE_FLAG
                || field.field.variable != variables
                || (field.native_type() == SHORT_BINARY && field.extended_flags() != 0)
                || (field.native_type() == SHORT_TEXT
                    && field.extended_flags() & !UNICODE_COMPRESSION_FLAG != 0)
            {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
            variables += 1;
            old_variables += usize::from(number < count);
        }
    }
    for field in schema.columns().iter().skip(count) {
        check(budget, cancelled)?;
        if field.fixed() && COLUMN_COUNT_BYTES + field.field.offset < fixed {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
    }
    let offsets = if old_variables == 0 {
        0
    } else {
        old_variables + 1
    };
    if original.offsets.len() != offsets
        || variables != schema.layout.variable_slots
        || fixed > original.data_end
        || expanded != schema.layout.fixed_end
    {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    Ok(Frame {
        count,
        fixed,
        expanded,
        variables,
        original,
    })
}

pub(in crate::table::mutation) fn qualify(
    schema: &TableSchema,
    column: usize,
    before: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Frame, EditError> {
    let frame = frame(schema, before, budget, cancelled)?;
    let count = frame.count;
    let selected = schema.columns().get(column).ok_or(EditError::Conflict)?;
    if selected.number() < count
        || !selected.fixed()
        || selected.native_type() != INT32
        || selected.storage_length() != size_of::<i32>()
    {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    if frame.expanded <= frame.fixed {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
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
    let Value::Int32(value) = value else {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    };
    let fields = schema.layout.max_columns;
    let delta = frame.expanded - frame.fixed;
    let data_end = frame
        .original
        .data_end
        .checked_add(delta)
        .ok_or(InspectError::StructureLimit)?;
    let footer = (frame.variables + 1)
        .checked_mul(VARIABLE_OFFSET_BYTES)
        .and_then(|bytes| bytes.checked_add(VARIABLE_COUNT_BYTES))
        .ok_or(InspectError::StructureLimit)?;
    let mask = data_end
        .checked_add(footer)
        .ok_or(InspectError::StructureLimit)?;
    let length = mask
        .checked_add(fields.div_ceil(MASK_BITS))
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
    after.resize(length, 0);
    let count = u16::try_from(fields).map_err(|_| InspectError::StructureLimit)?;
    after[..COLUMN_COUNT_BYTES].copy_from_slice(&count.to_le_bytes());
    after[COLUMN_COUNT_BYTES..frame.fixed]
        .copy_from_slice(&before[COLUMN_COUNT_BYTES..frame.fixed]);
    after[frame.expanded..data_end].copy_from_slice(&before[frame.fixed..frame.original.data_end]);
    let at = COLUMN_COUNT_BYTES + schema.columns()[column].field.offset;
    after[at..at + size_of::<i32>()].copy_from_slice(&value.to_le_bytes());
    let count_at = mask - VARIABLE_COUNT_BYTES;
    let variables = u16::try_from(frame.variables).map_err(|_| InspectError::StructureLimit)?;
    after[count_at..mask].copy_from_slice(&variables.to_le_bytes());
    for position in 0..=frame.variables {
        check(budget, cancelled)?;
        let original = if frame.original.offsets.is_empty() {
            frame.fixed
        } else if let Some(offset) = frame.original.offsets.get(position) {
            *offset
        } else {
            *frame
                .original
                .offsets
                .last()
                .ok_or(EditError::Preservation)?
        };
        let offset = u16::try_from(
            original
                .checked_add(delta)
                .ok_or(InspectError::StructureLimit)?,
        )
        .map_err(|_| InspectError::StructureLimit)?;
        let output = count_at - VARIABLE_OFFSET_BYTES * (position + 1);
        after[output..output + VARIABLE_OFFSET_BYTES].copy_from_slice(&offset.to_le_bytes());
    }
    let old_mask = &before[frame.original.mask_start..];
    after[mask..mask + old_mask.len()].copy_from_slice(old_mask);
    for number in frame.count..fields {
        check(budget, cancelled)?;
        after[mask + number / MASK_BITS] &= !(1 << (number % MASK_BITS));
    }
    let number = schema.columns()[column].number();
    after[mask + number / MASK_BITS] |= 1 << (number % MASK_BITS);
    record_layout::parse(&after, schema, budget)?;
    Ok(after)
}
