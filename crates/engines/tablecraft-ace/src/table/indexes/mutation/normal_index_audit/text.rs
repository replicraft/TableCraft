//! Physical Text marker/digest/ROW framing, separate from producer TextProfile.
use crate::{
    EditError, InspectError, NativeColumn, TableSchema, encode_text_index_key,
    format::{layout::INDEX_KEY_BYTES, page::PAGE_BYTES, row::MASK_BITS},
    header::check,
    table::record_layout,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const LEAF_FREE_AT: usize = 2;
const LEAF_MASK_AT: usize = 27;
const LEAF_ACTIVE_AT: usize = 480;
const LEAF_MASK_BYTES: usize = LEAF_ACTIVE_AT - LEAF_MASK_AT;
const LEAF_PAGE_BYTES: usize = PAGE_BYTES;

const ROW_BYTES: usize = size_of::<u32>();
const VALUE_MARKER: u8 = 0x7f;
const NULL_MARKER: u8 = 0;
const DIGEST_BYTES: usize = size_of::<u16>();
const POLYNOMIAL: u16 = 0x8005;
const HIGH_BIT: u16 = 1 << (u16::BITS - 1);

pub(in crate::table) type Key = Vec<u8>;

pub(in crate::table) fn original_value<'value>(
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

pub(in crate::table) fn requested(
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
            let mut bytes = vector(1 + ROW_BYTES, budget)?;
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
            let mut bytes = vector(1 + payload.as_bytes().len() + ROW_BYTES, budget)?;
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

pub(in crate::table) fn sort(
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

pub(in crate::table) fn wire(key: &Key) -> &[u8] {
    key
}

// Identical allocation accounting to the prior UPDATE proof owner. Callers
// retain their existing cancellation boundaries; no extra poll is introduced.
fn vector<T>(count: usize, budget: &mut ResourceBudget) -> Result<Vec<T>, EditError> {
    budget
        .charge_allocation(count as u64, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(result)
}

pub(in crate::table) fn distinct(
    keys: &[Key],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    let mut count = usize::from(!keys.is_empty());
    for pair in keys.windows(2) {
        check(budget, cancelled)?;
        let left = pair[0]
            .len()
            .checked_sub(ROW_BYTES)
            .ok_or(EditError::Preservation)?;
        let right = pair[1]
            .len()
            .checked_sub(ROW_BYTES)
            .ok_or(EditError::Preservation)?;
        budget
            .charge(Resource::Steps, left.max(right) as u64)
            .map_err(InspectError::from)?;
        count += usize::from(pair[0][..left] != pair[1][..right]);
    }
    Ok(count)
}

pub(in crate::table) fn require_population(
    before: &[u8],
    keys: &[Key],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let mut mask = [0; LEAF_MASK_BYTES];
    let mut active = 0_usize;
    for key in keys {
        check(budget, cancelled)?;
        let next = active
            .checked_add(key.len())
            .filter(|next| *next <= LEAF_PAGE_BYTES - LEAF_ACTIVE_AT)
            .ok_or(EditError::Preservation)?;
        budget
            .charge(Resource::Steps, key.len() as u64)
            .map_err(InspectError::from)?;
        if before.get(LEAF_ACTIVE_AT + active..LEAF_ACTIVE_AT + next) != Some(key.as_slice()) {
            return Err(EditError::Preservation);
        }
        mask[next / u8::BITS as usize] |= 1 << (next % u8::BITS as usize);
        active = next;
    }
    budget
        .charge(Resource::Steps, (LEAF_MASK_BYTES + size_of::<u16>()) as u64)
        .map_err(InspectError::from)?;
    let free = u16::from_le_bytes(
        before
            .get(LEAF_FREE_AT..LEAF_FREE_AT + size_of::<u16>())
            .ok_or(EditError::Preservation)?
            .try_into()
            .map_err(|_| EditError::Preservation)?,
    );
    if before.get(LEAF_MASK_AT..LEAF_ACTIVE_AT) != Some(mask.as_slice())
        || usize::from(free) != LEAF_PAGE_BYTES - LEAF_ACTIVE_AT - active
    {
        return Err(EditError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}

pub(in crate::table) fn write_population(
    expected: &mut [u8],
    keys: &[Key],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let mut mask = [0; LEAF_MASK_BYTES];
    let mut active = 0_usize;
    for key in keys {
        check(budget, cancelled)?;
        let next = active
            .checked_add(key.len())
            .filter(|next| *next <= LEAF_PAGE_BYTES - LEAF_ACTIVE_AT)
            .ok_or(EditError::Preservation)?;
        budget
            .charge(Resource::Steps, key.len() as u64)
            .map_err(InspectError::from)?;
        expected
            .get_mut(LEAF_ACTIVE_AT + active..LEAF_ACTIVE_AT + next)
            .ok_or(EditError::Preservation)?
            .copy_from_slice(key);
        mask[next / u8::BITS as usize] |= 1 << (next % u8::BITS as usize);
        active = next;
    }
    budget
        .charge(Resource::Steps, (LEAF_MASK_BYTES + size_of::<u16>()) as u64)
        .map_err(InspectError::from)?;
    expected
        .get_mut(LEAF_MASK_AT..LEAF_ACTIVE_AT)
        .ok_or(EditError::Preservation)?
        .copy_from_slice(&mask);
    let free = u16::try_from(LEAF_PAGE_BYTES - LEAF_ACTIVE_AT - active)
        .map_err(|_| EditError::Preservation)?;
    expected
        .get_mut(LEAF_FREE_AT..LEAF_FREE_AT + size_of::<u16>())
        .ok_or(EditError::Preservation)?
        .copy_from_slice(&free.to_le_bytes());
    check(budget, cancelled)?;
    Ok(())
}
