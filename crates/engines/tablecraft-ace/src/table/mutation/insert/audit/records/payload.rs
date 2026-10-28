//! Requested preferred variable wires; no builder serializer supplies proof.
mod decimal;
pub(in crate::table::mutation::insert::audit) mod long_value;
use crate::{
    EditError, InspectError, NativeColumn, WriteBlocker,
    format::{
        data_type::{LONG_TEXT, NUMERIC, OLE, SHORT_BINARY, SHORT_TEXT},
        layout::UNICODE_COMPRESSION_FLAG,
        text,
    },
    header::check,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const MARKER: [u8; 2] = [0xff, 0xfe];
const UNIT_BYTES: usize = size_of::<u16>();

pub(in crate::table::mutation::insert::audit) fn encode(
    field: &NativeColumn,
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    check(budget, cancelled)?;
    if matches!(field.native_type(), LONG_TEXT | OLE) {
        return long_value::encode_inline(field, value, budget, cancelled);
    }
    if field.native_type() == NUMERIC {
        return decimal::encode(field, value, budget, cancelled);
    }
    let (length, compressed) = match (field.native_type(), value) {
        (_, Value::Null) => return Ok(Vec::new()),
        (SHORT_BINARY, Value::Binary(bytes)) if bytes.len() <= field.storage_length() => {
            (bytes.len(), false)
        }
        (SHORT_TEXT, Value::Text(value)) => {
            let mut units = 0_usize;
            let mut narrow = true;
            for unit in value.encode_utf16() {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, 1)
                    .map_err(InspectError::from)?;
                units += 1;
                if units > field.storage_length() / UNIT_BYTES {
                    return Err(EditError::Unsupported(WriteBlocker::Column));
                }
                narrow &= unit != 0 && unit <= u16::from(u8::MAX);
            }
            let compressed = field.extended_flags() & UNICODE_COMPRESSION_FLAG != 0
                && narrow
                && units > MARKER.len();
            let length = if compressed {
                MARKER.len() + units
            } else {
                UNIT_BYTES * units
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
    check(budget, cancelled)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;

    match value {
        Value::Binary(bytes) => result.extend_from_slice(bytes),
        Value::Text(value) => {
            if compressed {
                result.extend_from_slice(&MARKER);
            }
            for unit in value.encode_utf16() {
                check(budget, cancelled)?;
                if compressed {
                    result.push(unit as u8);
                } else {
                    result.extend_from_slice(&unit.to_le_bytes());
                }
            }
            if text::decode(&result, true, budget)? != *value {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
        }
        _ => return Err(EditError::Preservation),
    }

    if result.len() != length {
        return Err(EditError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(result)
}
