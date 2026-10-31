//! Fixed-cell reconstruction belongs to this auditor, not the producer codec.
use super::super::super::super::{name_work, source::Source};
use crate::{
    EditError, FixedScalarUpdate, InspectError, NativeColumn, WriteBlocker,
    format::{
        data_type::{BOOLEAN, CURRENCY, DATE_TIME, INT32, INT64},
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        row::{COLUMN_COUNT_BYTES, MASK_BITS},
    },
    header::check,
    table::record_layout,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::ScalarType;

const WIDE_BYTES: usize = size_of::<i64>();
const INT32_BYTES: usize = size_of::<i32>();

#[derive(Clone, Copy)]
enum Carrier {
    Boolean,
    Int32,
    Int64,
    Currency,
    DateTime,
}

pub(super) struct Bound<'a> {
    pub(super) field: &'a NativeColumn,
    carrier: Carrier,
    pub(super) width: usize,
}

#[derive(PartialEq, Eq)]
enum Cell {
    Boolean(bool),
    Fixed(Option<[u8; WIDE_BYTES]>),
}

pub(super) fn select<'a>(
    original: &'a Source,
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Bound<'a>, EditError> {
    let schema = original.table.schema();
    let mut selected = None;
    for field in schema.columns() {
        check(budget, cancelled)?;
        name_work(field.name(), name, budget)?;
        if field.name().eq_ignore_ascii_case(name) && selected.replace(field).is_some() {
            return Err(EditError::Preservation);
        }
    }
    let field = selected.ok_or(EditError::Unsupported(WriteBlocker::Column))?;
    let (carrier, width) = match (field.native_type(), field.scalar_type()) {
        (BOOLEAN, Some(ScalarType::Boolean)) => (Carrier::Boolean, 0),
        (INT32, Some(ScalarType::Int32)) => (Carrier::Int32, INT32_BYTES),
        (INT64, Some(ScalarType::Int64)) => (Carrier::Int64, WIDE_BYTES),
        (CURRENCY, Some(ScalarType::Currency)) => (Carrier::Currency, WIDE_BYTES),
        (DATE_TIME, Some(ScalarType::DateTime)) => (Carrier::DateTime, WIDE_BYTES),
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    };
    if !field.fixed()
        || field.storage_length() != width
        || field.extended_flags() != 0
        || field.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
    {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    if width != 0 {
        let end = field
            .field
            .offset
            .checked_add(width)
            .ok_or(EditError::Preservation)?;
        for other in schema.columns() {
            check(budget, cancelled)?;
            if other.number() != field.number() && other.fixed() && other.storage_length() != 0 {
                let other_end = other
                    .field
                    .offset
                    .checked_add(other.storage_length())
                    .ok_or(EditError::Preservation)?;
                if other.field.offset < end && other_end > field.field.offset {
                    return Err(EditError::Unsupported(WriteBlocker::Column));
                }
            }
        }
    }
    for dependency in &original.dependencies {
        check(budget, cancelled)?;
        if *dependency == field.number() {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }
    for index in original.indexes.indexes() {
        for key in index.columns() {
            check(budget, cancelled)?;
            if key.number() == field.number() {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
        }
    }
    Ok(Bound {
        field,
        carrier,
        width,
    })
}

pub(super) fn overlay_request(
    original: &Source,
    selected: &Bound<'_>,
    request: &FixedScalarUpdate,
    expected: &mut [u8],
    budget: &mut ResourceBudget,
) -> Result<Option<u32>, EditError> {
    let number = selected.field.number();
    let before = encode(selected.carrier, request.expected())?;
    let replacement = encode(selected.carrier, request.value())?;
    let record = original
        .records
        .get(request.row())
        .ok_or(EditError::Conflict)?;
    let layout = record_layout::parse(&record.data, original.table.schema(), budget)?;
    if number >= layout.count {
        return Err(EditError::Preservation);
    }
    let mask_at = layout
        .mask_start
        .checked_add(number / MASK_BITS)
        .ok_or(EditError::Preservation)?;
    let mask = *record.data.get(mask_at).ok_or(InspectError::Malformed)?;
    let bit = 1 << (number % MASK_BITS);
    let output_mask_at = record
        .offset
        .checked_add(mask_at)
        .ok_or(EditError::Preservation)?;
    match (&before, &replacement) {
        (Cell::Boolean(before), Cell::Boolean(value)) => {
            if (mask & bit != 0) != *before {
                return Err(EditError::Conflict);
            }
            if before == value {
                return Ok(None);
            }
            set_bit(expected, output_mask_at, bit, *value)?;
        }
        (Cell::Fixed(before), Cell::Fixed(value)) => {
            if (mask & bit != 0) != before.is_some() {
                return Err(EditError::Conflict);
            }
            let at = COLUMN_COUNT_BYTES
                .checked_add(selected.field.field.offset)
                .ok_or(EditError::Preservation)?;
            let end = at
                .checked_add(selected.width)
                .ok_or(EditError::Preservation)?;
            if end > layout.data_end {
                return Err(EditError::Preservation);
            }
            let actual = record.data.get(at..end).ok_or(InspectError::Malformed)?;
            if before
                .as_ref()
                .is_some_and(|bytes| actual != &bytes[..selected.width])
            {
                return Err(EditError::Conflict);
            }
            if before == value {
                return Ok(None);
            }
            if let Some(bytes) = value {
                let start = record
                    .offset
                    .checked_add(at)
                    .ok_or(EditError::Preservation)?;
                let end = start
                    .checked_add(selected.width)
                    .ok_or(EditError::Preservation)?;
                expected
                    .get_mut(start..end)
                    .ok_or(EditError::Preservation)?
                    .copy_from_slice(&bytes[..selected.width]);
            }
            if before.is_some() != value.is_some() {
                // Accumulate only this bit in expected, never assign an
                // original whole mask byte over another group's edit.
                set_bit(expected, output_mask_at, bit, value.is_some())?;
            }
        }
        _ => return Err(EditError::Preservation),
    }
    Ok(Some(record.reference))
}

fn encode(carrier: Carrier, value: &Value) -> Result<Cell, EditError> {
    let bytes = match (carrier, value) {
        (Carrier::Boolean, Value::Boolean(value)) => return Ok(Cell::Boolean(*value)),
        (Carrier::Int32 | Carrier::Int64 | Carrier::Currency | Carrier::DateTime, Value::Null) => {
            return Ok(Cell::Fixed(None));
        }
        (Carrier::Int32, Value::Int32(value)) => {
            let mut bytes = [0; WIDE_BYTES];
            bytes[..INT32_BYTES].copy_from_slice(&value.to_le_bytes());
            bytes
        }
        (Carrier::Int64, Value::Int64(value)) => value.to_le_bytes(),
        (Carrier::Currency, Value::Currency(value)) => value.scaled().to_le_bytes(),
        (Carrier::DateTime, Value::DateTime(value)) => value.to_bits().to_le_bytes(),
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    };
    Ok(Cell::Fixed(Some(bytes)))
}

fn set_bit(expected: &mut [u8], at: usize, bit: u8, value: bool) -> Result<(), EditError> {
    let byte = expected.get_mut(at).ok_or(EditError::Preservation)?;
    if value {
        *byte |= bit;
    } else {
        *byte &= !bit;
    }
    Ok(())
}
