//! Append-based reconstruction independent of the current-record write cursor.
use crate::{
    EditError, InspectError, TableSchema,
    format::{
        data_type::{CURRENCY, GUID, INT32, NUMERIC, SHORT_TEXT},
        row::{COLUMN_COUNT_BYTES, MASK_BITS},
    },
    header::check,
    table::{
        mutation::{
            record_insertion::proof::payload as insertion_payload,
            record_serialization::current_rows::{self, Language},
        },
        record_layout,
    },
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
    language: Language,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    check(budget, cancelled)?;
    let original = record_layout::parse(before, schema, budget)?;
    let field = schema
        .columns()
        .get(column)
        .ok_or(EditError::Preservation)?;
    let payload = match (language, field.native_type(), value) {
        (Language::LongText, INT32, Value::Int32(_) | Value::Null) if field.fixed() => Vec::new(),
        (Language::Currency, CURRENCY, Value::Currency(_) | Value::Null)
            if field.fixed() && field.storage_length() == size_of::<i64>() =>
        {
            Vec::new()
        }
        (Language::FixedScalars, native, value)
            if field.fixed()
                && current_rows::fixed_scalar_kind(native)
                    .is_some_and(|(_, width)| field.storage_length() == width)
                && current_rows::fixed_scalar_value(native, value) =>
        {
            Vec::new()
        }
        (Language::WideValues, GUID, Value::Guid(_) | Value::Null)
            if field.fixed() && field.storage_length() == size_of::<u128>() =>
        {
            Vec::new()
        }
        (Language::WideValues, NUMERIC, value)
            if !field.fixed() && current_rows::wide_value(field, value) =>
        {
            insertion_payload::encode(field, value, budget, cancelled)?
        }
        (Language::LongText, SHORT_TEXT, Value::Text(_) | Value::Null) if !field.fixed() => {
            super::payload::encode(field, value, budget, cancelled)?
        }
        _ => return Err(EditError::Preservation),
    };
    let variables = schema.physical_layout().variable_slots;
    let fields = schema.physical_layout().max_columns;
    let old_slots = original.offsets.len().saturating_sub(1);
    if old_slots > variables {
        return Err(EditError::Preservation);
    }
    let fixed_stop = original
        .offsets
        .first()
        .copied()
        .unwrap_or(original.data_end);
    let fixed_stop_after = fixed_stop.max(schema.physical_layout().fixed_end);
    let original_end = original.offsets.last().copied().unwrap_or(fixed_stop);
    let selected = (!field.fixed()).then_some(field.physical_field().variable);
    budget
        .charge_allocation(variables as u64, size_of::<&[u8]>() as u64)
        .map_err(InspectError::from)?;
    let mut segments = Vec::new();
    segments
        .try_reserve_exact(variables)
        .map_err(|_| InspectError::Allocation)?;
    let mut bytes = fixed_stop_after;
    for slot in 0..variables {
        check(budget, cancelled)?;
        let segment = if selected == Some(slot) {
            payload.as_slice()
        } else if slot < old_slots {
            before
                .get(original.offsets[slot]..original.offsets[slot + 1])
                .ok_or(EditError::Preservation)?
        } else {
            &[]
        };
        bytes = bytes
            .checked_add(segment.len())
            .ok_or(InspectError::StructureLimit)?;
        segments.push(segment);
    }
    let padding = before
        .get(original_end..original.data_end)
        .ok_or(EditError::Preservation)?;
    let footer = if variables == 0 {
        0
    } else {
        variables
            .checked_add(2)
            .and_then(|words| words.checked_mul(size_of::<u16>()))
            .ok_or(InspectError::StructureLimit)?
    };
    let data_end = bytes
        .checked_add(padding.len())
        .ok_or(InspectError::StructureLimit)?
        .max(original.data_end);
    let length = data_end
        .checked_add(footer)
        .and_then(|length| length.checked_add(fields.div_ceil(MASK_BITS)))
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
    expected.extend(
        u16::try_from(fields)
            .map_err(|_| InspectError::StructureLimit)?
            .to_le_bytes(),
    );
    expected.extend_from_slice(
        before
            .get(COLUMN_COUNT_BYTES..fixed_stop)
            .ok_or(EditError::Preservation)?,
    );
    expected.resize(fixed_stop_after, 0);
    if let Value::Int32(value) = value {
        let start = COLUMN_COUNT_BYTES + field.physical_field().offset;
        expected
            .get_mut(start..start + size_of::<i32>())
            .ok_or(EditError::Preservation)?
            .copy_from_slice(&value.to_le_bytes());
    }
    if let Value::Currency(value) = value {
        let start = COLUMN_COUNT_BYTES + field.physical_field().offset;
        expected
            .get_mut(start..start + size_of::<i64>())
            .ok_or(EditError::Preservation)?
            .copy_from_slice(&value.scaled().to_le_bytes());
    }
    if matches!(language, Language::FixedScalars) {
        // Independent scalar reconstruction writes literal carrier bytes into
        // the separately assembled fixed prefix, without the producer cursor.
        let start = COLUMN_COUNT_BYTES + field.physical_field().offset;
        let width = field.storage_length();
        let target = expected
            .get_mut(start..start + width)
            .ok_or(EditError::Preservation)?;
        match value {
            Value::Byte(value) => target.copy_from_slice(&[*value]),
            Value::Int16(value) => target.copy_from_slice(&value.to_le_bytes()),
            Value::Int64(value) => target.copy_from_slice(&value.to_le_bytes()),
            Value::Single(value) => target.copy_from_slice(&value.to_bits().to_le_bytes()),
            Value::Double(value) => target.copy_from_slice(&value.to_bits().to_le_bytes()),
            Value::DateTime(value) => target.copy_from_slice(&value.to_bits().to_le_bytes()),
            Value::Null => {}
            _ => return Err(EditError::Preservation),
        }
    }
    if let (Language::WideValues, Value::Guid(value)) = (language, value) {
        let mut native = value.to_bytes();
        native[..4].reverse();
        native[4..6].reverse();
        native[6..8].reverse();
        let start = COLUMN_COUNT_BYTES + field.physical_field().offset;
        expected
            .get_mut(start..start + native.len())
            .ok_or(EditError::Preservation)?
            .copy_from_slice(&native);
    }
    for segment in &segments {
        check(budget, cancelled)?;
        expected.extend_from_slice(segment);
    }
    expected.extend_from_slice(padding);
    expected.resize(data_end, 0);
    if variables != 0 {
        let mut offset = bytes;
        expected.extend(
            u16::try_from(offset)
                .map_err(|_| InspectError::StructureLimit)?
                .to_le_bytes(),
        );
        for segment in segments.into_iter().rev() {
            check(budget, cancelled)?;
            offset = offset
                .checked_sub(segment.len())
                .ok_or(EditError::Preservation)?;
            expected.extend(
                u16::try_from(offset)
                    .map_err(|_| InspectError::StructureLimit)?
                    .to_le_bytes(),
            );
        }
        expected.extend(
            u16::try_from(variables)
                .map_err(|_| InspectError::StructureLimit)?
                .to_le_bytes(),
        );
    }
    let mask = expected.len();
    expected.extend_from_slice(&before[original.mask_start..]);
    expected.resize(length, 0);
    for number in original.count..fields {
        check(budget, cancelled)?;
        expected[mask + number / MASK_BITS] &= !(1 << (number % MASK_BITS));
    }
    let number = field.number();
    let bit = 1 << (number % MASK_BITS);
    let byte = expected
        .get_mut(mask + number / MASK_BITS)
        .ok_or(EditError::Preservation)?;
    *byte = (*byte & !bit) | if matches!(value, Value::Null) { 0 } else { bit };
    if expected.len() != length {
        return Err(EditError::Preservation);
    }
    Ok(expected)
}
