//! Reconstruct exact permitted payload bytes from the original image/requests.
use super::super::{
    FixedColumnUpdates, copy, name_work,
    source::{self, Source},
    unique,
};
use crate::{
    EditError, InspectError, NativeColumn, NativeImage, WriteBlocker,
    format::{
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        row::{COLUMN_COUNT_BYTES, MASK_BITS},
    },
    header::check,
    preservation::COPY_CHUNK_BYTES,
    table::record_layout,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

const PAYLOAD_BYTES: usize = size_of::<i32>();

pub(in crate::table::mutation::fixed::preserved) fn prove(
    image: &NativeImage,
    groups: &[FixedColumnUpdates<'_>],
    after: &[u8],
    count: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let [group] = groups else {
        return Err(EditError::Preservation);
    };
    let original = source::capture(image, group.table(), budget, cancelled)?;
    let expected = copy(image.as_bytes(), budget, cancelled)?;
    prove_original(&original, expected, groups, after, count, budget, cancelled)
}

pub(in crate::table::mutation::fixed::preserved) fn prove_original(
    original: &Source,
    expected: Vec<u8>,
    groups: &[FixedColumnUpdates<'_>],
    after: &[u8],
    count: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let [group] = groups else {
        return Err(EditError::Preservation);
    };
    let selected = select_original(original, group, budget, cancelled)?;
    prove_bound(
        original,
        expected,
        groups,
        selected,
        Observed {
            bytes: after,
            changed_rows: count,
        },
        budget,
        cancelled,
    )
}

pub(in crate::table::mutation::fixed::preserved) fn select_original<'a>(
    original: &'a Source,
    group: &FixedColumnUpdates<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'a NativeColumn, EditError> {
    let schema = original.table.schema();
    name_work(schema.name(), group.table(), budget)?;
    if !schema.name().eq_ignore_ascii_case(group.table()) {
        return Err(EditError::Preservation);
    }
    let mut selected = None;
    for field in schema.columns() {
        check(budget, cancelled)?;
        name_work(field.name(), group.column(), budget)?;
        if field.name().eq_ignore_ascii_case(group.column()) && selected.replace(field).is_some() {
            return Err(EditError::Preservation);
        }
    }
    let selected = selected.ok_or(EditError::Unsupported(WriteBlocker::Column))?;
    Ok(selected)
}

pub(in crate::table::mutation::fixed::preserved) struct Observed<'a> {
    pub bytes: &'a [u8],
    pub changed_rows: usize,
}

pub(in crate::table::mutation::fixed::preserved) fn prove_bound(
    original: &Source,
    mut expected: Vec<u8>,
    groups: &[FixedColumnUpdates<'_>],
    selected: &NativeColumn,
    observed: Observed<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let [group] = groups else {
        return Err(EditError::Preservation);
    };
    let schema = original.table.schema();
    let Observed {
        bytes: after,
        changed_rows: count,
    } = observed;
    if selected.scalar_type() != Some(ScalarType::Int32)
        || !selected.fixed()
        || selected.storage_length() != PAYLOAD_BYTES
        || selected.extended_flags() != 0
        || selected.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
    {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    let number = selected.number();
    let fixed_end = selected
        .field
        .offset
        .checked_add(PAYLOAD_BYTES)
        .ok_or(EditError::Preservation)?;
    for other in schema.columns() {
        check(budget, cancelled)?;
        if other.number() != number && other.fixed() && other.storage_length() != 0 {
            let other_end = other
                .field
                .offset
                .checked_add(other.storage_length())
                .ok_or(EditError::Preservation)?;
            if other.field.offset < fixed_end && other_end > selected.field.offset {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
        }
    }
    budget
        .charge(
            Resource::Steps,
            (original.dependencies.len()
                + original
                    .indexes
                    .indexes()
                    .iter()
                    .map(|index| index.columns().len())
                    .sum::<usize>()) as u64,
        )
        .map_err(InspectError::from)?;
    if original.dependencies.contains(&number)
        || original
            .indexes
            .indexes()
            .iter()
            .any(|index| index.columns().iter().any(|key| key.number() == number))
    {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    budget
        .charge_allocation(group.updates().len() as u64, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    let mut references = Vec::new();
    references
        .try_reserve_exact(group.updates().len())
        .map_err(|_| InspectError::Allocation)?;

    for (position, request) in group.updates().iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, position as u64)
            .map_err(InspectError::from)?;
        if group.updates()[..position]
            .iter()
            .any(|prior| prior.row() == request.row())
        {
            return Err(EditError::Conflict);
        }
        let before = match request.expected() {
            Value::Null => None,
            Value::Int32(value) => Some(*value),
            _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
        };
        let value = match request.value() {
            Value::Null => None,
            Value::Int32(value) => Some(*value),
            _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
        };
        let record = original
            .records
            .get(request.row())
            .ok_or(EditError::Conflict)?;
        let layout = record_layout::parse(&record.data, schema, budget)?;
        let mask = *record
            .data
            .get(layout.mask_start + number / MASK_BITS)
            .ok_or(InspectError::Malformed)?;
        let bit = 1 << (number % MASK_BITS);
        if (mask & bit != 0) != before.is_some() {
            return Err(EditError::Conflict);
        }
        let at = COLUMN_COUNT_BYTES
            .checked_add(selected.field.offset)
            .ok_or(EditError::Preservation)?;
        let end = at
            .checked_add(PAYLOAD_BYTES)
            .ok_or(EditError::Preservation)?;
        if end > layout.data_end {
            return Err(EditError::Preservation);
        }
        let actual: [u8; PAYLOAD_BYTES] = record
            .data
            .get(at..end)
            .ok_or(InspectError::Malformed)?
            .try_into()
            .map_err(|_| InspectError::Malformed)?;
        if let Some(before) = before
            && i32::from_le_bytes(actual) != before
        {
            return Err(EditError::Conflict);
        }
        if before != value {
            if let Some(value) = value {
                let start = record
                    .offset
                    .checked_add(at)
                    .ok_or(EditError::Preservation)?;
                let end = start
                    .checked_add(PAYLOAD_BYTES)
                    .ok_or(EditError::Preservation)?;
                expected
                    .get_mut(start..end)
                    .ok_or(EditError::Preservation)?
                    .copy_from_slice(&value.to_le_bytes());
            }
            if before.is_some() != value.is_some() {
                let mask_offset = record
                    .offset
                    .checked_add(layout.mask_start)
                    .and_then(|offset| offset.checked_add(number / MASK_BITS))
                    .ok_or(EditError::Preservation)?;
                let after_mask = expected
                    .get_mut(mask_offset)
                    .ok_or(EditError::Preservation)?;
                *after_mask = if value.is_some() {
                    mask | bit
                } else {
                    mask & !bit
                };
            }
            references.push(record.reference);
        }
    }
    if unique(&mut references, budget, cancelled)? != count || expected.len() != after.len() {
        return Err(EditError::Preservation);
    }
    for (expected, observed) in expected
        .chunks(COPY_CHUNK_BYTES)
        .zip(after.chunks(COPY_CHUNK_BYTES))
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, expected.len() as u64)
            .map_err(InspectError::from)?;
        if expected != observed {
            return Err(EditError::Preservation);
        }
    }
    check(budget, cancelled)?;
    Ok(())
}
