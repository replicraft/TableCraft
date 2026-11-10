//! Current logical identities retain opaque retired fixed and variable payloads.
use super::storage;
use crate::{
    EditError, InspectError, TableSchema, WriteBlocker,
    format::{
        data_type::{CURRENCY, INT32, SHORT_TEXT},
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        row::{COLUMN_COUNT_BYTES, MASK_BITS, VARIABLE_COUNT_BYTES, VARIABLE_OFFSET_BYTES},
    },
    header::check,
    table::record_layout::{self, RecordLayout},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

/// Each consuming action keeps its original declared scalar language closed.
#[derive(Clone, Copy)]
pub(in crate::table::mutation) enum Language {
    LongText,
    Currency,
}

pub(in crate::table::mutation) struct Frame {
    original: RecordLayout,
    fixed: usize,
    expanded: usize,
    variables: usize,
}

pub(in crate::table::mutation) fn qualify(
    schema: &TableSchema,
    column: usize,
    before: &[u8],
    language: Language,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Frame, EditError> {
    check(budget, cancelled)?;
    let field = schema.columns().get(column).ok_or(EditError::Conflict)?;
    match (language, field.native_type(), field.fixed()) {
        (Language::Currency, CURRENCY, true)
            if field.flags() == (FIXED_FLAG | UPDATABLE_FLAG)
                && field.extended_flags() == 0
                && field.storage_length() == size_of::<i64>() => {}
        (Language::LongText, INT32, true)
            if field.flags() == (FIXED_FLAG | UPDATABLE_FLAG)
                && field.extended_flags() == 0
                && field.storage_length() == size_of::<i32>() => {}
        (Language::LongText, SHORT_TEXT, false) => storage::qualify(field)?,
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    }
    let original = record_layout::parse(before, schema, budget)?;
    let variables = schema.physical_layout().variable_slots;
    let old_variables = original.offsets.len().saturating_sub(1);
    if old_variables > variables || original.count > schema.physical_layout().max_columns {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let fixed = original
        .offsets
        .first()
        .copied()
        .unwrap_or(original.data_end);
    let expanded = fixed.max(schema.physical_layout().fixed_end);
    if fixed < COLUMN_COUNT_BYTES || fixed > original.data_end {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    for live in schema.columns() {
        check(budget, cancelled)?;
        if !live.fixed() && live.physical_field().variable >= variables {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
    }
    Ok(Frame {
        original,
        fixed,
        expanded,
        variables,
    })
}

pub(in crate::table::mutation) fn build(
    schema: &TableSchema,
    column: usize,
    before: &[u8],
    value: &Value,
    language: Language,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let frame = qualify(schema, column, before, language, budget, cancelled)?;
    let field = &schema.columns()[column];
    let variable = (!field.fixed()).then_some(field.physical_field().variable);
    let payload = match (language, field.native_type(), value) {
        (Language::LongText, INT32, Value::Int32(_) | Value::Null)
        | (Language::Currency, CURRENCY, Value::Currency(_) | Value::Null) => Vec::new(),
        (Language::LongText, SHORT_TEXT, Value::Text(_) | Value::Null) => {
            storage::encode_preferred(field, value, budget, cancelled)?
        }
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    };
    let old_end = frame
        .original
        .offsets
        .last()
        .copied()
        .unwrap_or(frame.fixed);
    let removed = variable
        .and_then(|slot| {
            frame
                .original
                .offsets
                .get(slot)
                .zip(frame.original.offsets.get(slot + 1))
        })
        .map_or(0, |(start, end)| end - start);
    let data_end = frame
        .original
        .data_end
        .checked_add(frame.expanded - frame.fixed)
        .and_then(|bytes| bytes.checked_sub(removed))
        .and_then(|bytes| bytes.checked_add(payload.len()))
        .ok_or(InspectError::StructureLimit)?
        .max(frame.original.data_end);
    let footer = if frame.variables == 0 {
        0
    } else {
        (frame.variables + 1)
            .checked_mul(VARIABLE_OFFSET_BYTES)
            .and_then(|bytes| bytes.checked_add(VARIABLE_COUNT_BYTES))
            .ok_or(InspectError::StructureLimit)?
    };
    let mask = data_end
        .checked_add(footer)
        .ok_or(InspectError::StructureLimit)?;
    let fields = schema.physical_layout().max_columns;
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
    let start = COLUMN_COUNT_BYTES + field.physical_field().offset;
    match value {
        Value::Int32(value) => after
            .get_mut(start..start + size_of::<i32>())
            .ok_or(EditError::Preservation)?
            .copy_from_slice(&value.to_le_bytes()),
        Value::Currency(_) => {
            let encoded = crate::table::scalar_encoding::encode::<{ size_of::<i64>() }>(
                tablecraft_schema::ScalarType::Currency,
                value,
            )
            .ok_or(EditError::Preservation)?;
            after
                .get_mut(start..start + encoded.len())
                .ok_or(EditError::Preservation)?
                .copy_from_slice(&encoded);
        }
        _ => {}
    }
    let mut position = frame.expanded;
    if frame.variables != 0 {
        let count_at = mask - VARIABLE_COUNT_BYTES;
        let variables = u16::try_from(frame.variables).map_err(|_| InspectError::StructureLimit)?;
        after[count_at..mask].copy_from_slice(&variables.to_le_bytes());
        for slot in 0..=frame.variables {
            check(budget, cancelled)?;
            let offset = u16::try_from(position).map_err(|_| InspectError::StructureLimit)?;
            let at = count_at - VARIABLE_OFFSET_BYTES * (slot + 1);
            after[at..at + VARIABLE_OFFSET_BYTES].copy_from_slice(&offset.to_le_bytes());
            if slot == frame.variables {
                break;
            }
            let bytes = if variable == Some(slot) {
                payload.as_slice()
            } else {
                match frame
                    .original
                    .offsets
                    .get(slot)
                    .zip(frame.original.offsets.get(slot + 1))
                {
                    Some((start, end)) => &before[*start..*end],
                    None => &[],
                }
            };
            let end = position
                .checked_add(bytes.len())
                .ok_or(InspectError::StructureLimit)?;
            after
                .get_mut(position..end)
                .ok_or(EditError::Preservation)?
                .copy_from_slice(bytes);
            position = end;
        }
    }
    let padding = &before[old_end..frame.original.data_end];
    let padding_end = position
        .checked_add(padding.len())
        .ok_or(InspectError::StructureLimit)?;
    if padding_end > data_end {
        return Err(EditError::Preservation);
    }
    // Retain the old physical footprint on shrink. Original padding is copied;
    // newly unused payload space remains explicit zero slack before the footer.
    after[position..padding_end].copy_from_slice(padding);
    let old_mask = &before[frame.original.mask_start..];
    after
        .get_mut(mask..mask + old_mask.len())
        .ok_or(EditError::Preservation)?
        .copy_from_slice(old_mask);
    for number in frame.original.count..fields {
        check(budget, cancelled)?;
        after[mask + number / MASK_BITS] &= !(1 << (number % MASK_BITS));
    }
    let number = field.number();
    let bit = 1 << (number % MASK_BITS);
    let byte = after
        .get_mut(mask + number / MASK_BITS)
        .ok_or(EditError::Preservation)?;
    *byte = (*byte & !bit) | if matches!(value, Value::Null) { 0 } else { bit };
    record_layout::parse(&after, schema, budget)?;
    Ok(after)
}
