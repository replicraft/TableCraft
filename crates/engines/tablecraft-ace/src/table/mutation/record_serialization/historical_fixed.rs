//! Explicit absent-LONG materialization; stored defaults never supply values.
use crate::{
    EditError, InspectError, TableSchema, WriteBlocker,
    format::{
        data_type::INT32,
        row::{COLUMN_COUNT_BYTES, MASK_BITS},
    },
    header::check,
    table::record_layout,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const LONG_BYTES: usize = size_of::<i32>();

/// Validate only the dense fixed-LONG physical framing owned by this encoder.
/// Program, property, index and assignment permission remain with the planner.
pub(in crate::table::mutation) fn qualify(
    schema: &TableSchema,
    column: usize,
    before: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    check(budget, cancelled)?;
    if schema.columns().len() != schema.layout.max_columns || schema.layout.variable_slots != 0 {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    for (number, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if field.number() != number
            || field.native_type() != INT32
            || !field.fixed()
            || field.field.offset != number * LONG_BYTES
            || field.storage_length() != LONG_BYTES
        {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
    }
    let layout = record_layout::parse(before, schema, budget)?;
    let count = layout.count;
    let selected = schema.columns().get(column).ok_or(EditError::Conflict)?;
    let expected = COLUMN_COUNT_BYTES
        .checked_add(
            count
                .checked_mul(LONG_BYTES)
                .ok_or(InspectError::StructureLimit)?,
        )
        .and_then(|bytes| bytes.checked_add(count.div_ceil(MASK_BITS)))
        .ok_or(InspectError::StructureLimit)?;
    if count == 0
        || count >= schema.layout.max_columns
        || selected.number() < count
        || before.len() != expected
        || !layout.offsets.is_empty()
    {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    Ok(count)
}

pub(in crate::table::mutation) fn build(
    schema: &TableSchema,
    column: usize,
    before: &[u8],
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let count = qualify(schema, column, before, budget, cancelled)?;
    let Value::Int32(value) = value else {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    };
    let fields = schema.layout.max_columns;
    let header = u16::try_from(fields).map_err(|_| InspectError::StructureLimit)?;
    let mask = COLUMN_COUNT_BYTES
        .checked_add(
            fields
                .checked_mul(LONG_BYTES)
                .ok_or(InspectError::StructureLimit)?,
        )
        .ok_or(InspectError::StructureLimit)?;
    let bytes = mask
        .checked_add(fields.div_ceil(MASK_BITS))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge_allocation(bytes as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, bytes as u64)
        .map_err(InspectError::from)?;
    let mut after = Vec::new();
    after
        .try_reserve_exact(bytes)
        .map_err(|_| InspectError::Allocation)?;
    after.resize(bytes, 0);
    after[..COLUMN_COUNT_BYTES].copy_from_slice(&header.to_le_bytes());
    let old_end = COLUMN_COUNT_BYTES + count * LONG_BYTES;
    after[COLUMN_COUNT_BYTES..old_end].copy_from_slice(&before[COLUMN_COUNT_BYTES..old_end]);
    let old_mask = count.div_ceil(MASK_BITS);
    after[mask..mask + old_mask].copy_from_slice(&before[before.len() - old_mask..]);
    for number in count..fields {
        check(budget, cancelled)?;
        after[mask + number / MASK_BITS] &= !(1 << (number % MASK_BITS));
    }
    let number = schema.columns()[column].number();
    let at = COLUMN_COUNT_BYTES + number * LONG_BYTES;
    after[at..at + LONG_BYTES].copy_from_slice(&value.to_le_bytes());
    after[mask + number / MASK_BITS] |= 1 << (number % MASK_BITS);
    record_layout::parse(&after, schema, budget)?;
    Ok(after)
}
