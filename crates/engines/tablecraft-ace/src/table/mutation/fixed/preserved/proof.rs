//! Full-image authority independently reconstructed from original requests.
use super::{FixedColumnUpdates, copy, int32, name_work, scalar, source, unique};
use crate::{
    EditError, InspectError, NativeImage, WriteBlocker,
    format::{
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        row::MASK_BITS,
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

pub(super) fn prove(
    image: &NativeImage,
    groups: &[FixedColumnUpdates<'_>],
    after: &[u8],
    returned_count: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    if let [group] = groups
        && group
            .updates()
            .first()
            .is_some_and(|request| matches!(request.expected(), Value::Null))
    {
        let original = source::capture(image, group.table(), budget, cancelled)?;
        let expected = copy(image.as_bytes(), budget, cancelled)?;
        let selected = int32::proof::select_original(&original, group, budget, cancelled)?;
        return match selected.scalar_type() {
            Some(ScalarType::Int32) => int32::proof::prove_bound(
                &original,
                expected,
                groups,
                selected,
                int32::proof::Observed {
                    bytes: after,
                    changed_rows: returned_count,
                },
                budget,
                cancelled,
            ),
            Some(ScalarType::Int64 | ScalarType::Currency | ScalarType::DateTime) => {
                scalar::proof::prove_bound(
                    scalar::proof::Original {
                        image,
                        source: &original,
                    },
                    expected,
                    groups,
                    selected,
                    scalar::proof::Observed {
                        bytes: after,
                        changed_rows: returned_count,
                    },
                    budget,
                    cancelled,
                )
            }
            _ => Err(EditError::Unsupported(WriteBlocker::Column)),
        };
    }
    if scalar::requested_non_null(groups) {
        return scalar::proof::prove(image, groups, after, returned_count, budget, cancelled);
    }
    if int32::requested(groups) {
        return int32::proof::prove(image, groups, after, returned_count, budget, cancelled);
    }
    let table = groups.first().ok_or(EditError::Preservation)?.table();
    let original = source::capture(image, table, budget, cancelled)?;
    let schema = original.table.schema();
    let mut expected = copy(image.as_bytes(), budget, cancelled)?;
    let capacity = groups
        .iter()
        .try_fold(0_usize, |count, group| {
            count.checked_add(group.updates().len())
        })
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge_allocation(capacity as u64, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    let mut references = Vec::new();
    references
        .try_reserve_exact(capacity)
        .map_err(|_| InspectError::Allocation)?;

    for (position, group) in groups.iter().enumerate() {
        check(budget, cancelled)?;
        name_work(table, group.table(), budget)?;
        if !table.eq_ignore_ascii_case(group.table()) {
            return Err(EditError::Preservation);
        }
        for prior in &groups[..position] {
            name_work(prior.column(), group.column(), budget)?;
            if prior.column().eq_ignore_ascii_case(group.column()) {
                return Err(EditError::Conflict);
            }
        }
        let mut selected = None;
        for field in schema.columns() {
            name_work(field.name(), group.column(), budget)?;
            if field.name().eq_ignore_ascii_case(group.column())
                && selected.replace(field).is_some()
            {
                return Err(EditError::Preservation);
            }
        }
        let selected = selected.ok_or(EditError::Unsupported(WriteBlocker::Column))?;
        if selected.scalar_type() == Some(ScalarType::Int32)
            && groups.len() == 1
            && group.updates().is_empty()
        {
            // Reuse only this auditor's freshly recaptured source and original
            // byte copy; neither came from a staged numeric result.
            return int32::proof::prove_original(
                &original,
                expected,
                groups,
                after,
                returned_count,
                budget,
                cancelled,
            );
        }
        if groups.len() == 1
            && group.updates().is_empty()
            && matches!(
                selected.scalar_type(),
                Some(ScalarType::Int64 | ScalarType::Currency | ScalarType::DateTime)
            )
        {
            return scalar::proof::prove_bound(
                scalar::proof::Original {
                    image,
                    source: &original,
                },
                expected,
                groups,
                selected,
                scalar::proof::Observed {
                    bytes: after,
                    changed_rows: returned_count,
                },
                budget,
                cancelled,
            );
        }
        if selected.scalar_type() != Some(ScalarType::Boolean)
            || !selected.fixed()
            || selected.storage_length() != 0
            || selected.extended_flags() != 0
            || selected.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
        {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        let number = selected.number();
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
        for (request_position, request) in group.updates().iter().enumerate() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, request_position as u64)
                .map_err(InspectError::from)?;
            if group.updates()[..request_position]
                .iter()
                .any(|prior| prior.row() == request.row())
            {
                return Err(EditError::Conflict);
            }
            let record = original
                .records
                .get(request.row())
                .ok_or(EditError::Conflict)?;
            let layout = record_layout::parse(&record.data, schema, budget)?;
            let byte = layout.mask_start + number / MASK_BITS;
            let stored = *record.data.get(byte).ok_or(InspectError::Malformed)?;
            let bit = 1 << (number % MASK_BITS);
            let (Value::Boolean(before), Value::Boolean(value)) =
                (request.expected(), request.value())
            else {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            };
            if (stored & bit != 0) != *before {
                return Err(EditError::Conflict);
            }
            if before != value {
                let output = expected
                    .get_mut(record.offset + byte)
                    .ok_or(EditError::Preservation)?;
                if *value {
                    *output |= bit;
                } else {
                    *output &= !bit;
                }
                references.push(record.reference);
            }
        }
    }
    let changed = unique(&mut references, budget, cancelled)?;
    if changed != returned_count || expected.len() != after.len() {
        return Err(EditError::Preservation);
    }
    for (expected, actual) in expected
        .chunks(COPY_CHUNK_BYTES)
        .zip(after.chunks(COPY_CHUNK_BYTES))
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
