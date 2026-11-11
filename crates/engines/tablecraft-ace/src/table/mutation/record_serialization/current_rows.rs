//! Current logical identities retain opaque retired fixed and variable payloads.
use super::storage;
use crate::{
    EditError, InspectError, NativeColumn, TableSchema, WriteBlocker,
    format::{
        data_type::{
            BOOLEAN, BOOLEAN_BYTES, BYTE, CURRENCY, DATE_TIME, DOUBLE, GUID, INT16, INT32, INT64,
            NUMERIC, SHORT_TEXT, SINGLE,
        },
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        row::{COLUMN_COUNT_BYTES, MASK_BITS, VARIABLE_COUNT_BYTES, VARIABLE_OFFSET_BYTES},
    },
    header::check,
    table::{
        mutation::record_insertion::payload as insertion_payload,
        record_layout::{self, RecordLayout},
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

/// Each consuming action keeps its original declared scalar language closed.
#[derive(Clone, Copy)]
pub(in crate::table::mutation) enum Language {
    LongText,
    Currency,
    FixedScalars,
    WideValues,
    BooleanMaterialization,
}

/// Exact fixed payload types admitted only by the retained scalar action.
pub(in crate::table::mutation) fn fixed_scalar_kind(native: u8) -> Option<(ScalarType, usize)> {
    Some(match native {
        BYTE => (ScalarType::Byte, size_of::<u8>()),
        INT16 => (ScalarType::Int16, size_of::<i16>()),
        INT64 => (ScalarType::Int64, size_of::<i64>()),
        SINGLE => (ScalarType::Single, size_of::<f32>()),
        DOUBLE => (ScalarType::Double, size_of::<f64>()),
        DATE_TIME => (ScalarType::DateTime, size_of::<f64>()),
        _ => return None,
    })
}

pub(in crate::table::mutation) fn fixed_scalar_value(native: u8, value: &Value) -> bool {
    match (native, value) {
        (BYTE | INT16 | INT64 | SINGLE | DOUBLE | DATE_TIME, Value::Null)
        | (BYTE, Value::Byte(_))
        | (INT16, Value::Int16(_))
        | (INT64, Value::Int64(_))
        | (DATE_TIME, Value::DateTime(_)) => true,
        (SINGLE, Value::Single(value)) => f32::from_bits(value.to_bits()).is_finite(),
        (DOUBLE, Value::Double(value)) => f64::from_bits(value.to_bits()).is_finite(),
        _ => false,
    }
}

/// Field constraints remain native facts, separate from the request carrier.
pub(in crate::table::mutation) fn wide_value(field: &NativeColumn, value: &Value) -> bool {
    match (field.native_type(), value) {
        (GUID | NUMERIC, Value::Null) | (GUID, Value::Guid(_)) => true,
        (NUMERIC, Value::Decimal(value)) => {
            field
                .decimal_parameters()
                .is_some_and(|(precision, scale)| {
                    field.scalar_type() == Some(ScalarType::Decimal)
                        && value.scale() == scale
                        && value.coefficient().unsigned_abs() < 10_u128.pow(u32::from(precision))
                })
        }
        _ => false,
    }
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
        (Language::BooleanMaterialization, BOOLEAN, true)
            if field.scalar_type() == Some(ScalarType::Boolean)
                && field.storage_length() == BOOLEAN_BYTES
                && field.flags() == (FIXED_FLAG | UPDATABLE_FLAG)
                && field.extended_flags() == 0 => {}
        (Language::Currency, CURRENCY, true)
            if field.flags() == (FIXED_FLAG | UPDATABLE_FLAG)
                && field.extended_flags() == 0
                && field.storage_length() == size_of::<i64>() => {}
        (Language::LongText, INT32, true)
            if field.flags() == (FIXED_FLAG | UPDATABLE_FLAG)
                && field.extended_flags() == 0
                && field.storage_length() == size_of::<i32>() => {}
        (Language::FixedScalars, native, true)
            if fixed_scalar_kind(native).is_some_and(|(kind, width)| {
                field.scalar_type() == Some(kind)
                    && field.storage_length() == width
                    && field.flags() == (FIXED_FLAG | UPDATABLE_FLAG)
                    && field.extended_flags() == 0
            }) => {}
        (Language::WideValues, GUID, true)
            if field.scalar_type() == Some(ScalarType::Guid)
                && field.storage_length() == size_of::<u128>()
                && field.flags() == (FIXED_FLAG | UPDATABLE_FLAG)
                && field.extended_flags() == 0 => {}
        (Language::WideValues, NUMERIC, false) => {
            insertion_payload::qualify(field)?;
        }
        (Language::LongText, SHORT_TEXT, false) => storage::qualify(field)?,
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    }
    let original = record_layout::parse(before, schema, budget)?;
    if matches!(language, Language::BooleanMaterialization)
        && (field.number() < original.count
            || field.number() >= schema.physical_layout().max_columns)
    {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
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
        (Language::BooleanMaterialization, BOOLEAN, Value::Boolean(_)) => Vec::new(),
        (Language::LongText, INT32, Value::Int32(_) | Value::Null)
        | (Language::Currency, CURRENCY, Value::Currency(_) | Value::Null) => Vec::new(),
        (Language::FixedScalars, native, value) if fixed_scalar_value(native, value) => Vec::new(),
        (Language::WideValues, GUID, Value::Guid(_) | Value::Null) => Vec::new(),
        (Language::WideValues, NUMERIC, value) if wide_value(field, value) => {
            insertion_payload::encode(field, value, budget, cancelled)?
        }
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
    if matches!(language, Language::FixedScalars) && !matches!(value, Value::Null) {
        let (kind, width) =
            fixed_scalar_kind(field.native_type()).ok_or(EditError::Preservation)?;
        let target = after
            .get_mut(start..start + width)
            .ok_or(EditError::Preservation)?;
        match width {
            1 => write_fixed::<1>(kind, value, target)?,
            2 => write_fixed::<2>(kind, value, target)?,
            4 => write_fixed::<4>(kind, value, target)?,
            8 => write_fixed::<8>(kind, value, target)?,
            _ => return Err(EditError::Preservation),
        }
    }
    if matches!((language, value), (Language::WideValues, Value::Guid(_))) {
        let target = after
            .get_mut(start..start + size_of::<u128>())
            .ok_or(EditError::Preservation)?;
        write_fixed::<{ size_of::<u128>() }>(ScalarType::Guid, value, target)?;
    }
    match value {
        Value::Int32(value) => after
            .get_mut(start..start + size_of::<i32>())
            .ok_or(EditError::Preservation)?
            .copy_from_slice(&value.to_le_bytes()),
        Value::Currency(_) => {
            let encoded = crate::table::scalar_encoding::encode::<{ size_of::<i64>() }>(
                ScalarType::Currency,
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
    let set = match (language, value) {
        (Language::BooleanMaterialization, Value::Boolean(value)) => *value,
        (_, Value::Null) => false,
        _ => true,
    };
    *byte = (*byte & !bit) | if set { bit } else { 0 };
    record_layout::parse(&after, schema, budget)?;
    Ok(after)
}

fn write_fixed<const WIDTH: usize>(
    kind: ScalarType,
    value: &Value,
    target: &mut [u8],
) -> Result<(), EditError> {
    let encoded = crate::table::scalar_encoding::encode::<WIDTH>(kind, value)
        .ok_or(EditError::Preservation)?;
    if target.len() != encoded.len() {
        return Err(EditError::Preservation);
    }
    target.copy_from_slice(&encoded);
    Ok(())
}
