//! Independent Binary proof values/wire; producer serialization is separate.
use crate::{
    EditError, InspectError, NativeColumn, TableSchema, format::row::MASK_BITS, header::check,
    table::record_layout,
};
use std::cmp::Ordering;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const SEGMENT_BYTES: usize = 8;
const SEGMENT_WIRE_BYTES: usize = SEGMENT_BYTES + size_of::<u8>();
const MARKER_BYTES: usize = size_of::<u8>();
const CONTINUED_TAG: u8 = SEGMENT_BYTES as u8 + 1;
const ASCENDING_NULL_MARKER: u8 = 0;
const ASCENDING_PRESENT_MARKER: u8 = 0x7f;
const ASCENDING_PADDING: u8 = 0;
const MAX_VALUE_BYTES: usize = 255;
const ROW_BYTES: usize = size_of::<u32>();
const MAX_WIRE_BYTES: usize =
    MARKER_BYTES + MAX_VALUE_BYTES.div_ceil(SEGMENT_BYTES) * SEGMENT_WIRE_BYTES + ROW_BYTES;

pub(in crate::table) type Key<'value> = (Option<&'value [u8]>, u32);

pub(in crate::table) fn original_value<'value>(
    raw: &[u8],
    schema: &TableSchema,
    field: &NativeColumn,
    observed: &'value Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<&'value [u8]>, EditError> {
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
    let start = *layout
        .offsets
        .get(field.field.variable)
        .ok_or(EditError::Preservation)?;
    let end = *layout
        .offsets
        .get(field.field.variable + 1)
        .ok_or(EditError::Preservation)?;
    let actual = raw.get(start..end).ok_or(EditError::Preservation)?;
    let Some(value) = requested(field, observed)? else {
        return Err(EditError::Preservation);
    };
    budget
        .charge(Resource::Steps, actual.len() as u64)
        .map_err(InspectError::from)?;
    if actual != value {
        return Err(EditError::Preservation);
    }
    Ok(Some(value))
}

pub(in crate::table) fn requested<'value>(
    field: &NativeColumn,
    value: &'value Value,
) -> Result<Option<&'value [u8]>, EditError> {
    match value {
        Value::Null => Ok(None),
        Value::Binary(bytes) if bytes.len() <= field.storage_length() => Ok(Some(bytes)),
        _ => Err(EditError::Preservation),
    }
}

pub(in crate::table) fn wire(
    entry: Key<'_>,
    descending: bool,
) -> Result<([u8; MAX_WIRE_BYTES], usize), EditError> {
    let mut result = [0; MAX_WIRE_BYTES];
    let mut end = MARKER_BYTES;
    if let Some(value) = entry.0 {
        if value.len() > MAX_VALUE_BYTES {
            return Err(EditError::Preservation);
        }
        result[0] = if descending {
            !ASCENDING_PRESENT_MARKER
        } else {
            ASCENDING_PRESENT_MARKER
        };
        let padding = if descending {
            !ASCENDING_PADDING
        } else {
            ASCENDING_PADDING
        };

        for (position, chunk) in value.chunks(SEGMENT_BYTES).enumerate() {
            let segment = result
                .get_mut(end..end + SEGMENT_WIRE_BYTES)
                .ok_or(EditError::Preservation)?;
            segment[..SEGMENT_BYTES].fill(padding);
            for (target, byte) in segment.iter_mut().zip(chunk) {
                *target = if descending { !byte } else { *byte };
            }
            segment[SEGMENT_BYTES] = if (position + 1) * SEGMENT_BYTES < value.len() {
                CONTINUED_TAG
            } else if descending {
                !(chunk.len() as u8)
            } else {
                chunk.len() as u8
            };
            end += SEGMENT_WIRE_BYTES;
        }
    } else {
        result[0] = if descending {
            !ASCENDING_NULL_MARKER
        } else {
            ASCENDING_NULL_MARKER
        };
    }
    result
        .get_mut(end..end + ROW_BYTES)
        .ok_or(EditError::Preservation)?
        .copy_from_slice(&entry.1.to_be_bytes());
    Ok((result, end + ROW_BYTES))
}

pub(in crate::table) fn sort(
    values: &mut [Key<'_>],
    descending: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let levels = values.len().checked_ilog2().unwrap_or(0) as usize + 1;
    let work = values
        .len()
        .checked_mul(levels)
        .and_then(|count| count.checked_mul(2 * MAX_WIRE_BYTES))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    values.sort_unstable_by(|left, right| {
        compare_values(left.0, right.0, descending).then_with(|| left.1.cmp(&right.1))
    });
    check(budget, cancelled)?;
    Ok(())
}

fn compare_values(left: Option<&[u8]>, right: Option<&[u8]>, descending: bool) -> Ordering {
    // The observed wire has a marker alone for empty Binary. That marker is
    // a proper prefix of every nonempty present component in either direction.
    if let (Some(left), Some(right)) = (left, right)
        && (left.is_empty() || right.is_empty())
    {
        return left.len().cmp(&right.len());
    }
    let order = left.cmp(&right);
    if descending { order.reverse() } else { order }
}

pub(in crate::table) fn distinct(
    values: &[Key<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    let mut count = 0;
    for (position, entry) in values.iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, MAX_VALUE_BYTES as u64)
            .map_err(InspectError::from)?;
        if position == 0 || values[position - 1].0 != entry.0 {
            count += 1;
        }
    }
    Ok(count)
}
