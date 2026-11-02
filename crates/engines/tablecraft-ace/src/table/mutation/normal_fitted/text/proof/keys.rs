//! Physical Text marker/digest/ROW framing, separate from producer TextProfile.
use crate::{
    EditError, InspectError, NativeColumn, TableSchema, encode_text_index_key,
    format::{layout::INDEX_KEY_BYTES, row::MASK_BITS},
    header::check,
    table::{mutation::normal_fitted::binding, record_layout},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const ROW_BYTES: usize = size_of::<u32>();
const VALUE_MARKER: u8 = 0x7f;
const NULL_MARKER: u8 = 0;
const DIGEST_BYTES: usize = size_of::<u16>();
const POLYNOMIAL: u16 = 0x8005;
const HIGH_BIT: u16 = 1 << (u16::BITS - 1);

pub(super) type Key = Vec<u8>;

pub(super) fn original_value<'value>(
    raw: &[u8],
    schema: &TableSchema,
    field: &NativeColumn,
    observed: &'value Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'value Value, EditError> {
    check(budget, cancelled)?;
    let layout = record_layout::parse(raw, schema, budget)?;
    if field.number() >= layout.count || field.extended_flags() != 0 {
        return Err(EditError::Preservation);
    }
    let mask = layout
        .mask_start
        .checked_add(field.number() / MASK_BITS)
        .ok_or(EditError::Preservation)?;
    let present =
        raw.get(mask).ok_or(EditError::Preservation)? & (1 << (field.number() % MASK_BITS)) != 0;
    if !present {
        if observed != &Value::Null {
            return Err(EditError::Preservation);
        }
        return Ok(observed);
    }
    let start = *layout
        .offsets
        .get(field.field.variable)
        .ok_or(EditError::Preservation)?;
    let end = *layout
        .offsets
        .get(field.field.variable + 1)
        .ok_or(EditError::Preservation)?;
    let payload = raw.get(start..end).ok_or(EditError::Preservation)?;
    let Value::Text(text) = observed else {
        return Err(EditError::Preservation);
    };
    if !payload.len().is_multiple_of(size_of::<u16>()) {
        return Err(EditError::Preservation);
    }
    let mut units = text.encode_utf16();
    for bytes in payload.as_chunks::<{ size_of::<u16>() }>().0 {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, size_of::<u16>() as u64)
            .map_err(InspectError::from)?;
        if units.next() != Some(u16::from_le_bytes(*bytes)) {
            return Err(EditError::Preservation);
        }
    }
    if units.next().is_some() {
        return Err(EditError::Preservation);
    }
    Ok(observed)
}

pub(super) fn requested(
    field: &NativeColumn,
    value: &Value,
    reference: u32,
    descending: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Key, EditError> {
    check(budget, cancelled)?;
    let mut encoded = match value {
        Value::Null => {
            let mut bytes = binding::vector(1 + ROW_BYTES, budget)?;
            bytes.push(if descending {
                !NULL_MARKER
            } else {
                NULL_MARKER
            });
            bytes
        }
        Value::Text(text) => {
            let order = field.text_sort_order().ok_or(EditError::Preservation)?;
            if order.general_family().is_none() {
                return Err(EditError::Preservation);
            }
            let payload = encode_text_index_key(order, text, descending, budget, cancelled)?;
            let mut bytes = binding::vector(1 + payload.as_bytes().len() + ROW_BYTES, budget)?;
            bytes.push(if descending {
                !VALUE_MARKER
            } else {
                VALUE_MARKER
            });
            budget
                .charge(Resource::Steps, payload.as_bytes().len() as u64)
                .map_err(InspectError::from)?;
            bytes.extend_from_slice(payload.as_bytes());
            if bytes.len() > INDEX_KEY_BYTES {
                let keep = INDEX_KEY_BYTES - DIGEST_BYTES;
                let mut register = 0_u16;
                for byte in &bytes[keep..bytes.len() - 1] {
                    check(budget, cancelled)?;
                    budget
                        .charge(Resource::Steps, u8::BITS as u64)
                        .map_err(InspectError::from)?;
                    for _ in 0..u8::BITS {
                        let high = register & HIGH_BIT != 0;
                        register = register.wrapping_shl(1);
                        if high {
                            register ^= POLYNOMIAL;
                        }
                    }
                    register ^= u16::from(*byte) << u8::BITS;
                }
                register ^= u16::from(*bytes.last().ok_or(EditError::Preservation)?);
                bytes.truncate(INDEX_KEY_BYTES);
                budget
                    .charge(Resource::Steps, DIGEST_BYTES as u64)
                    .map_err(InspectError::from)?;
                bytes[keep..].copy_from_slice(&register.to_le_bytes());
            }
            bytes
        }
        _ => return Err(EditError::Preservation),
    };
    budget
        .charge(Resource::Steps, ROW_BYTES as u64)
        .map_err(InspectError::from)?;
    encoded.extend_from_slice(&reference.to_be_bytes());
    Ok(encoded)
}

pub(super) fn sort(
    keys: &mut [Key],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let levels = keys.len().checked_ilog2().unwrap_or(0) as usize + 1;
    let work = keys
        .len()
        .checked_mul(levels)
        .and_then(|count| count.checked_mul(INDEX_KEY_BYTES + ROW_BYTES))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    keys.sort_unstable();
    check(budget, cancelled)?;
    Ok(())
}

pub(super) fn wire(key: &Key) -> &[u8] {
    key
}
