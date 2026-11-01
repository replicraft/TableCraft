//! Independent optional/directional Int32 wire replay; no prepared insertion keys.
use super::{FREE_AT, KEY_AT, MASK_AT, MASK_BYTES, RequestedRows};
use crate::{
    EditError, InspectError, NativeColumn, NativeImage, NativeIndex, TableSchema,
    format::{
        bytes::u16_at,
        page::PAGE_BYTES,
        row::{COLUMN_COUNT_BYTES, MASK_BITS},
    },
    header::check,
    table::{mutation::preserved_source::Source, record_layout},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const ASC_NULL: u8 = 0;
const ASC_PRESENT: u8 = 0x7f;
const DESC_NULL: u8 = u8::MAX;
const DESC_PRESENT: u8 = 0x80;
const MARKER_BYTES: usize = size_of::<u8>();
const VALUE_BYTES: usize = size_of::<i32>();
const ROW_BYTES: usize = size_of::<u32>();
const MAX_WIRE_BYTES: usize = MARKER_BYTES + VALUE_BYTES + ROW_BYTES;
const SIGN_BIT: u32 = 1 << (u32::BITS - 1);

type Key = (Option<i32>, u32);

pub(super) fn reconstruct(
    image: &NativeImage,
    source: &Source,
    index: &NativeIndex,
    rows: RequestedRows<'_>,
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let RequestedRows {
        requests,
        references,
    } = rows;
    if requests.len() != references.len() || source.table.rows().len() != source.records.len() {
        return Err(EditError::Preservation);
    }
    let [key] = index.columns() else {
        return Err(EditError::Preservation);
    };
    let schema = source.table.schema();
    let mut column = None;
    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if field.number() == key.number() {
            column = Some(position);
        }
    }
    let column = column.ok_or(EditError::Preservation)?;
    let field = &schema.columns()[column];
    let mut original = super::super::vector(source.records.len(), budget, cancelled)?;
    for (record, observed) in source.records.iter().zip(source.table.rows()) {
        check(budget, cancelled)?;
        let value = original_value(
            &record.data,
            schema,
            field,
            observed.get(column).ok_or(EditError::Preservation)?,
            budget,
            cancelled,
        )?;
        original.push((value, record.reference));
    }
    sort(&mut original, key.descending(), budget, cancelled)?;
    let base = (index.root_page() as usize)
        .checked_mul(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    let end = base
        .checked_add(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    let page = image
        .as_bytes()
        .get(base..end)
        .ok_or(EditError::Preservation)?;
    let mut original_mask = [0; MASK_BYTES];
    let mut original_bytes = 0;
    for pair in &original {
        check(budget, cancelled)?;
        let (wire, width) = wire(*pair, key.descending());
        let next = original_bytes + width;
        if next > PAGE_BYTES - KEY_AT {
            return Err(EditError::Preservation);
        }
        budget
            .charge(Resource::Steps, width as u64)
            .map_err(InspectError::from)?;
        if page.get(KEY_AT + original_bytes..KEY_AT + next) != Some(&wire[..width]) {
            return Err(EditError::Preservation);
        }
        original_mask[next / 8] |= 1 << (next % 8);
        original_bytes = next;
    }
    budget
        .charge(Resource::Steps, MASK_BYTES as u64)
        .map_err(InspectError::from)?;
    if page[MASK_AT..KEY_AT] != original_mask
        || usize::from(u16_at(page, FREE_AT)?) != PAGE_BYTES - KEY_AT - original_bytes
    {
        return Err(EditError::Preservation);
    }
    let capacity = original
        .len()
        .checked_add(requests.len())
        .ok_or(InspectError::StructureLimit)?;
    let mut final_keys = super::super::vector(capacity, budget, cancelled)?;
    let copy_work = original
        .len()
        .checked_mul(size_of::<Key>())
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, copy_work as u64)
        .map_err(InspectError::from)?;
    final_keys.extend_from_slice(&original);
    for (request, reference) in requests.iter().zip(references) {
        check(budget, cancelled)?;
        let value = match request.values().get(column) {
            Some(Value::Int32(value)) => Some(*value),
            Some(Value::Null) => None,
            _ => return Err(EditError::Preservation),
        };
        final_keys.push((value, *reference));
    }
    sort(&mut final_keys, key.descending(), budget, cancelled)?;
    let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
    let before_distinct = distinct(&original, budget, cancelled)?;
    let mut previous = None;
    let mut after_distinct = 0;
    let mut mask = [0; MASK_BYTES];
    let mut active_bytes = 0;
    for pair in &final_keys {
        check(budget, cancelled)?;
        if (constraints.required() && pair.0.is_none())
            || (constraints.unique() && pair.0.is_some() && previous == Some(pair.0))
        {
            return Err(EditError::Preservation);
        }
        if previous != Some(pair.0) {
            after_distinct += 1;
        }
        previous = Some(pair.0);
        let (wire, width) = wire(*pair, key.descending());
        let next = active_bytes + width;
        if next > PAGE_BYTES - KEY_AT {
            return Err(EditError::Preservation);
        }
        budget
            .charge(Resource::Steps, width as u64)
            .map_err(InspectError::from)?;
        expected
            .get_mut(base + KEY_AT + active_bytes..base + KEY_AT + next)
            .ok_or(EditError::Preservation)?
            .copy_from_slice(&wire[..width]);
        mask[next / 8] |= 1 << (next % 8);
        active_bytes = next;
    }
    budget
        .charge(Resource::Steps, (MASK_BYTES + size_of::<u16>()) as u64)
        .map_err(InspectError::from)?;
    expected
        .get_mut(base + MASK_AT..base + KEY_AT)
        .ok_or(EditError::Preservation)?
        .copy_from_slice(&mask);
    let free = u16::try_from(PAGE_BYTES - KEY_AT - active_bytes)
        .map_err(|_| InspectError::StructureLimit)?;
    expected
        .get_mut(base + FREE_AT..base + FREE_AT + size_of::<u16>())
        .ok_or(EditError::Preservation)?
        .copy_from_slice(&free.to_le_bytes());
    // Only the new active prefix is written. The full expected image retains
    // original headers, prefix0, every inactive tail, all maps and EOF.
    super::reconstruct_history(
        image,
        source,
        index,
        (before_distinct, after_distinct),
        expected,
        budget,
        cancelled,
    )
}

pub(super) fn original_value(
    raw: &[u8],
    schema: &TableSchema,
    field: &NativeColumn,
    observed: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<i32>, EditError> {
    check(budget, cancelled)?;
    let layout = record_layout::parse(raw, schema, budget)?;
    if field.number() >= layout.count {
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
        return Ok(None);
    }
    let start = COLUMN_COUNT_BYTES
        .checked_add(field.field.offset)
        .ok_or(EditError::Preservation)?;
    let end = start
        .checked_add(VALUE_BYTES)
        .ok_or(EditError::Preservation)?;
    if end > layout.mask_start {
        return Err(EditError::Preservation);
    }
    budget
        .charge(Resource::Steps, VALUE_BYTES as u64)
        .map_err(InspectError::from)?;
    let value = i32::from_le_bytes(
        raw.get(start..end)
            .ok_or(EditError::Preservation)?
            .try_into()
            .map_err(|_| EditError::Preservation)?,
    );
    if observed != &Value::Int32(value) {
        return Err(EditError::Preservation);
    }
    Ok(Some(value))
}

pub(super) fn sort(
    values: &mut [Key],
    descending: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let levels = values.len().checked_ilog2().unwrap_or(0) as usize + 1;
    let work = values
        .len()
        .checked_mul(levels)
        .and_then(|count| count.checked_mul(size_of::<Key>()))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    values.sort_unstable_by(|left, right| {
        let order = left.0.cmp(&right.0);
        let order = if descending { order.reverse() } else { order };
        order.then(left.1.cmp(&right.1))
    });
    check(budget, cancelled)?;
    Ok(())
}

pub(super) fn distinct(
    values: &[Key],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    let mut previous = None;
    let mut distinct = 0;
    for pair in values {
        check(budget, cancelled)?;
        if previous != Some(pair.0) {
            distinct += 1;
        }
        previous = Some(pair.0);
    }
    Ok(distinct)
}

pub(super) fn wire((value, reference): Key, descending: bool) -> ([u8; MAX_WIRE_BYTES], usize) {
    let mut raw = [0; MAX_WIRE_BYTES];
    let key_end = if let Some(value) = value {
        raw[0] = if descending {
            DESC_PRESENT
        } else {
            ASC_PRESENT
        };
        let biased = (value as u32) ^ SIGN_BIT;
        let encoded = if descending { !biased } else { biased };
        raw[MARKER_BYTES..MARKER_BYTES + VALUE_BYTES].copy_from_slice(&encoded.to_be_bytes());
        MARKER_BYTES + VALUE_BYTES
    } else {
        raw[0] = if descending { DESC_NULL } else { ASC_NULL };
        MARKER_BYTES
    };
    raw[key_end..key_end + ROW_BYTES].copy_from_slice(&reference.to_be_bytes());
    (raw, key_end + ROW_BYTES)
}

#[cfg(test)]
mod tests;
