//! Joint encoding selection precedes emission and is ordered by native slot.
use super::{request, serialization::RecordPolicy};
use crate::{
    EditError, InspectError, NativeColumn, TableSchema, WriteBlocker,
    format::layout::UNICODE_COMPRESSION_FLAG,
    table::{
        mutation::{page_update::record, slot::storage},
        record_layout::RecordLayout,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const MARKER_BYTES: usize = 2;
const UTF16_BYTES: usize = size_of::<u16>();

pub(super) struct Cell<'a> {
    pub column: usize,
    pub value: &'a Value,
}

struct Choice {
    selected: usize,
    preferred: usize,
    minimum: usize,
    length: usize,
}

pub(super) fn rebuild(
    schema: &TableSchema,
    before: &[u8],
    layout: &RecordLayout,
    cells: &[Cell<'_>],
    policy: RecordPolicy,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    match cells {
        [] => return super::stage::copy(before, budget, cancelled),
        [cell] => return single(schema, before, layout, cell, policy, budget, cancelled),
        _ => {}
    }
    budget
        .charge_allocation(cells.len() as u64, size_of::<Choice>() as u64)
        .map_err(InspectError::from)?;
    let mut choices = Vec::new();
    choices
        .try_reserve_exact(cells.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut removed = 0_usize;
    let mut preferred = 0_usize;
    for (selected, cell) in cells.iter().enumerate() {
        request::check(budget, cancelled)?;
        let field = &schema.columns()[cell.column];
        let variable = field.field.variable;
        let start = *layout
            .offsets
            .get(variable)
            .ok_or(InspectError::Malformed)?;
        let end = *layout
            .offsets
            .get(variable + 1)
            .ok_or(InspectError::Malformed)?;
        removed = removed
            .checked_add(end.checked_sub(start).ok_or(InspectError::Malformed)?)
            .ok_or(InspectError::StructureLimit)?;
        let (declared, minimum) = lengths(field, cell.value, budget, cancelled)?;
        let length = if matches!(policy, RecordPolicy::FittedDeclared) {
            declared
        } else {
            minimum
        };
        preferred = preferred
            .checked_add(length)
            .ok_or(InspectError::StructureLimit)?;
        choices.push(Choice {
            selected,
            preferred: declared,
            minimum,
            length,
        });
    }
    let levels = choices.len().checked_ilog2().unwrap_or(0) as usize + 1;
    budget
        .charge(Resource::Steps, choices.len().saturating_mul(levels) as u64)
        .map_err(InspectError::from)?;
    choices.sort_unstable_by_key(|choice| {
        schema.columns()[cells[choice.selected].column]
            .field
            .variable
    });

    if matches!(policy, RecordPolicy::FittedDeclared) {
        let preserved = layout
            .offsets
            .last()
            .ok_or(InspectError::Malformed)?
            .checked_sub(removed)
            .ok_or(InspectError::Malformed)?;
        let capacity = layout
            .data_end
            .checked_sub(preserved)
            .ok_or(InspectError::Malformed)?;
        for choice in &mut choices {
            request::check(budget, cancelled)?;
            if preferred <= capacity {
                break;
            }
            preferred -= choice.preferred - choice.minimum;
            choice.length = choice.minimum;
        }
        if preferred > capacity {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
    }
    budget
        .charge_allocation(
            choices.len() as u64,
            size_of::<record::multiple::Payload>() as u64,
        )
        .map_err(InspectError::from)?;
    let mut payloads = Vec::new();
    payloads
        .try_reserve_exact(choices.len())
        .map_err(|_| InspectError::Allocation)?;
    for choice in choices {
        request::check(budget, cancelled)?;
        let cell = &cells[choice.selected];
        let field = &schema.columns()[cell.column];
        let bytes = if matches!(policy, RecordPolicy::GrowingPreferred)
            || matches!(cell.value, Value::Null)
        {
            storage::encode_preferred(field, cell.value, budget, cancelled)?
        } else {
            storage::encode_for_span(field, cell.value, choice.length, budget, cancelled)?
        };
        payloads.push(record::multiple::Payload {
            column: cell.column,
            bytes,
            present: !matches!(cell.value, Value::Null),
        });
    }
    record::multiple::compose(schema, before, &payloads, budget, cancelled)
}

fn lengths(
    field: &NativeColumn,
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(usize, usize), EditError> {
    match value {
        Value::Null => Ok((0, 0)),
        Value::Binary(bytes) => Ok((bytes.len(), bytes.len())),
        Value::Text(text) => {
            let mut count = 0_usize;
            let mut narrow = true;
            for unit in text.encode_utf16() {
                request::check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, 1)
                    .map_err(InspectError::from)?;
                count = count.checked_add(1).ok_or(InspectError::StructureLimit)?;
                narrow &= unit > 0 && unit <= u16::from(u8::MAX);
            }
            let raw = count
                .checked_mul(UTF16_BYTES)
                .ok_or(InspectError::StructureLimit)?;
            if narrow && field.extended_flags() & UNICODE_COMPRESSION_FLAG != 0 {
                let declared = count
                    .checked_add(MARKER_BYTES)
                    .ok_or(InspectError::StructureLimit)?;
                Ok((declared, declared.min(raw)))
            } else {
                Ok((raw, raw))
            }
        }
        _ => Err(EditError::Unsupported(WriteBlocker::Column)),
    }
}

fn single(
    schema: &TableSchema,
    before: &[u8],
    layout: &RecordLayout,
    cell: &Cell<'_>,
    policy: RecordPolicy,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    if matches!(policy, RecordPolicy::GrowingPreferred) {
        return record::build(schema, cell.column, before, cell.value, budget, cancelled);
    }
    let variable = schema.columns()[cell.column].field.variable;
    let start = *layout
        .offsets
        .get(variable)
        .ok_or(InspectError::Malformed)?;
    let end = *layout
        .offsets
        .get(variable + 1)
        .ok_or(InspectError::Malformed)?;
    let preserved = layout
        .offsets
        .last()
        .ok_or(InspectError::Malformed)?
        .checked_sub(end - start)
        .ok_or(InspectError::Malformed)?;
    let capacity = layout
        .data_end
        .checked_sub(preserved)
        .ok_or(InspectError::Malformed)?;
    let payload = storage::encode_declared(
        &schema.columns()[cell.column],
        cell.value,
        capacity,
        budget,
        cancelled,
    )?;
    record::replace_payload(
        schema,
        cell.column,
        before,
        &payload,
        !matches!(cell.value, Value::Null),
        budget,
        cancelled,
    )
}
