//! Independent nullable/directional Binary population and raw leaf reconstruction.
use super::{FREE_AT, KEY_AT, MASK_AT, MASK_BYTES, RequestedRows};
use crate::{
    EditError, InspectError, NativeColumn, NativeImage, NativeIndex, TableSchema,
    format::{bytes::u16_at, page::PAGE_BYTES, row::MASK_BITS},
    header::check,
    table::{mutation::preserved_source::Source, record_layout},
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

type Key<'value> = (Option<&'value [u8]>, u32);

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
    let page = image
        .as_bytes()
        .get(base..base + PAGE_BYTES)
        .ok_or(EditError::Preservation)?;
    let mut original_mask = [0; MASK_BYTES];
    let mut original_bytes = 0_usize;
    for entry in &original {
        check(budget, cancelled)?;
        let (bytes, width) = wire(*entry, key.descending())?;
        let next = original_bytes
            .checked_add(width)
            .ok_or(InspectError::StructureLimit)?;
        if next > PAGE_BYTES - KEY_AT {
            return Err(EditError::Preservation);
        }
        budget
            .charge(Resource::Steps, width as u64)
            .map_err(InspectError::from)?;
        if page.get(KEY_AT + original_bytes..KEY_AT + next) != Some(&bytes[..width]) {
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
    budget
        .charge(
            Resource::Steps,
            (original.len() * size_of::<Key<'_>>()) as u64,
        )
        .map_err(InspectError::from)?;
    final_keys.extend_from_slice(&original);
    for (request, reference) in requests.iter().zip(references) {
        check(budget, cancelled)?;
        let value = requested(
            field,
            request
                .values()
                .get(column)
                .ok_or(EditError::Preservation)?,
        )?;
        final_keys.push((value, *reference));
    }
    sort(&mut final_keys, key.descending(), budget, cancelled)?;
    let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
    let before_distinct = distinct(&original, budget, cancelled)?;
    let after_distinct = distinct(&final_keys, budget, cancelled)?;
    let mut previous: Option<Option<&[u8]>> = None;
    let mut mask = [0; MASK_BYTES];
    let mut active_bytes = 0_usize;
    for entry in &final_keys {
        check(budget, cancelled)?;
        if constraints.unique() && entry.0.is_some() {
            let bytes = entry
                .0
                .map_or(0, <[u8]>::len)
                .max(previous.flatten().map_or(0, <[u8]>::len));
            budget
                .charge(Resource::Steps, bytes as u64)
                .map_err(InspectError::from)?;
        }
        if constraints.required() && entry.0.is_none()
            || constraints.unique() && entry.0.is_some() && previous == Some(entry.0)
        {
            return Err(EditError::Preservation);
        }
        previous = Some(entry.0);
        let (bytes, width) = wire(*entry, key.descending())?;
        let next = active_bytes
            .checked_add(width)
            .ok_or(InspectError::StructureLimit)?;
        if next > PAGE_BYTES - KEY_AT {
            return Err(EditError::Preservation);
        }
        budget
            .charge(Resource::Steps, width as u64)
            .map_err(InspectError::from)?;
        expected
            .get_mut(base + KEY_AT + active_bytes..base + KEY_AT + next)
            .ok_or(EditError::Preservation)?
            .copy_from_slice(&bytes[..width]);
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

    // The original clone retains every inactive tail byte, declaration, map,
    // property and unrelated object. Only each physical active prefix changes.
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

fn original_value<'value>(
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

fn requested<'value>(
    field: &NativeColumn,
    value: &'value Value,
) -> Result<Option<&'value [u8]>, EditError> {
    match value {
        Value::Null => Ok(None),
        Value::Binary(bytes) if bytes.len() <= field.storage_length() => Ok(Some(bytes)),
        _ => Err(EditError::Preservation),
    }
}

fn wire(entry: Key<'_>, descending: bool) -> Result<([u8; MAX_WIRE_BYTES], usize), EditError> {
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

fn sort(
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

fn distinct(
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
