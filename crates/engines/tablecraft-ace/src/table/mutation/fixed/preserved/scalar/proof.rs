//! Eight-byte candidate authority reconstructed without producer encodings.
use super::super::{
    FixedColumnUpdates, copy, name_work,
    source::{self, Source},
    unique,
};
use crate::{
    EditError, InspectError, NativeColumn, NativeImage, WriteBlocker,
    format::{
        data_type::{CURRENCY, DATE_TIME, INT64},
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

const PAYLOAD_BYTES: usize = size_of::<i64>();

pub(in crate::table::mutation::fixed::preserved) struct Original<'a> {
    pub image: &'a NativeImage,
    pub source: &'a Source,
}

pub(in crate::table::mutation::fixed::preserved) struct Observed<'a> {
    pub bytes: &'a [u8],
    pub changed_rows: usize,
}

pub(in crate::table::mutation::fixed::preserved) fn prove(
    image: &NativeImage,
    groups: &[FixedColumnUpdates<'_>],
    after: &[u8],
    count: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let [group] = groups else {
        return Err(EditError::Preservation);
    };
    let original = source::capture(image, group.table(), budget, cancelled)?;
    let selected =
        super::super::int32::proof::select_original(&original, group, budget, cancelled)?;
    let expected = copy(image.as_bytes(), budget, cancelled)?;
    prove_bound(
        Original {
            image,
            source: &original,
        },
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

pub(in crate::table::mutation::fixed::preserved) fn prove_bound(
    original: Original<'_>,
    mut expected: Vec<u8>,
    groups: &[FixedColumnUpdates<'_>],
    selected: &NativeColumn,
    observed: Observed<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let [group] = groups else {
        return Err(EditError::Preservation);
    };
    if group.updates().len() > NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS {
        return Err(InspectError::StructureLimit.into());
    }
    let source = original.source;
    let schema = source.table.schema();
    name_work(schema.name(), group.table(), budget)?;
    name_work(selected.name(), group.column(), budget)?;
    budget
        .charge(Resource::Steps, schema.columns().len() as u64)
        .map_err(InspectError::from)?;
    if !schema.name().eq_ignore_ascii_case(group.table())
        || !selected.name().eq_ignore_ascii_case(group.column())
        || !schema
            .columns()
            .iter()
            .any(|field| std::ptr::eq(field, selected))
    {
        return Err(EditError::Preservation);
    }
    let kind = match (selected.native_type(), selected.scalar_type()) {
        (INT64, Some(ScalarType::Int64)) => ScalarType::Int64,
        (CURRENCY, Some(ScalarType::Currency)) => ScalarType::Currency,
        (DATE_TIME, Some(ScalarType::DateTime)) => ScalarType::DateTime,
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    };
    if !selected.fixed()
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
    for field in schema.columns() {
        check(budget, cancelled)?;
        if field.number() != number && field.fixed() && field.storage_length() != 0 {
            let end = field
                .field
                .offset
                .checked_add(field.storage_length())
                .ok_or(EditError::Preservation)?;
            if field.field.offset < fixed_end && end > selected.field.offset {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
        }
    }
    for dependency in &source.dependencies {
        check(budget, cancelled)?;
        if *dependency == number {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }
    for index in source.indexes.indexes() {
        for column in index.columns() {
            check(budget, cancelled)?;
            if column.number() == number {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
        }
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
            .charge(
                Resource::Steps,
                position as u64 + (PAYLOAD_BYTES * 2) as u64,
            )
            .map_err(InspectError::from)?;
        if group.updates()[..position]
            .iter()
            .any(|prior| prior.row() == request.row())
        {
            return Err(EditError::Conflict);
        }
        let before = encode(kind, request.expected())?;
        let value = encode(kind, request.value())?;
        let record = source
            .records
            .get(request.row())
            .ok_or(EditError::Conflict)?;
        let layout = record_layout::parse(&record.data, schema, budget)?;
        if number >= layout.count {
            return Err(EditError::Preservation);
        }
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
        let actual = record.data.get(at..end).ok_or(InspectError::Malformed)?;
        if before.is_some_and(|before| actual != before.as_slice()) {
            return Err(EditError::Conflict);
        }
        if before == value {
            // Equality is exact carrier/payload equality. NULL never inspects or
            // normalizes inactive bytes, including an invalid inactive DATE.
            continue;
        }
        if let Some(payload) = value {
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
                .copy_from_slice(&payload);
        }
        if before.is_some() != value.is_some() {
            let at = record
                .offset
                .checked_add(layout.mask_start)
                .and_then(|at| at.checked_add(number / MASK_BITS))
                .ok_or(EditError::Preservation)?;
            *expected.get_mut(at).ok_or(EditError::Preservation)? = if value.is_some() {
                mask | bit
            } else {
                mask & !bit
            };
        }
        references.push(record.reference);
    }
    if unique(&mut references, budget, cancelled)? != observed.changed_rows
        || expected.len() != observed.bytes.len()
    {
        return Err(EditError::Preservation);
    }
    super::super::validation::expected(
        original.image,
        group.table(),
        &source.table,
        &expected,
        budget,
        cancelled,
    )?;
    for (expected, actual) in expected
        .chunks(COPY_CHUNK_BYTES)
        .zip(observed.bytes.chunks(COPY_CHUNK_BYTES))
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, expected.len() as u64)
            .map_err(InspectError::from)?;
        if expected != actual {
            return Err(EditError::Preservation);
        }
    }
    check(budget, cancelled)?;
    Ok(())
}

fn encode(kind: ScalarType, value: &Value) -> Result<Option<[u8; PAYLOAD_BYTES]>, EditError> {
    Ok(Some(match (kind, value) {
        (_, Value::Null) => return Ok(None),
        (ScalarType::Int64, Value::Int64(value)) => value.to_le_bytes(),
        (ScalarType::Currency, Value::Currency(value)) => value.scaled().to_le_bytes(),
        (ScalarType::DateTime, Value::DateTime(value)) => value.to_bits().to_le_bytes(),
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    }))
}

#[cfg(test)]
mod tests;
