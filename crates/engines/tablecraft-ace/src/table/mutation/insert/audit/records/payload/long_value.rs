//! Request-derived ordinary Memo/OLE bytes and inline descriptors.
use crate::{
    EditError, InspectError, NativeColumn, WriteBlocker,
    format::{
        data_type::{LONG_TEXT, OLE},
        text,
    },
    header::check,
    limits::{MAX_LONG_TEXT_BYTES, MAX_LONG_VALUE_BYTES},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const INLINE_LIMIT: usize = 64;
const MEMO_COMPRESSION_LIMIT: usize = 1024;
const COMPRESSED_UNICODE: u8 = 1;
const COMPRESSION_MARKER: [u8; 2] = [0xff, 0xfe];
const INLINE_TAG: u32 = 0x8000_0000;
const DESCRIPTOR_BYTES: usize = 3 * size_of::<u32>();
const CHECK_BYTES: usize = 256;

pub(super) enum Payload {
    Null,
    Present(Vec<u8>),
}

pub(super) fn requested(
    field: &NativeColumn,
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Payload, EditError> {
    check(budget, cancelled)?;
    let (length, compact) = match (field.native_type(), value) {
        (LONG_TEXT | OLE, Value::Null) => return Ok(Payload::Null),
        (OLE, Value::Binary(bytes)) => (bytes.len(), false),
        (LONG_TEXT, Value::Text(value)) => {
            if value.len() > MAX_LONG_TEXT_BYTES {
                return Err(InspectError::StructureLimit.into());
            }
            let mut count = 0_usize;
            let mut narrow = true;
            for unit in value.encode_utf16() {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, 1)
                    .map_err(InspectError::from)?;
                count = count.checked_add(1).ok_or(InspectError::StructureLimit)?;
                narrow &= unit != 0 && unit <= u16::from(u8::MAX);
            }
            let compact = field.extended_flags() == COMPRESSED_UNICODE
                && (COMPRESSION_MARKER.len() + 1..=MEMO_COMPRESSION_LIMIT).contains(&count)
                && narrow;
            let length = if compact {
                count.checked_add(COMPRESSION_MARKER.len())
            } else {
                count.checked_mul(size_of::<u16>())
            }
            .ok_or(InspectError::StructureLimit)?;
            (length, compact)
        }
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    };
    if length > MAX_LONG_VALUE_BYTES {
        return Err(InspectError::StructureLimit.into());
    }

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
        Value::Binary(value) => {
            for chunk in value.chunks(CHECK_BYTES) {
                check(budget, cancelled)?;
                bytes.extend_from_slice(chunk);
            }
        }
        Value::Text(value) => {
            if compact {
                bytes.extend_from_slice(&COMPRESSION_MARKER);
            }
            for unit in value.encode_utf16() {
                check(budget, cancelled)?;
                if compact {
                    bytes.push(u8::try_from(unit).map_err(|_| EditError::Preservation)?);
                } else {
                    bytes.extend_from_slice(&unit.to_le_bytes());
                }
            }
            let decoded = text::decode_long_with_limits(
                &bytes,
                MAX_LONG_VALUE_BYTES,
                MAX_LONG_TEXT_BYTES,
                budget,
                cancelled,
            )?;
            if decoded != *value {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
        }
        _ => return Err(EditError::Preservation),
    }
    if bytes.len() != length {
        return Err(EditError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(Payload::Present(bytes))
}

pub(super) fn encode_inline(
    field: &NativeColumn,
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let payload = requested(field, value, budget, cancelled)?;
    let Payload::Present(payload) = payload else {
        return Ok(Vec::new());
    };
    if payload.len() > INLINE_LIMIT {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let length = DESCRIPTOR_BYTES
        .checked_add(payload.len())
        .ok_or(InspectError::StructureLimit)?;
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
    let count = u32::try_from(payload.len()).map_err(|_| InspectError::StructureLimit)?;
    result.extend_from_slice(&(INLINE_TAG | count).to_le_bytes());
    result.extend_from_slice(&[0; DESCRIPTOR_BYTES - size_of::<u32>()]);
    result.extend_from_slice(&payload);
    Ok(result)
}
