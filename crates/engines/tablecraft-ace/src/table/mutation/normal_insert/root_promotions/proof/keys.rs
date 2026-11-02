//! Raw fixed carrier/mask binding and independent signed key+ROW interpretation.
use crate::{
    EditError, InspectError, NativeColumn, TableSchema,
    format::row::{COLUMN_COUNT_BYTES, MASK_BITS},
    header::check,
    table::record_layout,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Currency, Value},
};
use tablecraft_schema::ScalarType;

#[derive(Clone, Copy)]
pub(super) struct Key {
    pub units: i64,
    pub reference: u32,
}

pub(super) fn requested(value: &Value, field: &NativeColumn) -> Result<i64, EditError> {
    match (field.scalar_type(), value) {
        (Some(ScalarType::Int32), Value::Int32(value)) => Ok(i64::from(*value)),
        (Some(ScalarType::Int64), Value::Int64(value)) => Ok(*value),
        (Some(ScalarType::Currency), Value::Currency(value)) => Ok(value.scaled()),
        _ => Err(EditError::Preservation),
    }
}

pub(super) fn original(
    raw: &[u8],
    schema: &TableSchema,
    field: &NativeColumn,
    observed: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<i64, EditError> {
    check(budget, cancelled)?;
    let layout = record_layout::parse(raw, schema, budget)?;
    if field.number() >= layout.count {
        return Err(EditError::Preservation);
    }
    let mask = layout
        .mask_start
        .checked_add(field.number() / MASK_BITS)
        .ok_or(EditError::Preservation)?;
    if raw.get(mask).ok_or(EditError::Preservation)? & (1 << (field.number() % MASK_BITS)) == 0 {
        return Err(EditError::Preservation);
    }
    let width = field.storage_length();
    let start = COLUMN_COUNT_BYTES
        .checked_add(field.field.offset)
        .ok_or(EditError::Preservation)?;
    let end = start
        .checked_add(width)
        .filter(|end| *end <= layout.mask_start)
        .ok_or(EditError::Preservation)?;
    budget
        .charge(Resource::Steps, width as u64)
        .map_err(InspectError::from)?;
    let bytes = raw.get(start..end).ok_or(EditError::Preservation)?;
    let value = match field.scalar_type() {
        Some(ScalarType::Int32) => Value::Int32(i32::from_le_bytes(
            bytes.try_into().map_err(|_| EditError::Preservation)?,
        )),
        Some(ScalarType::Int64) => Value::Int64(i64::from_le_bytes(
            bytes.try_into().map_err(|_| EditError::Preservation)?,
        )),
        Some(ScalarType::Currency) => Value::Currency(Currency::from_scaled(i64::from_le_bytes(
            bytes.try_into().map_err(|_| EditError::Preservation)?,
        ))),
        _ => return Err(EditError::Preservation),
    };
    if &value != observed {
        return Err(EditError::Preservation);
    }
    requested(&value, field)
}

pub(super) fn sort(
    keys: &mut [Key],
    descending: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let levels = keys.len().checked_ilog2().unwrap_or(0) as usize + 1;
    let work = keys
        .len()
        .checked_mul(levels)
        .and_then(|n| n.checked_mul(size_of::<Key>()))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    keys.sort_unstable_by(|left, right| {
        let order = left.units.cmp(&right.units);
        (if descending { order.reverse() } else { order })
            .then(left.reference.cmp(&right.reference))
    });
    check(budget, cancelled)?;
    Ok(())
}

pub(super) fn distinct(
    keys: &[Key],
    unique: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    let mut count = 0_usize;
    let mut previous = None;
    for key in keys {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, size_of::<i64>() as u64)
            .map_err(InspectError::from)?;
        if previous == Some(key.units) {
            if unique {
                return Err(EditError::Preservation);
            }
        } else {
            count = count.checked_add(1).ok_or(InspectError::StructureLimit)?;
        }
        previous = Some(key.units);
    }
    Ok(count)
}

pub(super) fn wire(key: Key, wide: bool, descending: bool) -> ([u8; 13], usize) {
    let mut result = [0; 13];
    result[0] = if descending { 0x80 } else { 0x7f };
    let width = if wide { 8 } else { 4 };
    if wide {
        result[1..9].copy_from_slice(&((key.units as u64) ^ 0x8000_0000_0000_0000).to_be_bytes());
    } else {
        result[1..5].copy_from_slice(&((key.units as u32) ^ 0x8000_0000).to_be_bytes());
    }
    if descending {
        for byte in &mut result[1..1 + width] {
            *byte = !*byte;
        }
    }
    result[1 + width..5 + width].copy_from_slice(&key.reference.to_be_bytes());
    (result, 5 + width)
}
