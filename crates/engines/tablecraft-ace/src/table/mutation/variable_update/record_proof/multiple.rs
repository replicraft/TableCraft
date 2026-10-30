//! Whole-row proof derives all payload choices afresh from immutable requests.
use super::super::super::{EditError, WriteBlocker};
use super::{copy, payload};
use crate::{
    InspectError, NativeColumn, TableSchema,
    format::{
        layout::UNICODE_COMPRESSION_FLAG,
        page::PAGE_BYTES,
        row::{MASK_BITS, VARIABLE_COUNT_BYTES, VARIABLE_OFFSET_BYTES, slot_offset},
    },
    header::check,
    table::record_layout,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const MARKER: usize = 2;
const WORD: usize = size_of::<u16>();

pub(in crate::table::mutation) struct Requested<'a> {
    pub column: usize,
    pub value: &'a Value,
}

#[derive(Clone, Copy)]
pub(in crate::table::mutation) enum Encoding {
    Fitted,
    Growing,
}

struct Selection<'a> {
    field: &'a NativeColumn,
    value: &'a Value,
    length: usize,
    minimum: usize,
    bytes: Vec<u8>,
}

pub(in crate::table::mutation) fn reconstruct(
    schema: &TableSchema,
    before: &[u8],
    requests: &[Requested<'_>],
    encoding: Encoding,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    check(budget, cancelled)?;
    let frame = record_layout::parse(before, schema, budget)?;
    if frame.count != schema.layout.max_columns {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let variables = frame
        .offsets
        .len()
        .checked_sub(1)
        .ok_or(EditError::Preservation)?;
    let origin = *frame.offsets.first().ok_or(EditError::Preservation)?;
    budget
        .charge_allocation(requests.len() as u64, size_of::<Selection<'_>>() as u64)
        .map_err(InspectError::from)?;
    let mut selected = Vec::new();
    selected
        .try_reserve_exact(requests.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut old_selected = 0_usize;
    let mut planned = 0_usize;
    for request in requests {
        check(budget, cancelled)?;
        let field = schema
            .columns()
            .get(request.column)
            .ok_or(EditError::Preservation)?;
        if field.fixed() || field.field.variable >= variables {
            return Err(EditError::Preservation);
        }
        let slot = field.field.variable;
        let width = frame.offsets[slot + 1]
            .checked_sub(frame.offsets[slot])
            .ok_or(EditError::Preservation)?;
        old_selected = old_selected
            .checked_add(width)
            .ok_or(EditError::Preservation)?;
        let (preferred, minimum) = measure(field, request.value, budget, cancelled)?;
        let length = match encoding {
            Encoding::Fitted => preferred,
            Encoding::Growing => minimum,
        };
        planned = planned.checked_add(length).ok_or(EditError::Preservation)?;
        selected.push(Selection {
            field,
            value: request.value,
            length,
            minimum,
            bytes: Vec::new(),
        });
    }
    let levels = selected.len().checked_ilog2().unwrap_or(0) as usize + 1;
    budget
        .charge(
            Resource::Steps,
            selected.len().saturating_mul(levels) as u64,
        )
        .map_err(InspectError::from)?;
    selected.sort_unstable_by_key(|item| item.field.field.variable);
    for pair in selected.windows(2) {
        check(budget, cancelled)?;
        if pair[0].field.field.variable >= pair[1].field.field.variable {
            return Err(EditError::Preservation);
        }
    }
    if matches!(encoding, Encoding::Fitted) {
        let untouched = frame
            .offsets
            .last()
            .ok_or(EditError::Preservation)?
            .checked_sub(old_selected)
            .ok_or(EditError::Preservation)?;
        let available = frame
            .data_end
            .checked_sub(untouched)
            .ok_or(EditError::Preservation)?;
        for item in &mut selected {
            check(budget, cancelled)?;
            if planned <= available {
                break;
            }
            planned = planned
                .checked_sub(item.length - item.minimum)
                .ok_or(EditError::Preservation)?;
            item.length = item.minimum;
        }
        if planned > available {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
    }
    for item in &mut selected {
        check(budget, cancelled)?;
        item.bytes = match encoding {
            Encoding::Fitted => {
                payload::encode_declared(item.field, item.value, item.length, budget, cancelled)?
            }
            Encoding::Growing => payload::encode(item.field, item.value, budget, cancelled)?,
        };
        if item.bytes.len() != item.length {
            return Err(EditError::Preservation);
        }
    }

    budget
        .charge_allocation((variables + 1) as u64, size_of::<usize>() as u64)
        .map_err(InspectError::from)?;
    let mut ends = Vec::new();
    ends.try_reserve_exact(variables + 1)
        .map_err(|_| InspectError::Allocation)?;
    ends.push(origin);
    let mut selection = 0;
    let mut end = origin;
    for slot in 0..variables {
        check(budget, cancelled)?;
        let count = if selected
            .get(selection)
            .is_some_and(|item| item.field.field.variable == slot)
        {
            let length = selected[selection].bytes.len();
            selection += 1;
            length
        } else {
            frame.offsets[slot + 1] - frame.offsets[slot]
        };
        end = end.checked_add(count).ok_or(EditError::Preservation)?;
        ends.push(end);
    }
    let footer = before
        .len()
        .checked_sub(frame.data_end)
        .ok_or(EditError::Preservation)?;
    let length = before
        .len()
        .max(end.checked_add(footer).ok_or(EditError::Preservation)?);
    if length > PAGE_BYTES - slot_offset(1) {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    budget
        .charge_allocation(length as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, length as u64)
        .map_err(InspectError::from)?;
    let mut expected = Vec::new();
    expected
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    expected.resize(length, 0);
    copy(before, &mut expected[..before.len()], budget, cancelled)?;

    selection = 0;
    for slot in 0..variables {
        check(budget, cancelled)?;
        let source = if selected
            .get(selection)
            .is_some_and(|item| item.field.field.variable == slot)
        {
            let bytes = selected[selection].bytes.as_slice();
            selection += 1;
            bytes
        } else {
            &before[frame.offsets[slot]..frame.offsets[slot + 1]]
        };
        copy(
            source,
            &mut expected[ends[slot]..ends[slot + 1]],
            budget,
            cancelled,
        )?;
    }
    let mask = length
        .checked_sub(frame.count.div_ceil(MASK_BITS))
        .ok_or(EditError::Preservation)?;
    copy(
        &before[frame.mask_start..],
        &mut expected[mask..],
        budget,
        cancelled,
    )?;
    for item in &selected {
        check(budget, cancelled)?;
        let number = item.field.number();
        let bit = 1 << (number % MASK_BITS);
        let byte = expected
            .get_mut(mask + number / MASK_BITS)
            .ok_or(EditError::Preservation)?;
        *byte = (*byte & !bit)
            | if matches!(item.value, Value::Null) {
                0
            } else {
                bit
            };
    }
    let count = mask
        .checked_sub(VARIABLE_COUNT_BYTES)
        .ok_or(EditError::Preservation)?;
    expected[count..mask].copy_from_slice(
        &u16::try_from(variables)
            .map_err(|_| EditError::Preservation)?
            .to_le_bytes(),
    );
    for (position, end) in ends.into_iter().enumerate() {
        check(budget, cancelled)?;
        let at = count
            .checked_sub((position + 1) * VARIABLE_OFFSET_BYTES)
            .ok_or(EditError::Preservation)?;
        expected[at..at + VARIABLE_OFFSET_BYTES].copy_from_slice(
            &u16::try_from(end)
                .map_err(|_| EditError::Preservation)?
                .to_le_bytes(),
        );
    }
    record_layout::parse(&expected, schema, budget)?;
    Ok(expected)
}

fn measure(
    field: &NativeColumn,
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(usize, usize), EditError> {
    match value {
        Value::Null => Ok((0, 0)),
        Value::Binary(bytes) => Ok((bytes.len(), bytes.len())),
        Value::Text(text) => {
            let mut units = 0_usize;
            let mut compact = field.extended_flags() & UNICODE_COMPRESSION_FLAG != 0;
            for unit in text.encode_utf16() {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, 1)
                    .map_err(InspectError::from)?;
                units = units.checked_add(1).ok_or(EditError::Preservation)?;
                compact &= unit != 0 && unit <= u16::from(u8::MAX);
            }
            let raw = units.checked_mul(WORD).ok_or(EditError::Preservation)?;
            if compact {
                let preferred = units.checked_add(MARKER).ok_or(EditError::Preservation)?;
                Ok((preferred, preferred.min(raw)))
            } else {
                Ok((raw, raw))
            }
        }
        _ => Err(EditError::Unsupported(WriteBlocker::Column)),
    }
}
