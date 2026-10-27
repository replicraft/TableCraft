//! Preferred variable-record bytes, independent of the exact-slot chooser.
use super::super::super::{EditError, WriteBlocker, slot::storage};
use crate::{
    InspectError, NativeColumn,
    format::{layout::UNICODE_COMPRESSION_FLAG, text},
    header::check,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const MARKER: [u8; 2] = [0xff, 0xfe];
const UNIT_BYTES: usize = size_of::<u16>();

pub(super) fn encode(
    field: &NativeColumn,
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    storage::validate(field, value)?;
    check(budget, cancelled)?;

    let (length, compressed) = match value {
        Value::Null => return Ok(Vec::new()),
        Value::Binary(bytes) => (bytes.len(), false),
        Value::Text(value) => {
            let mut count = 0;
            let mut narrow = true;
            for unit in value.encode_utf16() {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, 1)
                    .map_err(InspectError::from)?;
                count += 1;
                narrow &= unit != 0 && unit <= u16::from(u8::MAX);
            }
            let compressed = field.extended_flags() & UNICODE_COMPRESSION_FLAG != 0
                && count > MARKER.len()
                && narrow;
            let length = if compressed {
                MARKER.len() + count
            } else {
                UNIT_BYTES * count
            };
            (length, compressed)
        }
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    };

    budget
        .charge_allocation(length as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, length as u64)
        .map_err(InspectError::from)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    match value {
        Value::Binary(value) => bytes.extend_from_slice(value),
        Value::Text(value) => {
            if compressed {
                bytes.extend_from_slice(&MARKER);
            }
            for unit in value.encode_utf16() {
                check(budget, cancelled)?;
                if compressed {
                    bytes.push(unit as u8);
                } else {
                    bytes.extend_from_slice(&unit.to_le_bytes());
                }
            }
            if text::decode(&bytes, true, budget)? != *value {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
        }
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    }
    Ok(bytes)
}
